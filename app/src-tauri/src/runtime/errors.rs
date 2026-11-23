// ─── CLI error classification ─────────────────────────────────────────
//
// Texts observed on Apple `container` 1.4.1:
//   stop/delete unknown id: `Error: internalError: "failed to stop container"
//       (cause: "notFound: "container with ID X not found"")`
//   start unknown id:       `Error: get failed: container X not found`
//   delete while running:   `... invalidState: "container X is running and
//       can not be deleted"`
//   stop on an already-stopped container succeeds (exit 0).
//   services down (every subcommand, incl. `system status`):
//       `Error: Plugins are unavailable. Start the container system services
//       and retry: container system start`
// Matching is on lowercase substrings of the whole message; ids/names can't
// contain spaces so they can't produce a false "not found" / "not running".
/// The container doesn't exist (any of stop/delete/start/inspect).
pub fn is_not_found_error(msg: &str) -> bool {
    msg.to_ascii_lowercase().contains("not found")
}

/// Stop was a no-op because the container wasn't running. 1.4.1 exits 0 in
/// this case; older CLIs reported "container is not running".
pub fn is_not_running_error(msg: &str) -> bool {
    let m = msg.to_ascii_lowercase();
    m.contains("not running") || m.contains("already stopped")
}

/// A failed `stop` that leaves the container in the desired (not running)
/// state: it's gone or already stopped.
pub fn is_benign_stop_error(msg: &str) -> bool {
    is_not_found_error(msg) || is_not_running_error(msg)
}

/// A failed `delete` that leaves the container in the desired (absent)
/// state. "is running and can not be deleted" is deliberately NOT benign.
pub fn is_benign_delete_error(msg: &str) -> bool {
    is_not_found_error(msg)
}

/// The runtime's API server / plugins aren't up (`container system start`
/// needed). The binary itself works, so `available()` is still true.
pub fn is_services_down_error(msg: &str) -> bool {
    let m = msg.to_ascii_lowercase();
    m.contains("plugins are unavailable") || m.contains("container system start")
}

// Shared with the image-ls tests in parse.rs.
#[cfg(test)]
pub(super) const SERVICES_DOWN: &str = "`container stop web` exited exit status: 1: Error: Plugins are unavailable. Start the container system services and retry:\n\n    container system start";

#[cfg(test)]
mod tests {
    use super::*;

    const STOP_NOT_FOUND: &str = "`container stop web` exited exit status: 1: Error: internalError: \"failed to stop container\" (cause: \"notFound: \"container with ID web not found\"\")";

    const DELETE_NOT_FOUND: &str = "`container delete web` exited exit status: 1: Error: internalError: \"failed to delete container\" (cause: \"notFound: \"container with ID web not found\"\")";

    const DELETE_RUNNING: &str = "`container delete web` exited exit status: 1: Error: internalError: \"failed to delete container\" (cause: \"invalidState: \"container web is running and can not be deleted\"\")";

    #[test]
    fn stop_errors_for_missing_or_stopped_containers_are_benign() {
        assert!(is_benign_stop_error(STOP_NOT_FOUND));
        assert!(is_benign_stop_error(
            "`container stop web` exited exit status: 1: Error: container is not running"
        ));
        assert!(is_benign_stop_error("container web already stopped"));
    }

    #[test]
    fn real_stop_errors_are_not_benign() {
        assert!(!is_benign_stop_error(SERVICES_DOWN));
        assert!(!is_benign_stop_error(
            "`container stop web` timed out after 60s"
        ));
        assert!(!is_benign_stop_error(
            "failed to spawn `container stop web`"
        ));
    }

    #[test]
    fn delete_only_tolerates_not_found() {
        assert!(is_benign_delete_error(DELETE_NOT_FOUND));
        assert!(!is_benign_delete_error(DELETE_RUNNING));
        assert!(!is_benign_delete_error(SERVICES_DOWN));
    }

    #[test]
    fn restart_stop_tolerates_only_not_running() {
        // not-found must propagate from restart's stop (the user asked to
        // restart a container that doesn't exist).
        assert!(!is_not_running_error(STOP_NOT_FOUND));
        assert!(is_not_running_error("Error: container is not running"));
        assert!(!is_not_running_error(SERVICES_DOWN));
    }

    #[test]
    fn services_down_error_is_recognised() {
        assert!(is_services_down_error(SERVICES_DOWN));
        assert!(is_services_down_error(
            "Error: Plugins are unavailable. Start the container system services and retry"
        ));
        assert!(!is_services_down_error(STOP_NOT_FOUND));
        assert!(!is_services_down_error(DELETE_RUNNING));
    }

    #[test]
    fn services_down_matches_1_6_plugin_text() {
        // Verbatim 1.6+/1.7 text for every subcommand incl. build/builder.
        for sub in ["build", "builder status", "image ls"] {
            let msg = format!(
                "`container {sub}` exited exit status: 64: Error: Plugins are unavailable. Start the container system services and retry:\n\n    container system start"
            );
            assert!(is_services_down_error(&msg), "{sub}");
        }
        assert!(is_services_down_error("Plugins are unavailable."));
    }
}
