use super::*;

// Raw ls + stats; the merge happens in state.rs because CPU% needs deltas
// across polls, which require persistent state.
pub async fn list_containers_raw() -> Result<(Vec<Container>, Vec<StatRow>)> {
    let (ls_res, stats_res) =
        tokio::join!(run(&["ls", "--all", "--format", "json"]), stats_snapshot(),);
    let bytes = ls_res?;
    let raw: Vec<Value> = serde_json::from_slice(&bytes).context("parse `container ls` json")?;
    let cs: Vec<Container> = raw.into_iter().map(parse_container).collect();
    Ok((cs, stats_res.unwrap_or_default()))
}

pub async fn stats_snapshot() -> Result<Vec<StatRow>> {
    let bytes = run(&["stats", "--no-stream", "--format", "json"]).await?;
    let raw: Vec<Value> = serde_json::from_slice(&bytes).unwrap_or_default();
    Ok(raw
        .into_iter()
        .filter_map(|v| serde_json::from_value(v).ok())
        .collect())
}

/// Match a stats row to a container by exact id. `Container.id` is the full
/// CLI id and `container stats` reports the same id; prefix matching would
/// conflate containers that share a prefix (e.g. `stack_web` / `stack_worker`).
pub fn match_stat<'a>(stats: &'a [StatRow], id: &str) -> Option<&'a StatRow> {
    stats.iter().find(|s| s.id == id)
}

/// Display-only abbreviation of a container id (first 8 chars).
pub fn short_container_id(id: &str) -> String {
    id.chars().take(8).collect()
}

// ─── Inspect / actions ────────────────────────────────────────────────
pub async fn inspect(id: &str) -> Result<String> {
    reject_flag_like("container id", id)?;
    inspect_args(&["inspect", id]).await
}

/// Current run identity of one container, straight from `inspect`:
/// `(running, started_at)`. Used to confirm that a log read really belongs
/// to the run it was planned for (a container can restart between the poll
/// that planned the read and the read itself). Errors when the container no
/// longer exists.
pub async fn run_info(id: &str) -> Result<(bool, Option<i64>)> {
    reject_flag_like("container id", id)?;
    let bytes = run(&["inspect", id]).await?;
    let v: Value = serde_json::from_slice(&bytes)?;
    let first = v
        .as_array()
        .and_then(|a| a.first())
        .cloned()
        .ok_or_else(|| anyhow!("container {id} not found"))?;
    let c = parse_container(first);
    Ok((c.status.eq_ignore_ascii_case("running"), c.started_unix))
}

pub(super) async fn inspect_args(args: &[&str]) -> Result<String> {
    let bytes = run(args).await?;
    match serde_json::from_slice::<Value>(&bytes) {
        Ok(v) => Ok(serde_json::to_string_pretty(&v)
            .unwrap_or_else(|_| String::from_utf8_lossy(&bytes).into_owned())),
        Err(_) => Ok(String::from_utf8_lossy(&bytes).into_owned()),
    }
}

pub async fn start(id: &str) -> Result<()> {
    reject_flag_like("container id", id)?;
    run_with_timeout(&["start", id], LIFECYCLE_TIMEOUT)
        .await
        .map(|_| ())
}

pub async fn stop(id: &str) -> Result<()> {
    reject_flag_like("container id", id)?;
    run_with_timeout(&["stop", id], LIFECYCLE_TIMEOUT)
        .await
        .map(|_| ())
}

/// Kill with an optional signal (`container kill -s <signal> <id>`).
/// `None` keeps the CLI's default (SIGKILL).
pub async fn kill(id: &str, signal: Option<&str>) -> Result<()> {
    reject_flag_like("container id", id)?;
    if let Some(sig) = signal.filter(|s| !s.is_empty()) {
        reject_flag_like("signal", sig)?;
    }
    match signal.filter(|s| !s.is_empty()) {
        Some(sig) => run_with_timeout(&["kill", "-s", sig, id], LIFECYCLE_TIMEOUT)
            .await
            .map(|_| ()),
        None => run_with_timeout(&["kill", id], LIFECYCLE_TIMEOUT)
            .await
            .map(|_| ()),
    }
}

pub async fn delete(id: &str) -> Result<()> {
    reject_flag_like("container id", id)?;
    run_with_timeout(&["delete", id], LIFECYCLE_TIMEOUT)
        .await
        .map(|_| ())
}

pub async fn restart(id: &str) -> Result<()> {
    reject_flag_like("container id", id)?;
    if let Err(e) = run_with_timeout(&["stop", id], LIFECYCLE_TIMEOUT).await {
        // Only "already stopped" is expected here; anything else (services
        // down, timeout, unknown id) must reach the user instead of being
        // masked by a confusing follow-on `start` error.
        if !is_not_running_error(&format!("{e:#}")) {
            return Err(e.context(format!("could not stop `{id}` before restarting")));
        }
    }
    run_with_timeout(&["start", id], LIFECYCLE_TIMEOUT)
        .await
        .map(|_| ())
}

// Args used to build a `container run -d` invocation. All optional except the
// image reference. Matches the fields the UI's RunImageModal collects;
// the advanced block (resources, mounts, identity, platform) landed
// with 1.0 parity phase 4.
#[derive(Debug, Default, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunArgs {
    pub image: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub ports: Vec<String>, // "host:container" each
    #[serde(default)]
    pub env: Vec<String>, // "KEY=value" each
    #[serde(default)]
    pub command: Option<String>, // free-form; shell-style splitting (split_command)
    #[serde(default)]
    pub cpus: Option<u32>,
    #[serde(default)]
    pub memory: Option<String>, // e.g. "2G"
    #[serde(default)]
    pub volumes: Vec<String>, // "name-or-path:containerpath" each
    #[serde(default)]
    pub labels: Vec<String>, // "key=value" each
    #[serde(default)]
    pub network: Option<String>,
    /// DNS aliases for the container on `network` (container 1.6+):
    /// emitted as `--network <net>,alias=A,alias=B`. See `network_spec`.
    #[serde(default)]
    pub aliases: Vec<String>,
    #[serde(default)]
    pub workdir: Option<String>,
    #[serde(default)]
    pub user: Option<String>,
    #[serde(default)]
    pub platform: Option<String>, // "os/arch[/variant]"
    #[serde(default)]
    pub rm: bool,
    #[serde(default)]
    pub rosetta: bool,
}

/// Validate a network DNS alias (container 1.6+, `--network net,alias=NAME`):
/// a hostname of 1-63 character DNS labels (letters, digits, inner `-`),
/// at most 253 characters in all. The CLI additionally wants a fully
/// qualified name (two or more labels, e.g. `api.example.test`) unless a
/// default DNS domain is configured, and says so itself; we don't duplicate
/// that rule because the configured domain isn't known here.
///
/// This also guarantees an alias can't smuggle `,` / `=` into the
/// `--network` option list.
pub fn validate_alias(alias: &str) -> Result<()> {
    if alias.is_empty() {
        return Err(anyhow!("network alias must not be empty"));
    }
    if alias.len() > 253 {
        return Err(anyhow!(
            "network alias `{alias}` is longer than 253 characters"
        ));
    }
    let name = alias.strip_suffix('.').unwrap_or(alias);
    for label in name.split('.') {
        let ok = (1..=63).contains(&label.len())
            && label.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
            && !label.starts_with('-')
            && !label.ends_with('-');
        if !ok {
            return Err(anyhow!(
                "network alias `{alias}` is not a valid DNS name: each part must be 1-63 letters, digits or `-`, not starting or ending with `-`"
            ));
        }
    }
    Ok(())
}

/// The value for `--network`: the network name (or `default` when only
/// aliases were given) followed by `,alias=NAME` per alias. `None` when
/// there is neither a network nor an alias, so the CLI default applies.
/// Aliases must already be validated (`validate_alias`).
pub fn network_spec(network: Option<&str>, aliases: &[String]) -> Option<String> {
    let net = network.filter(|s| !s.is_empty());
    let aliases: Vec<&String> = aliases.iter().filter(|a| !a.is_empty()).collect();
    if aliases.is_empty() {
        return net.map(String::from);
    }
    let mut spec = net.unwrap_or("default").to_string();
    for a in aliases {
        spec.push_str(",alias=");
        spec.push_str(a);
    }
    Some(spec)
}

pub fn run_image_argv(args: &RunArgs) -> Result<Vec<String>> {
    for a in args.aliases.iter().filter(|a| !a.is_empty()) {
        validate_alias(a)?;
    }
    let mut argv: Vec<String> = vec!["run".into(), "-d".into()];
    let mut opt = |flag: &str, val: Option<&str>| {
        if let Some(v) = val.filter(|s| !s.is_empty()) {
            argv.push(flag.into());
            argv.push(v.into());
        }
    };
    opt("--name", args.name.as_deref());
    opt("--cpus", args.cpus.map(|c| c.to_string()).as_deref());
    opt("--memory", args.memory.as_deref());
    opt(
        "--network",
        network_spec(args.network.as_deref(), &args.aliases).as_deref(),
    );
    opt("--workdir", args.workdir.as_deref());
    opt("--user", args.user.as_deref());
    opt("--platform", args.platform.as_deref());
    // Re-borrow argv after the closure (NLL ends its borrow above).
    for p in args.ports.iter().filter(|s| !s.is_empty()) {
        argv.push("-p".into());
        argv.push(p.clone());
    }
    for e in args.env.iter().filter(|s| !s.is_empty()) {
        argv.push("-e".into());
        argv.push(e.clone());
    }
    for v in args.volumes.iter().filter(|s| !s.is_empty()) {
        argv.push("--volume".into());
        argv.push(v.clone());
    }
    for l in args.labels.iter().filter(|s| !s.is_empty()) {
        argv.push("--label".into());
        argv.push(l.clone());
    }
    if args.rm {
        argv.push("--rm".into());
    }
    if args.rosetta {
        argv.push("--rosetta".into());
    }
    argv.push(args.image.clone());
    if let Some(cmd) = args.command.as_deref().filter(|s| !s.is_empty()) {
        argv.extend(split_command(cmd).context("could not parse the command")?);
    }
    Ok(argv)
}

pub async fn run_image(args: RunArgs) -> Result<String> {
    reject_flag_like("image reference", &args.image)?;
    if let Some(name) = args.name.as_deref().filter(|s| !s.is_empty()) {
        validate_entity_name("container", name)?;
    }
    let argv = run_image_argv(&args)?;
    let argv_ref: Vec<&str> = argv.iter().map(String::as_str).collect();
    // First-run image pulls + VM provisioning blow past the default
    // inspect-sized timeout; use the long envelope so the modal isn't
    // killed out from under the user mid-pull.
    let bytes = run_with_timeout(&argv_ref, RUN_LONG_TIMEOUT).await?;
    Ok(String::from_utf8_lossy(&bytes).trim().to_string())
}

// ─── Container filesystem: copy / export (1.0 parity) ─────────────────
/// `container copy <src> <dst>` — either side may be `<id>:<path>`.
pub async fn copy_path(src: &str, dst: &str) -> Result<()> {
    reject_flag_like("source path", src)?;
    reject_flag_like("destination path", dst)?;
    run_with_timeout(&["copy", src, dst], RUN_LONG_TIMEOUT)
        .await
        .map(|_| ())
}

pub async fn export_container(id: &str, output: &str) -> Result<()> {
    reject_flag_like("container id", id)?;
    reject_flag_like("output path", output)?;
    run_with_timeout(&["export", "-o", output, id], RUN_LONG_TIMEOUT)
        .await
        .map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn run_image_argv_keeps_quoted_command_as_single_args() {
        let args = RunArgs {
            image: "alpine".into(),
            command: Some(r#"sh -c "echo hi && sleep 5""#.into()),
            ..Default::default()
        };
        let argv = run_image_argv(&args).unwrap();
        assert_eq!(
            &argv[argv.len() - 4..],
            ["alpine", "sh", "-c", "echo hi && sleep 5"]
        );
        let bad = RunArgs {
            image: "alpine".into(),
            command: Some("sh -c \"oops".into()),
            ..Default::default()
        };
        let e = format!("{:#}", run_image_argv(&bad).unwrap_err());
        assert!(e.contains("unterminated"), "{e}");
    }

    #[test]
    fn run_image_argv_full_surface() {
        let args = RunArgs {
            image: "alpine:latest".into(),
            name: Some("web".into()),
            ports: vec!["8080:80".into()],
            env: vec!["A=1".into(), "".into()],
            command: Some("sh -c true".into()),
            cpus: Some(2),
            memory: Some("2G".into()),
            volumes: vec!["data:/data".into()],
            labels: vec!["team=infra".into()],
            network: Some("default".into()),
            aliases: vec![],
            workdir: Some("/srv".into()),
            user: Some("1000:1000".into()),
            platform: Some("linux/arm64".into()),
            rm: true,
            rosetta: false,
        };
        let argv = run_image_argv(&args).unwrap();
        let s = argv.join(" ");
        assert!(s.starts_with("run -d "));
        for frag in [
            "--name web",
            "--cpus 2",
            "--memory 2G",
            "--network default",
            "--workdir /srv",
            "--user 1000:1000",
            "--platform linux/arm64",
            "-p 8080:80",
            "-e A=1",
            "--volume data:/data",
            "--label team=infra",
            "--rm",
        ] {
            assert!(s.contains(frag), "missing {frag} in {s}");
        }
        assert!(!s.contains("--rosetta"));
        // Image before the command tokens, command split on whitespace.
        assert!(s.ends_with("alpine:latest sh -c true"));
        // Empty env entry filtered.
        assert_eq!(argv.iter().filter(|a| *a == "-e").count(), 1);
    }

    #[test]
    fn run_image_argv_minimal() {
        let args = RunArgs {
            image: "redis:7".into(),
            ..Default::default()
        };
        assert_eq!(run_image_argv(&args).unwrap(), vec!["run", "-d", "redis:7"]);
    }

    #[test]
    fn match_stat_is_exact_not_prefix() {
        let stat = |id: &str| StatRow {
            id: id.into(),
            ..Default::default()
        };
        let stats = vec![stat("stack_web"), stat("stack_worker")];
        assert_eq!(
            match_stat(&stats, "stack_worker").unwrap().id,
            "stack_worker"
        );
        assert_eq!(match_stat(&stats, "stack_web").unwrap().id, "stack_web");
        assert!(match_stat(&stats, "stack_").is_none());
        assert!(match_stat(&stats, "stack_web_extra").is_none());
    }
}
