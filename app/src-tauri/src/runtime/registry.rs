use super::*;

// ─── Registry logins (1.0 parity) ─────────────────────────────────────

#[derive(Debug, Default, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct RegistryLogin {
    pub hostname: String,
    pub username: String,
}

pub async fn registry_list() -> Result<Vec<RegistryLogin>> {
    let bytes = run(&["registry", "list", "--format", "json"]).await?;
    let raw: Vec<Value> = serde_json::from_slice(&bytes).context("parse `registry list` json")?;
    Ok(raw
        .into_iter()
        .map(|v| RegistryLogin {
            hostname: v
                .get("hostname")
                .or_else(|| v.get("server"))
                .and_then(Value::as_str)
                .unwrap_or("?")
                .to_string(),
            username: v
                .get("username")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string(),
        })
        .collect())
}

/// Log in to a registry. The password travels over the child's stdin
/// (`--password-stdin`) so it never appears in an argv that `ps` or
/// crash logs could capture. The CLI stores it in the user's keychain;
/// we never persist it.
pub async fn registry_login(server: &str, username: &str, password: &str) -> Result<()> {
    reject_flag_like("registry server", server)?;
    reject_flag_like("registry username", username)?;
    let mut child = Command::new(bin())
        .args([
            "registry",
            "login",
            "--password-stdin",
            "-u",
            username,
            server,
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .with_context(|| format!("failed to spawn `{} registry login`", bin()))?;
    {
        use tokio::io::AsyncWriteExt;
        let mut stdin = child.stdin.take().context("registry login stdin")?;
        stdin.write_all(password.as_bytes()).await?;
        // Drop closes the pipe so the CLI sees EOF and proceeds.
    }
    let out = tokio::time::timeout(RUN_TIMEOUT, child.wait_with_output())
        .await
        .map_err(|_| {
            anyhow!(
                "`registry login` timed out after {}s",
                RUN_TIMEOUT.as_secs()
            )
        })??;
    if !out.status.success() {
        return Err(anyhow!(
            "`registry login` exited {}: {}",
            out.status,
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    Ok(())
}

pub async fn registry_logout(server: &str) -> Result<()> {
    reject_flag_like("registry server", server)?;
    run(&["registry", "logout", server]).await.map(|_| ())
}
