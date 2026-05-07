// UI preferences persisted to ~/.config/cgui-gui/state.json. Kept separate
// from cgui's state.json so the GUI can evolve its own keys.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Prefs {
    pub dark: bool,
    pub sidebar_collapsed: bool,
    pub runtime: String,
    pub last_tab: String,
}

impl Default for Prefs {
    fn default() -> Self {
        Self {
            dark: true,
            sidebar_collapsed: false,
            runtime: "container".into(),
            last_tab: "containers".into(),
        }
    }
}

impl Prefs {
    pub fn load() -> Self {
        match path().and_then(|p| std::fs::read_to_string(&p).ok()) {
            Some(s) => serde_json::from_str(&s).unwrap_or_default(),
            None => Self::default(),
        }
    }

    pub fn save(&self) -> Result<()> {
        let p = path().context("could not resolve $XDG_CONFIG_HOME or $HOME")?;
        if let Some(parent) = p.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("create {}", parent.display()))?;
        }
        let s = serde_json::to_string_pretty(self)?;
        std::fs::write(&p, s).with_context(|| format!("write {}", p.display()))?;
        Ok(())
    }
}

fn path() -> Option<PathBuf> {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))?;
    Some(base.join("cgui-gui").join("state.json"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_prefs_match_app_defaults() {
        let p = Prefs::default();
        assert!(p.dark);
        assert!(!p.sidebar_collapsed);
        assert_eq!(p.runtime, "container");
        assert_eq!(p.last_tab, "containers");
    }

    #[test]
    fn prefs_round_trip_via_json() {
        let p = Prefs {
            dark: false,
            sidebar_collapsed: true,
            runtime: "podman".into(),
            last_tab: "logs".into(),
        };
        let s = serde_json::to_string(&p).unwrap();
        let back: Prefs = serde_json::from_str(&s).unwrap();
        assert!(!back.dark);
        assert!(back.sidebar_collapsed);
        assert_eq!(back.runtime, "podman");
        assert_eq!(back.last_tab, "logs");
    }

    #[test]
    fn prefs_tolerates_missing_fields() {
        // Older state.json versions may lack `last_tab`. Verify the default
        // fills in cleanly.
        let s = r#"{"dark": false}"#;
        let p: Prefs = serde_json::from_str(s).unwrap();
        assert!(!p.dark);
        assert_eq!(p.last_tab, "containers"); // default
    }
}
