//! Config-directory resolution shared by prefs, history, stacks and doctor.
//!
//! The base is `$XDG_CONFIG_HOME` when set, otherwise `$HOME/.config`. Each
//! caller picks its own subfolder (`cgui` for stacks and the doctor's config
//! checks, `cgui-gui` for prefs and the history DB); those names are part of
//! users' on-disk layout, so they must not change.

use std::ffi::OsString;
use std::path::PathBuf;

/// `<config base>/<app_dir>`, or `None` when neither `$XDG_CONFIG_HOME` nor
/// `$HOME` is set.
pub(crate) fn config_dir(app_dir: &str) -> Option<PathBuf> {
    resolve(
        std::env::var_os("XDG_CONFIG_HOME"),
        std::env::var_os("HOME"),
        app_dir,
    )
}

fn resolve(xdg: Option<OsString>, home: Option<OsString>, app_dir: &str) -> Option<PathBuf> {
    let base = xdg
        .map(PathBuf::from)
        .or_else(|| home.map(|h| PathBuf::from(h).join(".config")))?;
    Some(base.join(app_dir))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn os(s: &str) -> Option<OsString> {
        Some(OsString::from(s))
    }

    #[test]
    fn xdg_wins_over_home() {
        assert_eq!(
            resolve(os("/x"), os("/h"), "cgui-gui"),
            Some(PathBuf::from("/x/cgui-gui"))
        );
    }

    #[test]
    fn home_fallback_uses_dot_config() {
        assert_eq!(
            resolve(None, os("/h"), "cgui"),
            Some(PathBuf::from("/h/.config/cgui"))
        );
    }

    #[test]
    fn neither_set_is_none() {
        assert_eq!(resolve(None, None, "cgui"), None);
    }

    #[test]
    fn callers_keep_their_existing_paths() {
        // The exact suffixes each module appended before the helper existed.
        let base = |sub: &str| resolve(os("/c"), None, sub).unwrap();
        assert_eq!(
            base("cgui-gui").join("state.json"),
            PathBuf::from("/c/cgui-gui/state.json")
        );
        assert_eq!(
            base("cgui-gui").join("history.db"),
            PathBuf::from("/c/cgui-gui/history.db")
        );
        assert_eq!(base("cgui").join("stacks"), PathBuf::from("/c/cgui/stacks"));
        assert_eq!(base("cgui").join("x.yml"), PathBuf::from("/c/cgui/x.yml"));
    }
}
