//! Log-history persistence (B12), decoupled from the display stream.
//!
//! The Logs view runs `container logs -f [-n N] <id>` for display; that
//! command REPLAYS the log on every open, so persisting the display stream
//! stored the same lines again each time a view opened.
//!
//! Verified against Apple container 1.7.1:
//!   * `logs -n 0` prints nothing; `logs -f -n 0` prints only lines written
//!     after it attaches (no replay).
//!   * `logs -f` never exits when the container stops (the process lingers
//!     until killed), so followers must be owned by something with a
//!     lifetime. Ours are owned by the display stream that triggered them.
//!
//! Design: display streams persist nothing. Instead, opening a view starts
//! (at most one per container) a separate persisting follower:
//!   * no stored rows yet  -> `logs -f <id>`: the replay is stored once, so
//!     history exists after the container is gone;
//!   * rows already stored -> `logs -f -n 0 <id>`: only new lines, no
//!     duplicates.
//!
//! The follower is stopped with the display stream that started it.
//! Known limit: lines written while no view was open and after the first
//! persisted replay are not captured (there is no offset to resume from).

use crate::{history, runtime};
use std::collections::HashMap;
use std::sync::{LazyLock, Mutex, MutexGuard};
use tokio::task::AbortHandle;

/// argv for the persisting follower. Pure so the choice is unit-tested.
pub fn follow_args(container: &str, has_history: bool) -> Vec<String> {
    let mut v: Vec<String> = vec!["logs".into(), "-f".into()];
    if has_history {
        v.push("-n".into());
        v.push("0".into());
    }
    v.push(container.into());
    v
}

struct Entry {
    owner: String,
    handle: Option<AbortHandle>,
}

#[derive(Default)]
struct Registry {
    map: Mutex<HashMap<String, Entry>>,
}

impl Registry {
    fn lock(&self) -> MutexGuard<'_, HashMap<String, Entry>> {
        self.map.lock().unwrap_or_else(|p| p.into_inner())
    }
    /// Claim `container` for `owner`; false if a follower already exists.
    fn claim(&self, container: &str, owner: &str) -> bool {
        let mut m = self.lock();
        if m.contains_key(container) {
            return false;
        }
        m.insert(
            container.to_string(),
            Entry {
                owner: owner.to_string(),
                handle: None,
            },
        );
        true
    }
    /// Attach the task handle; false if the claim was released meanwhile.
    fn attach(&self, container: &str, owner: &str, handle: AbortHandle) -> bool {
        match self.lock().get_mut(container) {
            Some(e) if e.owner == owner => {
                e.handle = Some(handle);
                true
            }
            _ => false,
        }
    }
    /// Drop the claim (follower ended by itself) without aborting.
    fn release(&self, container: &str, owner: &str) {
        let mut m = self.lock();
        if m.get(container).is_some_and(|e| e.owner == owner) {
            m.remove(container);
        }
    }
    /// Abort and drop every claim held by `owner`.
    fn stop_owner(&self, owner: &str) {
        self.lock().retain(|_, e| {
            if e.owner == owner {
                if let Some(h) = &e.handle {
                    h.abort();
                }
                false
            } else {
                true
            }
        });
    }
    fn stop_all(&self) {
        for (_, e) in self.lock().drain() {
            if let Some(h) = e.handle {
                h.abort();
            }
        }
    }
}

static REGISTRY: LazyLock<Registry> = LazyLock::new(Registry::default);

/// Ensure `container`'s output is being persisted, on behalf of display
/// stream `owner`. Returns Ok(true) if a follower was started, Ok(false)
/// if one already runs (or history is unavailable).
pub async fn start(owner: &str, container: &str) -> anyhow::Result<bool> {
    if !REGISTRY.claim(container, owner) {
        return Ok(false);
    }
    // Queued lines from a previous follower must be on disk before we
    // decide between full replay and `-n 0`.
    let cid = container.to_string();
    let has = tokio::task::spawn_blocking(move || {
        history::flush_logs();
        history::has_logs(&cid)
    })
    .await
    .ok()
    .flatten();
    let Some(has) = has else {
        REGISTRY.release(container, owner);
        return Ok(false);
    };
    let argv = follow_args(container, has);
    let argv_ref: Vec<&str> = argv.iter().map(String::as_str).collect();
    let child = match runtime::spawn(&argv_ref) {
        Ok(c) => c,
        Err(e) => {
            REGISTRY.release(container, owner);
            return Err(e);
        }
    };
    let (cid, own) = (container.to_string(), owner.to_string());
    let task = tokio::spawn(async move {
        let cid2 = cid.clone();
        let _ = runtime::drain_lines(child, move |line| history::record_log(&cid2, &line)).await;
        REGISTRY.release(&cid, &own);
    });
    if !REGISTRY.attach(container, owner, task.abort_handle()) {
        // The display stream was stopped while we were starting up.
        task.abort();
        return Ok(false);
    }
    Ok(true)
}

/// Stop the follower(s) started on behalf of display stream `owner`.
pub fn stop_owner(owner: &str) {
    REGISTRY.stop_owner(owner);
}

/// App exit.
pub fn stop_all() {
    REGISTRY.stop_all();
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn first_view_replays_then_later_views_only_follow() {
        assert_eq!(follow_args("c1", false), ["logs", "-f", "c1"]);
        assert_eq!(follow_args("c1", true), ["logs", "-f", "-n", "0", "c1"]);
    }

    #[test]
    fn one_follower_per_container() {
        let r = Registry::default();
        assert!(r.claim("c", "s1"));
        assert!(!r.claim("c", "s2"));
        assert!(r.claim("d", "s2"));
    }

    #[test]
    fn release_only_by_owner() {
        let r = Registry::default();
        assert!(r.claim("c", "s1"));
        r.release("c", "s2");
        assert!(!r.claim("c", "s3"), "foreign release must not free it");
        r.release("c", "s1");
        assert!(r.claim("c", "s3"));
    }

    #[tokio::test]
    async fn stop_owner_aborts_and_frees_and_blocks_late_attach() {
        let r = Registry::default();
        assert!(r.claim("c", "s1"));
        let t = tokio::spawn(async { tokio::time::sleep(Duration::from_secs(3600)).await });
        assert!(r.attach("c", "s1", t.abort_handle()));
        r.stop_owner("s1");
        assert!(t.await.unwrap_err().is_cancelled());
        assert!(r.claim("c", "s2"), "claim freed");

        // A start that loses the race against stop_owner must not attach.
        assert!(r.claim("e", "s9"));
        r.stop_owner("s9");
        let t2 = tokio::spawn(async { tokio::time::sleep(Duration::from_secs(3600)).await });
        assert!(!r.attach("e", "s9", t2.abort_handle()));
        t2.abort();
    }
}
