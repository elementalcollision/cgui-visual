// Update checks. Hits GitHub Releases API for the components we track and
// compares against the locally-installed version. Uses curl so we don't pull
// reqwest+rustls into the build.
//
// 60 unauthenticated GitHub req/hr is plenty for interactive use; we do not
// cache. If that ever becomes a problem, write a 24h cache to
// ~/.config/cgui-gui/updates-cache.json.

use serde::Deserialize;
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

pub async fn check() -> Vec<Update> {
    let mut out = Vec::new();
    for c in COMPONENTS {
        // Skip silently when the companion isn't installed locally.
        // The in-app self-updater handles cgui-visual's own updates
        // separately; this list is purely for companions on the host.
        let Some(installed) = installed_version(c).await else {
            continue;
        };
        let Some(release) = fetch_release(c.repo).await else {
            continue;
        };
        let latest = strip_v_prefix(&release.tag_name);
        if installed == latest {
            continue;
        } // up to date
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
    out
}

/// Resolve the installed version of a component, or None when the
/// probe doesn't return something parseable as a version. Distinct
/// failure modes (binary missing, non-zero exit, garbled output) all
/// collapse into None — callers don't need to distinguish.
async fn installed_version(c: &Component) -> Option<String> {
    let (bin, args) = c.installed_version_cmd;
    let out = Command::new(bin).args(args).output().await.ok()?;
    if !out.status.success() {
        return None;
    }
    let s = String::from_utf8_lossy(&out.stdout);
    // Pull the first version-looking token out of the line. `--version`
    // output varies: "container CLI version 0.12.3 (build: release, ...)"
    // or just "0.13.0", etc.
    extract_version(&s)
}

fn extract_version(s: &str) -> Option<String> {
    s.split(|c: char| c.is_whitespace() || c == ',' || c == '(' || c == ')')
        .find(|tok| {
            let t = tok.trim_start_matches('v');
            !t.is_empty() && t.chars().next().is_some_and(|c| c.is_ascii_digit()) && t.contains('.')
        })
        .map(strip_v_prefix)
}

fn strip_v_prefix(s: &str) -> String {
    s.trim().trim_start_matches('v').to_string()
}

async fn fetch_release(repo: &str) -> Option<GhRelease> {
    let url = format!("https://api.github.com/repos/{repo}/releases/latest");
    let fut = Command::new("curl")
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
}
