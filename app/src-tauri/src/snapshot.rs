// Stack snapshot / restore (B11).
//
// Scope (intentional MVP): the stack *configuration* — its TOML — plus
// metadata about when + by whom it was captured. Volume *data* is
// deliberately NOT in scope: Apple's `container` runtime doesn't expose
// a stable host-side mountpoint we can snapshot from outside the
// container, and an inconsistent restore is worse than no restore.
// The frontend communicates this via copy in the Snapshot button hint.
//
// Wire format: a single self-describing JSON envelope. No tar, no gzip,
// no new crates — `serde_json` was already a dependency. Round-trip is
// `restore(create(name)) == name's TOML on disk`.

use anyhow::{anyhow, Context, Result};
use chrono::Utc;
use serde::{Deserialize, Serialize};

const MAGIC: &str = "cgui-snapshot";
const VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Envelope {
    /// Magic discriminator so the restore command can refuse files of
    /// unknown shape early instead of producing confusing parse errors.
    pub kind: String,
    /// Schema version. We only ship v1 today; future versions can
    /// migrate older payloads on read.
    pub version: u32,
    /// ISO-8601 timestamp of when the snapshot was taken.
    pub created_at: String,
    /// cgui-visual app version string at snapshot time. Diagnostic only.
    pub cgui_version: String,
    /// Stack payload — name + raw TOML body.
    pub stack: SnapshotStack,
    /// Free-form note from the user. Empty when omitted.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub note: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SnapshotStack {
    pub name: String,
    pub toml: String,
}

/// Build a snapshot envelope for `name` and serialise to a pretty
/// JSON string suitable for piping into a Blob download. Returns Err
/// when the stack file can't be located or read.
pub fn create(name: &str, note: &str) -> Result<String> {
    let dir = crate::stacks::stacks_dir().context("could not resolve $XDG_CONFIG_HOME or $HOME")?;
    let path = dir.join(format!("{name}.toml"));
    let toml_body =
        std::fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;

    let env = Envelope {
        kind: MAGIC.into(),
        version: VERSION,
        created_at: Utc::now().to_rfc3339(),
        cgui_version: env!("CARGO_PKG_VERSION").to_string(),
        stack: SnapshotStack {
            name: name.into(),
            toml: toml_body,
        },
        note: note.to_string(),
    };
    serde_json::to_string_pretty(&env).context("serialise snapshot envelope")
}

/// Restore a snapshot. Reads the JSON envelope, validates magic +
/// version, and writes the TOML body back to ~/.config/cgui/stacks/
/// `<name>.toml`. Refuses to overwrite an existing stack of the same
/// name unless `overwrite` is true; returns the destination path on
/// success so the UI can surface it in a toast.
pub fn restore(json: &str, overwrite: bool) -> Result<String> {
    let env: Envelope =
        serde_json::from_str(json).context("snapshot file is not a cgui-snapshot envelope")?;
    if env.kind != MAGIC {
        return Err(anyhow!("expected kind '{MAGIC}', got '{}'", env.kind));
    }
    if env.version != VERSION {
        return Err(anyhow!(
            "snapshot version {} not supported (this build understands v{VERSION})",
            env.version
        ));
    }
    if env.stack.name.is_empty() {
        return Err(anyhow!("snapshot has empty stack name"));
    }

    let dir = crate::stacks::stacks_dir().context("could not resolve $XDG_CONFIG_HOME or $HOME")?;
    std::fs::create_dir_all(&dir).with_context(|| format!("create {}", dir.display()))?;
    let dest = dir.join(format!("{}.toml", env.stack.name));
    if dest.exists() && !overwrite {
        return Err(anyhow!(
            "stack '{}' already exists at {}; pass overwrite=true to replace",
            env.stack.name,
            dest.display()
        ));
    }
    std::fs::write(&dest, &env.stack.toml).with_context(|| format!("write {}", dest.display()))?;
    Ok(dest.display().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_envelope() -> String {
        serde_json::to_string_pretty(&Envelope {
            kind: MAGIC.into(),
            version: VERSION,
            created_at: "2026-05-07T20:00:00Z".into(),
            cgui_version: "0.1.1".into(),
            stack: SnapshotStack {
                name: "demo".into(),
                toml: "name = \"demo\"\n[[service]]\nname = \"web\"\nimage = \"nginx\"\n".into(),
            },
            note: String::new(),
        })
        .unwrap()
    }

    #[test]
    fn restore_rejects_unknown_kind() {
        let bad = r#"{"kind":"not-cgui","version":1,"createdAt":"x","cguiVersion":"y","stack":{"name":"a","toml":""}}"#;
        let err = restore(bad, true).unwrap_err().to_string();
        assert!(err.contains("expected kind"));
    }

    #[test]
    fn restore_rejects_unsupported_version() {
        let bad = r#"{"kind":"cgui-snapshot","version":99,"createdAt":"x","cguiVersion":"y","stack":{"name":"a","toml":""}}"#;
        let err = restore(bad, true).unwrap_err().to_string();
        assert!(err.contains("version 99"));
    }

    #[test]
    fn restore_rejects_empty_stack_name() {
        let bad = r#"{"kind":"cgui-snapshot","version":1,"createdAt":"x","cguiVersion":"y","stack":{"name":"","toml":""}}"#;
        let err = restore(bad, true).unwrap_err().to_string();
        assert!(err.contains("empty stack name"));
    }

    #[test]
    fn envelope_round_trips_through_json() {
        let s = sample_envelope();
        let parsed: Envelope = serde_json::from_str(&s).unwrap();
        assert_eq!(parsed.kind, MAGIC);
        assert_eq!(parsed.stack.name, "demo");
        assert!(parsed.stack.toml.contains("nginx"));
    }
}
