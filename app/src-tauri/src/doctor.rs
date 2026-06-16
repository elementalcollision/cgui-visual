// Mirrors `cgui doctor` — health checks for the local environment.
// Returns the same shape the UI's DoctorModal already renders.

use std::path::PathBuf;
use tokio::process::Command;

use crate::model::{DoctorCheck, DoctorFix};

const RUNTIME_BIN: &str = "container";

pub async fn run() -> Vec<DoctorCheck> {
    let mut out: Vec<DoctorCheck> = Vec::new();

    // 1. Active profile (we hardcode `container` until the Settings picker
    // is wired through to the runtime selector).
    out.push(ok(format!("active profile: container → {RUNTIME_BIN}")));

    // 2. Runtime binary on PATH.
    match Command::new("which").arg(RUNTIME_BIN).output().await {
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
    match Command::new(RUNTIME_BIN).arg("--version").output().await {
        Ok(o) if o.status.success() => {
            let v = String::from_utf8_lossy(&o.stdout).trim().to_string();
            out.push(ok(format!("`{RUNTIME_BIN} --version` → {v}")));
        }
        _ => out.push(err(format!("`{RUNTIME_BIN} --version` failed"))),
    }

    // 4. system status (Apple-container-specific).
    match Command::new(RUNTIME_BIN)
        .args(["system", "status"])
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
    match Command::new("which").arg("trivy").output().await {
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

    out
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
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))?;
    Some(base.join("cgui").join(name))
}
