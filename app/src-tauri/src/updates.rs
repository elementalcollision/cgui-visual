// Update checks. Hits GitHub Releases API for the components we track and
// compares against the locally-installed version. Uses curl so we don't pull
// reqwest+rustls into the build.
//
// 60 unauthenticated GitHub req/hr is plenty for interactive use; we do not
// cache. If that ever becomes a problem, write a 24h cache to
// ~/.config/cgui-gui/updates-cache.json.

use serde::Deserialize;
use std::cmp::Ordering;
use std::time::Duration;
use tokio::process::Command;

use crate::model::Update;

const CHECK_TIMEOUT: Duration = Duration::from_secs(6);

#[derive(Debug, Deserialize)]
struct GhRelease {
    tag_name: String,
    #[serde(default)]
    body: String,
    #[serde(default)]
    published_at: String,
}

struct Component {
    /// Display label shown in the UpdateModal. Phrased so users don't
    /// confuse a *companion-component* update with a cgui-visual
    /// self-update — the latter goes through tauri-plugin-updater
    /// + the latest.json manifest, not this code path.
    label: &'static str,
    repo: &'static str,
    /// Local probe used to detect the installed version. The first
    /// member is the binary to spawn; the second is the argv tail.
    /// Components whose probe doesn't return a parseable version are
    /// skipped entirely — surfacing `? → X` made users think the GUI
    /// itself was out of date.
    installed_version_cmd: (&'static str, &'static [&'static str]),
}

const COMPONENTS: &[Component] = &[
    Component {
        label: "Apple container CLI",
        repo: "apple/container",
        installed_version_cmd: ("container", &["--version"]),
    },
    Component {
        label: "cgui TUI (companion)",
        repo: "elementalcollision/cgui",
        installed_version_cmd: ("cgui", &["--version"]),
    },
];

/// An empty list means "up to date". When every release lookup failed
/// (offline, rate-limited) that's an error, not "up to date".
pub async fn check() -> Result<Vec<Update>, String> {
    let mut out = Vec::new();
    let mut looked_up = 0usize;
    let mut failed = 0usize;
    for c in COMPONENTS {
        // Skip silently when the companion isn't installed locally.
        // The in-app self-updater handles cgui-visual's own updates
        // separately; this list is purely for companions on the host.
        let Some(installed) = installed_version(c).await else {
            continue;
        };
        looked_up += 1;
        let Some(release) = fetch_release(c.repo).await else {
            failed += 1;
            continue;
        };
        let latest = strip_v_prefix(&release.tag_name);
        // Up to date, or a newer local build than the latest release.
        if !update_available(&installed, &latest) {
            continue;
        }
        out.push(Update {
            component: c.label.to_string(),
            installed,
            latest: latest.clone(),
            published: release.published_at,
            notes: trim_body(&release.body),
            // Deep-link to the specific tag, not just /releases, so the
            // user lands on the notes for the version we're advertising.
            url: format!("https://github.com/{}/releases/tag/v{latest}", c.repo),
        });
    }
    lookup_verdict(looked_up, failed)?;
    Ok(out)
}

fn lookup_verdict(looked_up: usize, failed: usize) -> Result<(), String> {
    if looked_up > 0 && failed == looked_up {
        return Err(
            "couldn't reach GitHub to check for updates (offline or rate-limited)".to_string(),
        );
    }
    Ok(())
}

/// Resolve the installed version of a component, or None when the
/// probe doesn't return something parseable as a version. Distinct
/// failure modes (binary missing, non-zero exit, garbled output) all
/// collapse into None — callers don't need to distinguish.
async fn installed_version(c: &Component) -> Option<String> {
    let (bin, args) = c.installed_version_cmd;
    let out = Command::new(bin)
        .kill_on_drop(true)
        .args(args)
        .output()
        .await
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let s = String::from_utf8_lossy(&out.stdout);
    // Pull the first version-looking token out of the line. `--version`
    // output varies: "container CLI version 0.12.3 (build: release, ...)"
    // or just "0.13.0", etc.
    extract_version(&s)
}

pub(crate) fn extract_version(s: &str) -> Option<String> {
    s.split(|c: char| c.is_whitespace() || c == ',' || c == '(' || c == ')')
        .find(|tok| {
            let t = tok.trim_start_matches('v');
            !t.is_empty() && t.chars().next().is_some_and(|c| c.is_ascii_digit()) && t.contains('.')
        })
        .map(strip_v_prefix)
}

/// True when `latest` is strictly newer than `installed`. If either
/// string isn't a parseable version, any difference counts as an update.
fn update_available(installed: &str, latest: &str) -> bool {
    match version_cmp(installed, latest) {
        Some(ord) => ord == Ordering::Less,
        None => installed != latest,
    }
}

/// Numeric components of an `x.y.z` version plus whether it carries a
/// pre-release suffix (`-rc1`). Build metadata (`+abc`) is ignored.
fn parse_version(s: &str) -> Option<(Vec<u64>, bool)> {
    let s = s.trim().trim_start_matches('v');
    let s = s.split('+').next().unwrap_or(s);
    let (core, pre) = match s.split_once('-') {
        Some((c, _)) => (c, true),
        None => (s, false),
    };
    let parts: Option<Vec<u64>> = core.split('.').map(|p| p.parse().ok()).collect();
    let parts = parts.filter(|p| !p.is_empty())?;
    Some((parts, pre))
}

/// Numeric semver-style comparison (`1.10.0 > 1.7.1`). Missing trailing
/// components count as 0; a pre-release sorts before its release.
/// `None` when either side isn't a parseable version.
pub(crate) fn version_cmp(a: &str, b: &str) -> Option<Ordering> {
    let (mut pa, pre_a) = parse_version(a)?;
    let (mut pb, pre_b) = parse_version(b)?;
    let n = pa.len().max(pb.len());
    pa.resize(n, 0);
    pb.resize(n, 0);
    Some(pa.cmp(&pb).then(pre_b.cmp(&pre_a)))
}

fn strip_v_prefix(s: &str) -> String {
    s.trim().trim_start_matches('v').to_string()
}

async fn fetch_release(repo: &str) -> Option<GhRelease> {
    let url = format!("https://api.github.com/repos/{repo}/releases/latest");
    let fut = Command::new("/usr/bin/curl")
        .kill_on_drop(true)
        .args([
            "-sf",
            "-A",
            "cgui-gui",
            "-H",
            "Accept: application/vnd.github+json",
            &url,
        ])
        .output();
    let out = tokio::time::timeout(CHECK_TIMEOUT, fut).await.ok()?.ok()?;
    if !out.status.success() {
        return None;
    }
    serde_json::from_slice(&out.stdout).ok()
}

fn trim_body(body: &str) -> String {
    // Cap to ~6KB so we don't blast the modal with huge release notes.
    if body.len() <= 6000 {
        body.to_string()
    } else {
        let mut s: String = body.chars().take(6000).collect();
        s.push_str("\n\n[…notes truncated…]");
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_version_finds_first_dotted_token() {
        assert_eq!(
            extract_version("container CLI version 0.12.3 (build: release)").as_deref(),
            Some("0.12.3")
        );
        assert_eq!(extract_version("v0.13.0").as_deref(), Some("0.13.0"));
        assert_eq!(extract_version("cgui 1.4.5\n").as_deref(), Some("1.4.5"));
        assert_eq!(extract_version("no version here"), None);
    }

    #[test]
    fn version_cmp_is_numeric() {
        use Ordering::*;
        assert_eq!(version_cmp("1.4.0", "1.4.1"), Some(Less));
        assert_eq!(version_cmp("1.4.1", "1.7.1"), Some(Less));
        assert_eq!(version_cmp("1.7.1", "1.10.0"), Some(Less));
        assert_eq!(version_cmp("1.10.0", "1.7.1"), Some(Greater));
        assert_eq!(version_cmp("1.7.1", "v1.7.1"), Some(Equal));
        assert_eq!(version_cmp("1.7", "1.7.0"), Some(Equal));
        assert_eq!(version_cmp("1.8.0-rc1", "1.8.0"), Some(Less));
        assert_eq!(version_cmp("1.7.1+build5", "1.7.1"), Some(Equal));
        assert_eq!(version_cmp("garbage", "1.7.1"), None);
    }

    #[test]
    fn version_cmp_works_on_extracted_cli_output() {
        let v = extract_version("container CLI version 1.7.1 (build: release, commit: a15b460)")
            .unwrap();
        assert_eq!(v, "1.7.1");
        assert_eq!(version_cmp(&v, "1.4.1"), Some(Ordering::Greater));
        assert_eq!(version_cmp(&v, "1.7.1"), Some(Ordering::Equal));
    }

    #[test]
    fn newer_local_build_is_not_an_update() {
        assert!(!update_available("1.8.0", "1.7.1"));
        assert!(!update_available("1.7.1", "1.7.1"));
        assert!(update_available("1.7.1", "1.10.0"));
        // Unparseable on either side: fall back to "different means update".
        assert!(update_available("nightly", "1.7.1"));
    }

    #[test]
    fn lookup_verdict_only_errors_when_every_lookup_failed() {
        assert!(lookup_verdict(0, 0).is_ok()); // nothing installed
        assert!(lookup_verdict(2, 0).is_ok());
        assert!(lookup_verdict(2, 1).is_ok()); // partial: report what we have
        assert!(lookup_verdict(2, 2).is_err());
        assert!(lookup_verdict(1, 1).is_err());
    }

    #[test]
    fn strip_v_prefix_works() {
        assert_eq!(strip_v_prefix("v1.2.3"), "1.2.3");
        assert_eq!(strip_v_prefix(" 1.2.3 "), "1.2.3");
        // Multi-v tags shouldn't appear in real GitHub releases, but document
        // that we strip all leading 'v' chars.
        assert_eq!(strip_v_prefix("vvX"), "X");
    }

    #[test]
    fn trim_body_caps_long_notes() {
        let huge = "x".repeat(8000);
        let trimmed = trim_body(&huge);
        assert!(trimmed.len() < 6500);
        assert!(trimmed.ends_with("[…notes truncated…]"));
    }

    /// The app's own updates come from the tauri-plugin-updater endpoints in
    /// `tauri.conf.json`. The second address is the bridge for renaming the
    /// repository to `quayside` (a GitHub Pages address follows the repo's
    /// name; the plugin tries the addresses in order and skips one that
    /// fails). Copies that lack it stop getting updates after the rename, so
    /// it must ship, and stay, until every copy in use has it.
    #[test]
    fn updater_lists_current_then_post_rename_address() {
        let conf: serde_json::Value =
            serde_json::from_str(include_str!("../tauri.conf.json")).unwrap();
        let endpoints: Vec<&str> = conf["plugins"]["updater"]["endpoints"]
            .as_array()
            .expect("updater endpoints")
            .iter()
            .map(|v| v.as_str().unwrap())
            .collect();
        assert_eq!(
            endpoints,
            [
                "https://elementalcollision.github.io/cgui-visual/updates/latest.json",
                "https://elementalcollision.github.io/quayside/updates/latest.json",
            ]
        );
    }
}
