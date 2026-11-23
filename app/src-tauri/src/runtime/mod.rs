// Thin async wrapper around the active container runtime CLI (defaults to
// Apple's `container`). Mirrors the parsing logic in cgui's container.rs so
// the rest of the app gets typed `Container` values regardless of which CLI
// is active.
//
// Layout: this file holds the process plumbing (binary lookup, run/spawn,
// timeouts, argument validation, CLI version cache). Everything else lives in
// a submodule and is re-exported below, so callers keep using `runtime::name`.
//   parse            JSON -> model parsers, stats rows, ref counts
//   containers       list/inspect/lifecycle, run-from-image, copy/export
//   images           list/tag/delete/commit/build/save/load
//   volumes_networks list/create/delete + name/size/subnet validation
//   system           services, builder VM, df, prune, clean
//   machine          `container machine ...`
//   registry         registry login/logout/list
//   terminal         open Terminal.app on `exec -it`
//   errors           CLI error-text classification

use anyhow::{anyhow, Context, Result};
use serde::Deserialize;
use serde_json::Value;
use std::process::Stdio;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, Command};

use crate::model::{Container, Image, MemUsage, Network, Volume};

mod containers;
mod errors;
mod images;
mod machine;
mod parse;
mod registry;
mod system;
mod terminal;
mod volumes_networks;

pub use containers::*;
pub use errors::*;
pub use images::*;
pub use machine::*;
pub use parse::*;
pub use registry::*;
pub use system::*;
pub use terminal::*;
pub use volumes_networks::*;

#[cfg(test)]
mod features_test;
#[cfg(test)]
mod fixtures_test;

/// Validate a user-supplied container/machine name against the rules
/// Apple's `container` 1.4+ enforces (`ManagedContainer.nameValid`):
/// `^[a-zA-Z0-9][a-zA-Z0-9_.-]+$`, i.e. at least 2 characters, starting
/// with a letter or digit, and 63 characters or fewer. Older CLIs accept
/// this and more, so validating up front is purely additive.
///
/// Without this check the CLI rejects an invalid name with an unhelpful
/// "container ID `…` is not a valid container ID" — this gives a
/// specific, actionable reason instead.
pub fn validate_entity_name(kind: &str, name: &str) -> Result<()> {
    const MAX_LEN: usize = 63;
    let len = name.chars().count();
    if len > MAX_LEN {
        return Err(anyhow!(
            "{kind} name `{name}` is {len} characters; Apple container 1.4+ allows at most {MAX_LEN}"
        ));
    }
    if len < 2 {
        return Err(anyhow!(
            "{kind} name `{name}` is too short; Apple container 1.4+ requires at least 2 characters"
        ));
    }
    let mut chars = name.chars();
    let first = chars.next().unwrap();
    if !first.is_ascii_alphanumeric() {
        return Err(anyhow!(
            "{kind} name `{name}` must start with a letter or digit and contain only letters, digits, `_`, `.`, `-`"
        ));
    }
    if !chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '-')) {
        return Err(anyhow!(
            "{kind} name `{name}` must start with a letter or digit and contain only letters, digits, `_`, `.`, `-`"
        ));
    }
    Ok(())
}

/// Defence in depth for values handed to the CLI as *positional*
/// arguments (ids, names, image references, paths). We never go through a
/// shell, but a value such as `--all` supplied as an id would still be read
/// by the CLI as a flag. Reject empty values and anything starting with `-`.
/// Users with a file literally named `-foo` can pass `./-foo`.
///
/// Called at the `runtime` module boundary (and from the few
/// `commands.rs` / `pty.rs` spawners) so every caller is covered.
pub fn reject_flag_like(kind: &str, value: &str) -> Result<()> {
    if value.is_empty() {
        return Err(anyhow!("{kind} must not be empty"));
    }
    if value.starts_with('-') {
        let hint = if kind.ends_with("path") || kind == "build context" {
            " (prefix it with `./` to use a path that starts with `-`)"
        } else {
            ""
        };
        return Err(anyhow!(
            "{kind} `{value}` must not start with `-` (it would be read as a command-line flag){hint}"
        ));
    }
    Ok(())
}

/// Prepend the standard macOS install locations to our process's PATH
/// so spawned children can resolve `container` /
/// `trivy` regardless of how the app was launched.
///
/// **Why this is needed:** when a Tauri app is launched from Finder,
/// Spotlight, the Dock, or Launch Services, child processes inherit
/// the launchd PATH (`/usr/bin:/bin:/usr/sbin:/sbin`) — *not* the
/// user's interactive shell PATH. Apple's `container` installer drops
/// the binary at `/usr/local/bin/container`, and Homebrew on Apple
/// Silicon installs to `/opt/homebrew/bin`; both of those are absent
/// from the inherited PATH, so `Command::new("container")` fails with
/// "No such file or directory" even though `which container` works
/// fine in Terminal.
///
/// We deduplicate against existing PATH entries so this is idempotent
/// and safe to call from `setup()`. Called once at boot from lib.rs.
pub fn ensure_user_path() {
    let current = std::env::var("PATH").unwrap_or_default();
    if let Some(patched) = patch_path(&current, COMMON_BIN_DIRS) {
        // Safe: this runs single-threaded during `setup()`, before any
        // tokio worker spawns or pty thread launches.
        std::env::set_var("PATH", patched);
    }
}

const COMMON_BIN_DIRS: &[&str] = &[
    "/usr/local/bin",
    "/usr/local/sbin",
    "/opt/homebrew/bin",
    "/opt/homebrew/sbin",
];

/// Pure helper for `ensure_user_path` so it can be unit-tested without
/// touching process-global env state. Returns the new PATH when at
/// least one entry needs prepending; returns None when every needed
/// dir is already present (so the caller can skip the env mutation).
fn patch_path(current: &str, needed: &[&str]) -> Option<String> {
    let existing: std::collections::HashSet<&str> =
        current.split(':').filter(|s| !s.is_empty()).collect();
    let prepend: Vec<&str> = needed
        .iter()
        .copied()
        .filter(|p| !existing.contains(p))
        .collect();
    if prepend.is_empty() {
        return None;
    }
    Some(if current.is_empty() {
        prepend.join(":")
    } else {
        format!("{}:{}", prepend.join(":"), current)
    })
}

const RUN_TIMEOUT: Duration = Duration::from_secs(8);

/// Timeout for `container run` invocations. Even with `-d` (detach),
/// Apple's `container` may need to pull a missing image and stand up
/// the lightweight VM, which is comfortably out of the 8 s envelope
/// the rest of `run` uses for fast inspect-style queries. 3 minutes
/// is generous enough for a first-pull of a small base image like
/// alpine on a slow connection.
const RUN_LONG_TIMEOUT: Duration = Duration::from_secs(180);

/// Timeout for container lifecycle verbs (start/stop/kill/delete/restart).
/// A graceful stop or a cold VM start routinely exceeds the 8 s fast-query
/// envelope, which would report "timed out" while the work is still running.
const LIFECYCLE_TIMEOUT: Duration = Duration::from_secs(60);

/// Long timeout for `container run`, exposed for stacks.
pub(crate) fn run_long_timeout() -> Duration {
    RUN_LONG_TIMEOUT
}

/// Lifecycle timeout, exposed for stacks.
pub(crate) fn lifecycle_timeout() -> Duration {
    LIFECYCLE_TIMEOUT
}

fn bin() -> &'static str {
    "container"
}

/// Public accessor for the currently-active runtime binary name. Used
/// by callers (e.g. pty.rs) that want to compose their own argv without
/// going through the `run`/`spawn` helpers.
pub fn current_bin() -> &'static str {
    bin()
}

// True when the binary is reachable on PATH. Used so the UI can fall back to
// fixtures on machines without Apple's container runtime installed.
pub async fn available() -> bool {
    Command::new(bin())
        .kill_on_drop(true)
        .arg("--version")
        .output()
        .await
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Render an argv for logs and error text with environment values hidden:
/// `-e KEY=value` / `--env KEY=value` / `--env=KEY=value` become `KEY=***`.
/// Display only; the argv actually executed is never modified.
pub fn mask_env_args<S: AsRef<str>>(args: &[S]) -> Vec<String> {
    fn mask(kv: &str) -> String {
        match kv.split_once('=') {
            Some((k, _)) => format!("{k}=***"),
            None => kv.to_string(), // `-e KEY` (inherit) carries no value
        }
    }
    let mut out = Vec::with_capacity(args.len());
    let mut mask_next = false;
    for a in args {
        let a = a.as_ref();
        if mask_next {
            out.push(mask(a));
            mask_next = false;
        } else if a == "-e" || a == "--env" {
            out.push(a.to_string());
            mask_next = true;
        } else if let Some(kv) = a.strip_prefix("--env=") {
            out.push(format!("--env={}", mask(kv)));
        } else {
            out.push(a.to_string());
        }
    }
    out
}

async fn run(args: &[&str]) -> Result<Vec<u8>> {
    run_with_timeout(args, RUN_TIMEOUT).await
}

/// Variant of `run` with a caller-chosen timeout for operations that
/// are inherently slower than fast inspect / list queries (image pull,
/// `container run`, etc.). Same error envelope as `run`.
pub(crate) async fn run_with_timeout(args: &[&str], timeout: Duration) -> Result<Vec<u8>> {
    let out = run_output(args, timeout).await?;
    if !out.status.success() {
        return Err(anyhow!(
            "`{} {}` exited {}: {}",
            bin(),
            mask_env_args(args).join(" "),
            out.status,
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    Ok(out.stdout)
}

/// Spawn the runtime and collect its output regardless of exit status.
/// Errors only on spawn failure or timeout; callers decide what a
/// non-zero exit means.
async fn run_output(args: &[&str], timeout: Duration) -> Result<std::process::Output> {
    let fut = Command::new(bin()).args(args).kill_on_drop(true).output();
    tokio::time::timeout(timeout, fut)
        .await
        .map_err(|_| {
            anyhow!(
                "`{} {}` timed out after {}s",
                bin(),
                mask_env_args(args).join(" "),
                timeout.as_secs()
            )
        })?
        .with_context(|| {
            format!(
                "failed to spawn `{} {}`",
                bin(),
                mask_env_args(args).join(" ")
            )
        })
}

/// Split a command line into words the way a POSIX shell would, minus
/// every expansion (no variables, globs, substitution or operators):
///
/// * unquoted whitespace separates words;
/// * `'...'` is literal;
/// * inside `"..."` a backslash escapes only `"`, `\`, `$` and `` ` ``
///   (before any other character it is kept as-is), as in POSIX;
/// * outside quotes a backslash makes the next character literal, and
///   backslash-newline is a line continuation;
/// * `""` / `''` produce an empty word.
///
/// An unterminated quote (or a trailing backslash) is an error so the user
/// sees what is wrong instead of silently running a different command.
pub fn split_command(input: &str) -> Result<Vec<String>> {
    let mut words: Vec<String> = Vec::new();
    let mut cur = String::new();
    let mut in_word = false;
    let mut chars = input.chars();
    while let Some(c) = chars.next() {
        match c {
            c if c.is_whitespace() => {
                if in_word {
                    words.push(std::mem::take(&mut cur));
                    in_word = false;
                }
            }
            '\'' => {
                in_word = true;
                loop {
                    match chars.next() {
                        Some('\'') => break,
                        Some(ch) => cur.push(ch),
                        None => {
                            return Err(anyhow!("unterminated single quote (') in: {input}"));
                        }
                    }
                }
            }
            '"' => {
                in_word = true;
                loop {
                    match chars.next() {
                        Some('"') => break,
                        Some('\\') => match chars.next() {
                            Some(e @ ('"' | '\\' | '$' | '`')) => cur.push(e),
                            Some('\n') => {}
                            Some(other) => {
                                cur.push('\\');
                                cur.push(other);
                            }
                            None => {
                                return Err(anyhow!("unterminated double quote (\") in: {input}"));
                            }
                        },
                        Some(ch) => cur.push(ch),
                        None => {
                            return Err(anyhow!("unterminated double quote (\") in: {input}"));
                        }
                    }
                }
            }
            '\\' => match chars.next() {
                Some('\n') => {}
                Some(e) => {
                    in_word = true;
                    cur.push(e);
                }
                None => return Err(anyhow!("dangling backslash at the end of: {input}")),
            },
            other => {
                in_word = true;
                cur.push(other);
            }
        }
    }
    if in_word {
        words.push(cur);
    }
    Ok(words)
}

// ─── CLI capability gate ──────────────────────────────────────────────
// Features that need a newer CLI (commit 1.6+, DNS aliases 1.6+, native CPU
// 1.7.1+, ...) are gated in the UI on the detected CLI version. The version
// is cached for the process lifetime; start/stop/restart of the services
// clears it so an upgrade followed by a services restart is picked up.
static CLI_VERSION: std::sync::RwLock<Option<String>> = std::sync::RwLock::new(None);

fn clear_cli_version_cache() {
    *CLI_VERSION.write().unwrap() = None;
}

/// Version of the CLI binary (`container --version`), e.g. `1.7.1`. `None`
/// when the binary can't be run or prints nothing parseable. Only successful
/// detections are cached.
pub async fn cli_version() -> Option<String> {
    if let Some(v) = CLI_VERSION.read().unwrap().clone() {
        return Some(v);
    }
    let bytes = run(&["--version"]).await.ok()?;
    let v = crate::updates::extract_version(&String::from_utf8_lossy(&bytes))?;
    *CLI_VERSION.write().unwrap() = Some(v.clone());
    Some(v)
}

// ─── Streaming spawners ───────────────────────────────────────────────
//
// Each returns a spawned `Child` and a stream-consumer task. The caller
// (commands.rs) merges stdout+stderr into per-line callbacks and emits Tauri
// events. Callers should hold onto the Child to abort with `start_kill()`.
pub fn spawn(args: &[&str]) -> Result<Child> {
    Command::new(bin())
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .with_context(|| format!("spawn `{} {}`", bin(), mask_env_args(args).join(" ")))
}

// Drain stdout+stderr line-by-line, invoking `on_line` for each. Returns when
// the child exits; the child handle is consumed.
pub async fn drain_lines<F>(mut child: Child, mut on_line: F) -> Result<std::process::ExitStatus>
where
    F: FnMut(String) + Send + 'static,
{
    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<String>();
    let tx_out = tx.clone();
    let t_out = tokio::spawn(async move {
        if let Some(out) = stdout {
            let mut lines = BufReader::new(out).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                let _ = tx_out.send(line);
            }
        }
    });
    let tx_err = tx.clone();
    let t_err = tokio::spawn(async move {
        if let Some(err) = stderr {
            let mut lines = BufReader::new(err).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                let _ = tx_err.send(line);
            }
        }
    });
    drop(tx);
    let pump = tokio::spawn(async move {
        while let Some(line) = rx.recv().await {
            on_line(line);
        }
    });
    let status = child.wait().await?;
    let _ = t_out.await;
    let _ = t_err.await;
    let _ = pump.await;
    Ok(status)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn patch_path_prepends_missing_dirs() {
        let bare = "/usr/bin:/bin"; // launchd default minus a couple
        let patched = patch_path(bare, &["/usr/local/bin", "/opt/homebrew/bin"]).unwrap();
        // Both prepended in declaration order, original PATH preserved at the tail.
        assert_eq!(patched, "/usr/local/bin:/opt/homebrew/bin:/usr/bin:/bin");
    }

    #[test]
    fn patch_path_skips_already_present_dirs() {
        let already = "/opt/homebrew/bin:/usr/local/bin:/usr/bin";
        // Both needed dirs are already there — no change required.
        assert_eq!(
            patch_path(already, &["/usr/local/bin", "/opt/homebrew/bin"]),
            None,
        );
    }

    #[test]
    fn patch_path_partial_overlap() {
        // Only /usr/local/bin is missing; /opt/homebrew/bin already present.
        let mixed = "/opt/homebrew/bin:/usr/bin";
        let patched = patch_path(mixed, &["/usr/local/bin", "/opt/homebrew/bin"]).unwrap();
        assert_eq!(patched, "/usr/local/bin:/opt/homebrew/bin:/usr/bin");
    }

    #[test]
    fn patch_path_handles_empty_current() {
        let patched = patch_path("", &["/usr/local/bin"]).unwrap();
        assert_eq!(patched, "/usr/local/bin");
    }

    fn words(s: &str) -> Vec<String> {
        split_command(s).unwrap()
    }

    #[test]
    fn split_command_plain_whitespace() {
        assert_eq!(
            words("node server.js --port 3000"),
            ["node", "server.js", "--port", "3000"]
        );
        assert_eq!(words("  a \t b\n c  "), ["a", "b", "c"]);
        assert!(words("").is_empty());
        assert!(words("   ").is_empty());
    }

    #[test]
    fn split_command_sh_c_with_quoted_script() {
        // The F18 case: previously split into `"echo`, `hi`, `&&`, ...
        assert_eq!(
            words(r#"sh -c "echo hi && sleep 5""#),
            ["sh", "-c", "echo hi && sleep 5"]
        );
        assert_eq!(
            words("sh -c 'echo hi && sleep 5'"),
            ["sh", "-c", "echo hi && sleep 5"]
        );
    }

    #[test]
    fn split_command_quote_rules() {
        // single quotes are literal, including backslashes and double quotes
        assert_eq!(words(r#"echo 'a "b" \n $x'"#), ["echo", r#"a "b" \n $x"#]);
        // double quotes: \" and \\ escape, other backslashes stay
        assert_eq!(
            words(r#"echo "say \"hi\" \\ \n""#),
            ["echo", r#"say "hi" \ \n"#]
        );
        // adjacent quoted/unquoted pieces join into one word
        assert_eq!(words(r#"a"b c"'d e'f"#), ["ab cd ef"]);
        // empty quotes make an empty word
        assert_eq!(words(r#"echo "" ''"#), ["echo", "", ""]);
        // backslash outside quotes escapes a space, a quote or a backslash
        assert_eq!(words(r"echo a\ b \'c \\"), ["echo", "a b", "'c", "\\"]);
        // line continuation disappears
        assert_eq!(words("a \\\nb"), ["a", "b"]);
        // a single quote inside double quotes is just a character
        assert_eq!(words(r#"echo "it's""#), ["echo", "it's"]);
    }

    #[test]
    fn split_command_unterminated_input_is_an_error() {
        for bad in [
            r#"sh -c "echo hi"#,
            "sh -c 'echo hi",
            "echo \\",
            r#"echo "abc\"#,
        ] {
            let e = split_command(bad).unwrap_err().to_string();
            assert!(
                e.contains("unterminated") || e.contains("dangling"),
                "{bad}: {e}"
            );
        }
    }

    #[test]
    fn mask_env_args_hides_values_but_keeps_everything_else() {
        let argv = [
            "run",
            "-d",
            "--name",
            "s_web",
            "-e",
            "DB_PASS=hunter2=x",
            "-e",
            "INHERIT",
            "--env",
            "A=1",
            "--env=B=2",
            "-p",
            "80:80",
            "img",
            "sh",
            "-e",
            "not-env",
        ];
        let masked = mask_env_args(&argv);
        assert_eq!(
            masked,
            [
                "run",
                "-d",
                "--name",
                "s_web",
                "-e",
                "DB_PASS=***",
                "-e",
                "INHERIT",
                "--env",
                "A=***",
                "--env=B=***",
                "-p",
                "80:80",
                "img",
                "sh",
                "-e",
                "not-env",
            ]
        );
        assert!(!masked.join(" ").contains("hunter2"));
    }

    #[test]
    fn reject_flag_like_rejects_empty_and_dash_prefixed() {
        assert!(reject_flag_like("container id", "").is_err());
        assert!(reject_flag_like("container id", "--all").is_err());
        assert!(reject_flag_like("container id", "-f").is_err());
        assert!(reject_flag_like("container id", "web").is_ok());
        assert!(reject_flag_like("image reference", "docker.io/library/alpine:3").is_ok());
        // A dash anywhere but the front is fine.
        assert!(reject_flag_like("container id", "my-app").is_ok());
        assert!(reject_flag_like("path", "./-name").is_ok());
        let e = reject_flag_like("output path", "-x")
            .unwrap_err()
            .to_string();
        assert!(e.contains("./"), "path errors should hint at ./: {e}");
        let e = reject_flag_like("container id", "-x")
            .unwrap_err()
            .to_string();
        assert!(!e.contains("./"), "non-path errors need no hint: {e}");
    }

    // Every runtime entry point taking a positional value must reject a
    // flag-like one *before* spawning anything (these run with no CLI).
    #[tokio::test]
    async fn runtime_entry_points_reject_flag_like_values() {
        let bad = "--all";
        assert!(inspect(bad).await.is_err());
        assert!(inspect_volume(bad).await.is_err());
        assert!(inspect_network(bad).await.is_err());
        assert!(inspect_image(bad).await.is_err());
        assert!(start(bad).await.is_err());
        assert!(stop(bad).await.is_err());
        assert!(kill(bad, None).await.is_err());
        assert!(kill("web", Some("-9")).await.is_err());
        assert!(delete(bad).await.is_err());
        assert!(restart(bad).await.is_err());
        assert!(clean_containers(&["ok1".into(), bad.into()]).await.is_err());
        assert!(delete_image(bad).await.is_err());
        assert!(tag_image(bad, "x:1").await.is_err());
        assert!(tag_image("x:1", bad).await.is_err());
        assert!(save_image(bad, "/tmp/o.tar").await.is_err());
        assert!(save_image("x:1", "-o").await.is_err());
        assert!(load_image(bad).await.is_err());
        assert!(copy_path(bad, "/tmp/x").await.is_err());
        assert!(copy_path("web:/etc/hosts", bad).await.is_err());
        assert!(export_container(bad, "/tmp/o.tar").await.is_err());
        assert!(export_container("web", bad).await.is_err());
        assert!(machine_create(
            "ok-machine",
            bad,
            None,
            None,
            true,
            &MachineUserOpts::default()
        )
        .await
        .is_err());
        assert!(machine_stop(bad).await.is_err());
        assert!(machine_delete(bad).await.is_err());
        assert!(machine_set_default(bad).await.is_err());
        assert!(registry_login(bad, "u", "p").await.is_err());
        assert!(registry_login("ghcr.io", bad, "p").await.is_err());
        assert!(registry_logout(bad).await.is_err());
        assert!(delete_volume(bad).await.is_err());
        assert!(delete_network(bad).await.is_err());
        assert!(exec_in_terminal(bad).is_err());
        let args = RunArgs {
            image: bad.into(),
            ..Default::default()
        };
        assert!(run_image(args).await.is_err());
    }

    // Exercise the streaming line pump end-to-end against a controlled
    // child. Captures the same path used by start_log_stream / start_pull.
    #[tokio::test]
    async fn drain_lines_collects_all_output() {
        let child = Command::new("sh")
            .args([
                "-c",
                "printf 'alpha\\nbravo\\ncharlie\\n'; printf 'errline\\n' >&2",
            ])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .expect("spawn sh");
        let collected = std::sync::Arc::new(std::sync::Mutex::new(Vec::<String>::new()));
        let sink = collected.clone();
        let status = drain_lines(child, move |line| sink.lock().unwrap().push(line))
            .await
            .expect("drain ok");
        assert!(status.success());
        let lines = collected.lock().unwrap().clone();
        assert!(
            lines.iter().any(|l| l == "alpha"),
            "missing stdout line: {lines:?}"
        );
        assert!(
            lines.iter().any(|l| l == "bravo"),
            "missing stdout line: {lines:?}"
        );
        assert!(
            lines.iter().any(|l| l == "charlie"),
            "missing stdout line: {lines:?}"
        );
        assert!(
            lines.iter().any(|l| l == "errline"),
            "missing stderr line: {lines:?}"
        );
    }

    #[test]
    fn validate_entity_name_accepts_63_chars() {
        let name = "a".repeat(63);
        assert!(validate_entity_name("container", &name).is_ok());
    }

    #[test]
    fn validate_entity_name_rejects_64_chars() {
        let name = "a".repeat(64);
        let err = validate_entity_name("container", &name)
            .unwrap_err()
            .to_string();
        assert!(err.contains("64 characters"));
        assert!(err.contains("at most 63"));
    }

    #[test]
    fn validate_entity_name_rejects_leading_underscore_dot_dash() {
        for bad in ["_foo", ".foo", "-foo"] {
            assert!(
                validate_entity_name("container", bad).is_err(),
                "expected {bad} to be rejected"
            );
        }
    }

    #[test]
    fn validate_entity_name_rejects_single_char() {
        assert!(validate_entity_name("container", "a").is_err());
    }

    #[test]
    fn validate_entity_name_rejects_disallowed_chars() {
        for bad in ["foo bar", "foo/bar", "foo@bar", "foo:bar"] {
            assert!(
                validate_entity_name("container", bad).is_err(),
                "expected {bad} to be rejected"
            );
        }
    }

    #[test]
    fn validate_entity_name_accepts_valid_names() {
        for good in ["ab", "web-1", "db_primary", "app.v2", "Container123"] {
            assert!(validate_entity_name("container", good).is_ok());
        }
    }
}
