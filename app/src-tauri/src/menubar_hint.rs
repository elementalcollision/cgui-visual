//! One-time notification shown the first time the window is hidden by
//! closing it in Menu-bar mode. Menu-bar managers can hide third-party
//! status items, so the tray icon may be invisible; the notification says
//! how to get back (Spotlight/Finder reopen works regardless).

use crate::prefs::Prefs;
use tauri::Runtime;
use tauri_plugin_notification::NotificationExt;

/// Pure "only once" rule: post the hint unless it was already shown.
pub fn should_show(already_shown: bool) -> bool {
    !already_shown
}

pub fn message(global_hotkey: &str) -> String {
    let mut s = String::from(
        "cgui is still running in the menu bar. Click its menu bar icon, or open cgui \
         again (Spotlight or Finder), to bring the window back.",
    );
    let hk = global_hotkey.trim();
    if !hk.is_empty() {
        s.push_str(&format!(" You can also press {hk}."));
    }
    s
}

/// Call right after the window was hidden by a close in Menu-bar mode.
pub fn on_hidden_by_close<R: Runtime>(app: &tauri::AppHandle<R>) {
    let mut prefs = Prefs::load();
    if !should_show(prefs.menubar_hint_shown) {
        return;
    }
    let shown = app
        .notification()
        .builder()
        .title("cgui is still running")
        .body(message(&prefs.global_hotkey))
        .show()
        .is_ok();
    if shown {
        prefs.menubar_hint_shown = true;
        let _ = prefs.save();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shows_only_once() {
        assert!(should_show(false));
        assert!(!should_show(true));
    }

    #[test]
    fn message_mentions_hotkey_only_when_set() {
        let plain = message("");
        assert!(plain.contains("menu bar") && plain.contains("Spotlight"));
        assert!(!plain.contains("press"));
        assert_eq!(message("  "), plain);
        assert!(message("CmdOrCtrl+Alt+Space").contains("press CmdOrCtrl+Alt+Space"));
    }
}
