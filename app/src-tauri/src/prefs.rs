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
    /// When true, closing the main window hides it instead of quitting so
    /// the menu-bar tray stays alive. (UI affordance only — the close
    /// button still respects this in setup.)
    pub menubar_mode: bool,
    /// Global hotkey that summons + focuses the main window. Empty string
    /// disables. Format follows tauri-plugin-global-shortcut, e.g.
    /// "CmdOrCtrl+Alt+Space".
    pub global_hotkey: String,
    /// Fire a macOS notification when a container exits with a non-zero
    /// status. Only meaningful when the runtime is reachable.
    pub notify_on_exit: bool,
}

impl Default for Prefs {
    fn default() -> Self {
        Self {
            dark: true,
            sidebar_collapsed: false,
            runtime: "container".into(),
            last_tab: "containers".into(),
            menubar_mode: false,
            global_hotkey: String::new(),
            notify_on_exit: true,
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
            menubar_mode: true,
            global_hotkey: "CmdOrCtrl+Alt+Space".into(),
            notify_on_exit: false,
        };
        let s = serde_json::to_string(&p).unwrap();
        let back: Prefs = serde_json::from_str(&s).unwrap();
        assert!(!back.dark);
        assert!(back.sidebar_collapsed);
        assert_eq!(back.runtime, "podman");
        assert_eq!(back.last_tab, "logs");
        assert!(back.menubar_mode);
        assert_eq!(back.global_hotkey, "CmdOrCtrl+Alt+Space");
        assert!(!back.notify_on_exit);
    }

    #[test]
    fn prefs_tolerates_missing_new_fields() {
        // State files written by older versions lack menubar_mode /
        // global_hotkey / notify_on_exit. Verify defaults fill in.
        let s = r#"{"dark": false, "sidebarCollapsed": false, "runtime": "container", "lastTab": "containers"}"#;
        let p: Prefs = serde_json::from_str(s).unwrap();
        assert!(!p.menubar_mode);
        assert_eq!(p.global_hotkey, "");
        assert!(p.notify_on_exit);
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
