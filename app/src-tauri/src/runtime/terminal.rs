use super::*;

// Open a Terminal.app window running `<runtime> exec -it <id> /bin/sh`.
// macOS-only because the host project (Apple's `container` runtime) is
// macOS-only.
pub fn exec_in_terminal(id: &str) -> Result<()> {
    reject_flag_like("container id", id)?;
    let runtime_bin = bin();
    let cmd_line = format!(
        "{} exec -it {} /bin/sh",
        shell_quote(runtime_bin),
        shell_quote(id)
    );
    let script = format!(
        r#"tell application "Terminal" to do script "{}""#,
        cmd_line.replace('\\', "\\\\").replace('"', "\\\"")
    );
    std::process::Command::new("/usr/bin/osascript")
        .args(["-e", &script])
        .spawn()
        .with_context(|| "failed to spawn osascript")?;
    let _ = std::process::Command::new("/usr/bin/osascript")
        .args(["-e", r#"tell application "Terminal" to activate"#])
        .status();
    Ok(())
}

fn shell_quote(s: &str) -> String {
    if s.chars()
        .all(|c| c.is_ascii_alphanumeric() || "-._/=:".contains(c))
    {
        s.to_string()
    } else {
        format!("'{}'", s.replace('\'', r"'\''"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shell_quote_escapes_only_when_needed() {
        assert_eq!(shell_quote("simple"), "simple");
        assert_eq!(shell_quote("path/to/bin"), "path/to/bin");
        assert_eq!(shell_quote("has space"), "'has space'");
        assert_eq!(shell_quote("it's"), r"'it'\''s'");
    }
}
