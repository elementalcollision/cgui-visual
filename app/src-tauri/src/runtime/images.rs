use super::*;

// ─── Images / Volumes / Networks ──────────────────────────────────────
/// Result of `image ls`: the readable images plus an optional non-blocking
/// warning when the CLI skipped unreadable ones.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ImageListing {
    pub images: Vec<Image>,
    pub warning: Option<String>,
}

pub async fn list_images() -> Result<Vec<Image>> {
    list_images_checked().await.map(|l| l.images)
}

/// `container image ls`. Since container 1.6 the CLI skips unreadable
/// images with a warning, prints the readable ones as valid JSON, then
/// exits 1 ("failed to read N image(s)"). Treat that as a partial
/// success so one corrupt image doesn't blank the whole view.
pub async fn list_images_checked() -> Result<ImageListing> {
    let args = ["image", "ls", "--format", "json"];
    let out = run_output(&args, RUN_TIMEOUT).await?;
    let stderr = String::from_utf8_lossy(&out.stderr);
    parse_image_ls(out.status.success(), &out.stdout, &stderr, &out.status)
        .map_err(|e| e.context(format!("`{} {}`", bin(), args.join(" "))))
}

pub async fn inspect_image(reference: &str) -> Result<String> {
    reject_flag_like("image reference", reference)?;
    inspect_args(&["image", "inspect", reference]).await
}

pub async fn delete_image(reference: &str) -> Result<()> {
    reject_flag_like("image reference", reference)?;
    run(&["image", "delete", reference]).await.map(|_| ())
}

pub async fn tag_image(source: &str, target: &str) -> Result<()> {
    reject_flag_like("source image reference", source)?;
    reject_flag_like("target image reference", target)?;
    run(&["image", "tag", source, target]).await.map(|_| ())
}

/// Basic sanity check for a *new* image reference (a name the user types, as
/// opposed to one already in the image list): `[host[:port]/]repo[:tag]
/// [@digest]` with a lowercase repository, a plain tag, no whitespace, and
/// nothing flag-like. The CLI is stricter still ("invalid format for image
/// reference"); this just turns the common mistakes into a specific message.
pub fn validate_image_ref(kind: &str, reference: &str) -> Result<()> {
    reject_flag_like(kind, reference)?;
    if reference.len() > 255 {
        return Err(anyhow!(
            "{kind} `{reference}` is longer than 255 characters"
        ));
    }
    if reference
        .chars()
        .any(|c| c.is_whitespace() || c.is_control() || c == '\\')
    {
        return Err(anyhow!(
            "{kind} `{reference}` must not contain spaces or control characters"
        ));
    }
    let no_digest = reference.split('@').next().unwrap_or(reference);
    let (name, tag) = match no_digest.rfind(':') {
        // A `:` before the last `/` belongs to a registry host:port.
        Some(i) if !no_digest[i..].contains('/') => (&no_digest[..i], Some(&no_digest[i + 1..])),
        _ => (no_digest, None),
    };
    if name.is_empty() || name.ends_with('/') || name.starts_with('/') {
        return Err(anyhow!("{kind} `{reference}` has no repository name"));
    }
    if name.chars().any(|c| {
        !(c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '.' | '_' | '-' | '/' | ':'))
    }) {
        return Err(anyhow!(
            "{kind} `{reference}`: the repository name must be lowercase letters, digits, `.`, `_`, `-` and `/`"
        ));
    }
    if let Some(tag) = tag {
        if tag.is_empty()
            || tag.len() > 128
            || !tag
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
        {
            return Err(anyhow!(
                "{kind} `{reference}` has an invalid tag (letters, digits, `.`, `_`, `-`; up to 128 characters)"
            ));
        }
    }
    Ok(())
}

/// `container commit <id> <reference>` (1.6+): snapshot a container's
/// filesystem (running or stopped) into a new image. Copying the disk can
/// take a while on large containers, so this uses the long timeout.
/// Returns the CLI's stdout (the resolved reference).
pub async fn commit_container(id: &str, reference: &str) -> Result<String> {
    reject_flag_like("container id", id)?;
    validate_image_ref("image reference", reference)?;
    let bytes = run_with_timeout(&["commit", id, reference], RUN_LONG_TIMEOUT)
        .await
        .map_err(map_commit_unsupported)?;
    Ok(String::from_utf8_lossy(&bytes).trim().to_string())
}

/// `container commit` is new in 1.6; older CLIs fail with an
/// `unknown command`/plugin-not-found error. Say what is needed instead.
pub(super) fn map_commit_unsupported(e: anyhow::Error) -> anyhow::Error {
    let msg = e.to_string();
    if msg.contains("exit status: 64")
        && (msg.contains("Plugin 'container-commit' not found")
            || msg.contains("unknown command 'commit'"))
    {
        anyhow!("`container commit` requires Apple container 1.6 or later")
    } else {
        e
    }
}

// Args for a streamed `container build`. Mirrors the BuildModal form.
#[derive(Debug, Default, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BuildArgs {
    pub context: String,
    #[serde(default)]
    pub tag: Option<String>,
    #[serde(default)]
    pub dockerfile: Option<String>,
    #[serde(default)]
    pub build_args: Vec<String>, // KEY=value each
    #[serde(default)]
    pub target: Option<String>,
    #[serde(default)]
    pub no_cache: bool,
}

pub fn build_argv(args: &BuildArgs) -> Vec<String> {
    let mut argv: Vec<String> = vec!["build".into(), "--progress".into(), "plain".into()];
    if let Some(t) = args.tag.as_deref().filter(|s| !s.is_empty()) {
        argv.push("-t".into());
        argv.push(t.into());
    }
    if let Some(f) = args.dockerfile.as_deref().filter(|s| !s.is_empty()) {
        argv.push("-f".into());
        argv.push(f.into());
    }
    for ba in args.build_args.iter().filter(|s| !s.is_empty()) {
        argv.push("--build-arg".into());
        argv.push(ba.clone());
    }
    if let Some(stage) = args.target.as_deref().filter(|s| !s.is_empty()) {
        argv.push("--target".into());
        argv.push(stage.into());
    }
    if args.no_cache {
        argv.push("--no-cache".into());
    }
    argv.push(args.context.clone());
    argv
}

// ─── Image lifecycle: push / save / load (1.0 parity) ─────────────────
// All three can move gigabytes, so they run on the long timeout. Push
// streams via spawn + drain_lines in commands.rs instead (progress UI).
pub async fn save_image(reference: &str, output: &str) -> Result<()> {
    reject_flag_like("image reference", reference)?;
    reject_flag_like("output path", output)?;
    run_with_timeout(
        &["image", "save", "-o", output, reference],
        RUN_LONG_TIMEOUT,
    )
    .await
    .map(|_| ())
}

pub async fn load_image(input: &str) -> Result<String> {
    reject_flag_like("input path", input)?;
    let bytes = run_with_timeout(&["image", "load", "-i", input], RUN_LONG_TIMEOUT).await?;
    Ok(String::from_utf8_lossy(&bytes).trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_argv_full_surface() {
        let args = BuildArgs {
            context: "/proj".into(),
            tag: Some("app:dev".into()),
            dockerfile: Some("/proj/Dockerfile.dev".into()),
            build_args: vec!["V=1".into()],
            target: Some("runtime".into()),
            no_cache: true,
        };
        let s = build_argv(&args).join(" ");
        assert!(s.starts_with("build --progress plain"));
        for frag in [
            "-t app:dev",
            "-f /proj/Dockerfile.dev",
            "--build-arg V=1",
            "--target runtime",
            "--no-cache",
        ] {
            assert!(s.contains(frag), "missing {frag} in {s}");
        }
        assert!(s.ends_with(" /proj"));
    }
}
