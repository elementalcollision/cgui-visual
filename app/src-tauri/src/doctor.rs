// Mirrors `cgui doctor` — health checks for the local environment.
// Returns the same shape the UI's DoctorModal already renders.

use std::path::PathBuf;
use tokio::process::Command;

use crate::model::{DoctorCheck, DoctorFix};

const RUNTIME_BIN: &str = "container";

/// Oldest Apple container release cgui supports.
const MIN_CONTAINER_VERSION: &str = "1.4.1";

pub async fn run() -> Vec<DoctorCheck> {
    let mut out: Vec<DoctorCheck> = Vec::new();

    // 1. Active profile (always `container`; cgui is Apple-container-only).
    out.push(ok(format!("active profile: container → {RUNTIME_BIN}")));

    // 2. Runtime binary on PATH.
    match Command::new("/usr/bin/which")
        .kill_on_drop(true)
        .arg(RUNTIME_BIN)
        .output()
        .await
    {
        Ok(o) if o.status.success() => {
            let path = String::from_utf8_lossy(&o.stdout).trim().to_string();
            out.push(ok(format!("`{RUNTIME_BIN}` resolves to {path}")));
        }
        _ => {
            let mut c = err(format!("`{RUNTIME_BIN}` not on PATH"));
            c.fix = Some(DoctorFix::Url {
                label: "Open Apple container releases".into(),
                url: "https://github.com/apple/container/releases".into(),
            });
            out.push(c);
        }
    }

    // 3. Runtime --version.
    match Command::new(RUNTIME_BIN)
        .kill_on_drop(true)
        .arg("--version")
        .output()
        .await
    {
        Ok(o) if o.status.success() => {
            let v = String::from_utf8_lossy(&o.stdout).trim().to_string();
            out.push(ok(format!("`{RUNTIME_BIN} --version` → {v}")));
            if let Some(c) = min_version_check(crate::updates::extract_version(&v).as_deref()) {
                out.push(c);
            }
        }
        _ => out.push(err(format!("`{RUNTIME_BIN} --version` failed"))),
    }

    // 4. system status (Apple-container-specific). `--format json` gives
    // `parse_system_running` its top-level `status` field (1.4.1+, the
    // minimum supported version).
    match Command::new(RUNTIME_BIN)
        .kill_on_drop(true)
        .args(["system", "status", "--format", "json"])
        .output()
        .await
    {
        Ok(o) if o.status.success() => {
            let body = String::from_utf8_lossy(&o.stdout);
            // Precise match: a naive contains("running") also matches the
            // stopped message "apiserver is not running", which would hide
            // the Start-services fix exactly when it's needed.
            if crate::runtime::parse_system_running(&body) {
                out.push(ok("container system status: running".into()));
                let (client, server) = crate::runtime::parse_system_versions(&body);
                if let Some(c) = version_mismatch_check(client.as_deref(), server.as_deref()) {
                    out.push(c);
                }
            } else {
                let mut c = warn("container system status not running".into());
                c.fix = Some(DoctorFix::Run {
                    label: "Start services".into(),
                    action: "system-start".into(),
                });
                out.push(c);
            }
        }
        _ => {
            let mut c = warn("could not query `container system status`".into());
            c.fix = Some(DoctorFix::Run {
                label: "Start services".into(),
                action: "system-start".into(),
            });
            out.push(c);
        }
    }

    // 5. profiles.toml present.
    match config_path("profiles.toml") {
        Some(p) if p.exists() => out.push(ok(format!("profiles.toml at {} present", p.display()))),
        Some(p) => out.push(warn(format!(
            "no profiles.toml at {} (using built-in default)",
            p.display()
        ))),
        None => out.push(warn(
            "no $XDG_CONFIG_HOME or $HOME — skipping config checks".into(),
        )),
    }

    // 6. state.json parses.
    if let Some(p) = config_path("state.json") {
        if p.exists() {
            match std::fs::read_to_string(&p) {
                Ok(s) => match serde_json::from_str::<serde_json::Value>(&s) {
                    Ok(_) => out.push(ok(format!("state.json at {} parses cleanly", p.display()))),
                    Err(e) => out.push(err(format!(
                        "state.json at {} failed to parse: {e}",
                        p.display()
                    ))),
                },
                Err(e) => out.push(warn(format!("could not read state.json: {e}"))),
            }
        }
    }

    // 7. trivy on PATH (optional).
    match Command::new("/usr/bin/which")
        .kill_on_drop(true)
        .arg("trivy")
        .output()
        .await
    {
        Ok(o) if o.status.success() => {
            let path = String::from_utf8_lossy(&o.stdout).trim().to_string();
            out.push(ok(format!("trivy: {path} (image scan available)")));
        }
        _ => {
            let mut c =
                warn("trivy not on PATH (image scan disabled — `brew install trivy`)".into());
            c.fix = Some(DoctorFix::Copy {
                label: "Copy `brew install trivy`".into(),
                command: "brew install trivy".into(),
            });
            out.push(c);
        }
    }

    // 8. runtime API socket.
    let sock = "/var/run/container.sock";
    if std::path::Path::new(sock).exists() {
        out.push(ok(format!("runtime API reachable on {sock}")));
    } else {
        out.push(warn(format!("runtime API socket not found at {sock}")));
    }

    // 9. background log capture (see logpersist.rs).
    out.push(log_capture_check(crate::logpersist::status()));

    out
}

fn log_capture_check(s: crate::logpersist::Status) -> DoctorCheck {
    if !s.enabled {
        return ok("background log capture: off (Settings → Save container logs)".into());
    }
    ok(format!(
        "background log capture: {} container{} followed",
        s.followers,
        if s.followers == 1 { "" } else { "s" }
    ))
}

/// Error check when the CLI is older than the supported minimum. None when
/// it's fine (or the version couldn't be parsed; the `--version` row
/// already shows the raw output).
fn min_version_check(cli: Option<&str>) -> Option<DoctorCheck> {
    let cli = cli?;
    if crate::updates::version_cmp(cli, MIN_CONTAINER_VERSION) != Some(std::cmp::Ordering::Less) {
        return None;
    }
    let mut c = err(format!(
        "cgui needs Apple container {MIN_CONTAINER_VERSION} or later (found {cli})"
    ));
    c.fix = Some(DoctorFix::Url {
        label: "Open Apple container releases".into(),
        url: "https://github.com/apple/container/releases".into(),
    });
    Some(c)
}

/// Warning check when the CLI and the running apiserver differ (a stale
/// server keeps running after a pkg upgrade until the services restart).
/// None when they match or either version is unknown.
fn version_mismatch_check(client: Option<&str>, server: Option<&str>) -> Option<DoctorCheck> {
    let (client, server) = (client?, server?);
    let differ = match crate::updates::version_cmp(client, server) {
        Some(ord) => ord != std::cmp::Ordering::Equal,
        None => client != server,
    };
    if !differ {
        return None;
    }
    let mut c = warn(format!(
        "container CLI {client} is talking to a {server} server — restart the services"
    ));
    c.fix = Some(DoctorFix::Run {
        label: "Restart services".into(),
        action: "system-restart".into(),
    });
    Some(c)
}

fn ok(text: String) -> DoctorCheck {
    DoctorCheck {
        ok: true,
        text,
        warn: None,
        fix: None,
    }
}
fn warn(text: String) -> DoctorCheck {
    DoctorCheck {
        ok: false,
        text,
        warn: Some(true),
        fix: None,
    }
}
fn err(text: String) -> DoctorCheck {
    DoctorCheck {
        ok: false,
        text,
        warn: Some(false),
        fix: None,
    }
}

fn config_path(name: &str) -> Option<PathBuf> {
    Some(crate::paths::config_dir("cgui")?.join(name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn log_capture_line_reports_follower_count_or_off() {
        use crate::logpersist::Status;
        let s = |enabled, followers| Status {
            enabled,
            followers,
            catching_up: 0,
        };
        assert_eq!(
            log_capture_check(s(true, 1)).text,
            "background log capture: 1 container followed"
        );
        assert_eq!(
            log_capture_check(s(true, 3)).text,
            "background log capture: 3 containers followed"
        );
        assert!(log_capture_check(s(false, 0)).text.contains("off"));
    }

    #[test]
    fn mismatch_warns_with_restart_fix() {
        let c = version_mismatch_check(Some("1.7.1"), Some("1.4.1")).unwrap();
        assert!(!c.ok);
        assert_eq!(c.warn, Some(true));
        assert_eq!(
            c.text,
            "container CLI 1.7.1 is talking to a 1.4.1 server — restart the services"
        );
        match c.fix {
            Some(DoctorFix::Run { action, .. }) => assert_eq!(action, "system-restart"),
            other => panic!("unexpected fix: {other:?}"),
        }
    }

    #[test]
    fn matching_or_unknown_versions_do_not_warn() {
        assert!(version_mismatch_check(Some("1.7.1"), Some("1.7.1")).is_none());
        assert!(version_mismatch_check(Some("1.7.1"), Some("v1.7.1")).is_none());
        assert!(version_mismatch_check(Some("1.7.1"), None).is_none());
        assert!(version_mismatch_check(None, Some("1.7.1")).is_none());
    }

    #[test]
    fn min_version_errors_below_1_4_1_only() {
        let c = min_version_check(Some("1.4.0")).unwrap();
        assert_eq!(c.warn, Some(false));
        assert!(c
            .text
            .starts_with("cgui needs Apple container 1.4.1 or later"));
        assert!(min_version_check(Some("0.12.3")).is_some());
        assert!(min_version_check(Some("1.4.1")).is_none());
        assert!(min_version_check(Some("1.10.0")).is_none());
        assert!(min_version_check(None).is_none());
    }
}
