use super::*;

// ─── Container machines (`container machine …`, new in 1.0) ──────────
// Machines are named VMs that containers run inside. `machine list
// --format json` returns a flat shape: {id, status, cpus, memory,
// diskSize, default, createdDate}.
#[derive(Debug, Default, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Machine {
    pub id: String,
    pub status: String,
    pub cpus: u32,
    pub memory: u64,
    pub disk_size: u64,
    #[serde(rename = "default")]
    pub is_default: bool,
    pub created_date: String,
}

pub async fn machine_list() -> Result<Vec<Machine>> {
    let bytes = run(&["machine", "list", "--format", "json"]).await?;
    serde_json::from_slice(&bytes).context("parse `machine list` json")
}

/// Optional account settings for the machine's user (container 1.6+):
/// `-u/--user`, `--uid`, `--gid`, `--home`. Unset fields keep the CLI
/// defaults (the host user's name/ids, home `/home/<user>`).
#[derive(Debug, Default, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MachineUserOpts {
    /// `name` or `uid[:gid]` (the CLI's `-u` format).
    #[serde(default)]
    pub user: Option<String>,
    #[serde(default)]
    pub uid: Option<u32>,
    #[serde(default)]
    pub gid: Option<u32>,
    /// Absolute home directory inside the machine.
    #[serde(default)]
    pub home: Option<String>,
}

/// Validate and render `MachineUserOpts` as CLI flags (empty when unset).
pub fn machine_user_args(opts: &MachineUserOpts) -> Result<Vec<String>> {
    let mut out: Vec<String> = Vec::new();
    let text = |o: &Option<String>| {
        o.as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(String::from)
    };
    if let Some(user) = text(&opts.user) {
        reject_flag_like("machine user", &user)?;
        if user.chars().any(|c| c.is_whitespace() || c.is_control()) {
            return Err(anyhow!("machine user `{user}` must not contain spaces"));
        }
        out.push("--user".into());
        out.push(user);
    }
    if let Some(uid) = opts.uid {
        out.push("--uid".into());
        out.push(uid.to_string());
    }
    if let Some(gid) = opts.gid {
        out.push("--gid".into());
        out.push(gid.to_string());
    }
    if let Some(home) = text(&opts.home) {
        if !home.starts_with('/') || home.chars().any(|c| c.is_control()) {
            return Err(anyhow!(
                "machine home `{home}` must be an absolute path such as /home/dev"
            ));
        }
        out.push("--home".into());
        out.push(home);
    }
    Ok(out)
}

/// Create a machine. Pulls the image on first use, so long timeout.
/// `no_boot` creates without booting (cheaper; boots on first use).
/// `user` carries the optional 1.6+ account flags.
pub async fn machine_create(
    name: &str,
    image: &str,
    cpus: Option<u32>,
    memory: Option<String>,
    no_boot: bool,
    user: &MachineUserOpts,
) -> Result<()> {
    validate_entity_name("machine", name)?;
    reject_flag_like("image reference", image)?;
    let mut argv: Vec<String> = vec![
        "machine".into(),
        "create".into(),
        "--name".into(),
        name.into(),
    ];
    if let Some(c) = cpus.filter(|c| *c > 0) {
        argv.push("--cpus".into());
        argv.push(c.to_string());
    }
    if let Some(m) = memory.filter(|m| !m.is_empty()) {
        argv.push("--memory".into());
        argv.push(m);
    }
    if no_boot {
        argv.push("--no-boot".into());
    }
    argv.extend(machine_user_args(user)?);
    argv.push(image.into());
    let argv_ref: Vec<&str> = argv.iter().map(String::as_str).collect();
    run_with_timeout(&argv_ref, RUN_LONG_TIMEOUT)
        .await
        .map(|_| ())
}

/// `container machine logs [--boot] [-n N] <name>`: a one-shot fetch (no
/// `--follow`). `boot` shows the VM boot log instead of stdio. `tail` limits
/// to the last N lines (clamped to 1..=5000); the CLI prints everything when
/// omitted, which can be huge, so callers pass one. A machine that has
/// never booted has no log file and the CLI says so.
pub async fn machine_logs(name: &str, boot: bool, tail: Option<u32>) -> Result<String> {
    reject_flag_like("machine name", name)?;
    let mut argv: Vec<String> = vec!["machine".into(), "logs".into()];
    if boot {
        argv.push("--boot".into());
    }
    if let Some(n) = tail {
        argv.push("-n".into());
        argv.push(n.clamp(1, 5000).to_string());
    }
    argv.push(name.into());
    let argv_ref: Vec<&str> = argv.iter().map(String::as_str).collect();
    let bytes = run_with_timeout(&argv_ref, LIFECYCLE_TIMEOUT).await?;
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

/// `container machine inspect <name>` as pretty JSON (the CLI already
/// pretty-prints it; non-JSON output is passed through).
pub async fn machine_inspect(name: &str) -> Result<String> {
    reject_flag_like("machine name", name)?;
    inspect_args(&["machine", "inspect", name]).await
}

pub async fn machine_stop(name: &str) -> Result<()> {
    reject_flag_like("machine name", name)?;
    run_with_timeout(&["machine", "stop", name], RUN_LONG_TIMEOUT)
        .await
        .map(|_| ())
}

pub async fn machine_delete(name: &str) -> Result<()> {
    reject_flag_like("machine name", name)?;
    run_with_timeout(&["machine", "delete", name], RUN_LONG_TIMEOUT)
        .await
        .map(|_| ())
}

pub async fn machine_set_default(name: &str) -> Result<()> {
    reject_flag_like("machine name", name)?;
    run(&["machine", "set-default", name]).await.map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn machine_list_parses_1_0_shape() {
        // Verbatim from `container machine list --format json` on 1.0.0.
        let json = r#"[{"status":"stopped","memory":2147483648,"default":true,"diskSize":78725120,"cpus":2,"createdDate":"2026-06-12T15:32:16Z","id":"cgui-m-test"}]"#;
        let ms: Vec<Machine> = serde_json::from_slice(json.as_bytes()).unwrap();
        assert_eq!(ms.len(), 1);
        assert_eq!(ms[0].id, "cgui-m-test");
        assert_eq!(ms[0].status, "stopped");
        assert_eq!(ms[0].cpus, 2);
        assert_eq!(ms[0].memory, 2147483648);
        assert!(ms[0].is_default);
    }
}
