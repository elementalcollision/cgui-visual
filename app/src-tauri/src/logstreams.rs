//! Display log streams: the `container logs -f` children behind the Logs
//! view (`start_log_stream`), as opposed to the background capture
//! followers owned by `logpersist`.
//!
//! The frontend generates a stream id, subscribes to `logs:tick:<id>`, then
//! invokes `start_log_stream`. Every live stream is tracked in a registry so
//! `stop_log_stream(streamId)` (or app exit via `stop_all`) can abort the
//! task, which drops the child (kill_on_drop). Per-stream channels keep two
//! views from cross-talking.
//!
//! `container logs -f` never exits when its container stops or is deleted,
//! so the poll tick also calls [`reconcile`]: streams whose container is gone
//! or no longer running are ended with a final marker line.

use crate::model::Container;
use crate::runtime;
use std::collections::HashMap;
use std::future::Future;
use std::sync::{Arc, LazyLock, Mutex, MutexGuard};
use std::time::{Duration, Instant};
use tokio::task::AbortHandle;

/// Sink for a stream's lines (the Tauri event emit in the app, a collector
/// in tests).
pub type Emit = Arc<dyn Fn(String) + Send + Sync>;

// Keep in sync with LOG_STREAM_END_MARKERS in app/src/views.tsx, which
// turns these into the Logs header state.
pub const MARKER_FOLLOW_ENDED: &str = "[follow ended]";
pub const MARKER_REMOVED: &str = "[container removed — log stream ended]";
pub const MARKER_STOPPED: &str = "[container stopped — log stream ended]";

/// A stream is never ended in its first seconds: the container list that
/// triggered the reconcile may predate the stream's own container.
const MIN_AGE: Duration = Duration::from_secs(3);
/// A stream opened on a container that is already stopped replays its log
/// and then idles; give the replay this long before ending it.
const STOPPED_GRACE: Duration = Duration::from_secs(15);

struct Entry {
    abort: AbortHandle,
    container: String,
    boot: bool,
    started: Instant,
    seen_running: bool,
    emit: Emit,
}

#[derive(Default)]
pub struct LogStreams {
    map: Mutex<HashMap<String, Entry>>,
}

impl LogStreams {
    fn lock(&self) -> MutexGuard<'_, HashMap<String, Entry>> {
        self.map.lock().unwrap_or_else(|p| p.into_inner())
    }
    /// Register `id`; any stream already registered under it is aborted.
    fn register(map: &mut HashMap<String, Entry>, id: String, entry: Entry) {
        if let Some(old) = map.insert(id, entry) {
            old.abort.abort();
        }
    }
    /// Abort + forget. Returns whether the id was registered.
    pub fn stop(&self, id: &str) -> bool {
        match self.lock().remove(id) {
            Some(e) => {
                e.abort.abort();
                true
            }
            None => false,
        }
    }
    /// Forget without aborting (the stream ended by itself).
    fn finish(&self, id: &str) {
        self.lock().remove(id);
    }
    pub fn stop_all(&self) {
        for (_, e) in self.lock().drain() {
            e.abort.abort();
        }
    }
    pub fn len(&self) -> usize {
        self.lock().len()
    }
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// End display streams whose container is gone or stopped: abort the
    /// child, forget the entry (so a later `stop` is a harmless no-op) and
    /// emit the marker. Returns the ended stream ids.
    pub fn reconcile(&self, containers: &[Container]) -> Vec<String> {
        let mut states: HashMap<&str, bool> = HashMap::new();
        for c in containers {
            let running = c.status.eq_ignore_ascii_case("running");
            states.insert(c.id.as_str(), running);
            states.insert(c.name.as_str(), running);
        }
        let now = Instant::now();
        let mut map = self.lock();
        let views: Vec<StreamView> = map
            .iter_mut()
            .map(|(id, e)| {
                if states.get(e.container.as_str()) == Some(&true) {
                    e.seen_running = true;
                }
                StreamView {
                    stream_id: id.clone(),
                    container: e.container.clone(),
                    boot: e.boot,
                    age: now.duration_since(e.started),
                    seen_running: e.seen_running,
                }
            })
            .collect();
        let mut ended = Vec::new();
        for d in decide_ends(&views, &states) {
            if let Some(e) = map.remove(&d.stream_id) {
                e.abort.abort();
                (e.emit)(d.marker.to_string());
                ended.push(d.stream_id);
            }
        }
        ended
    }

    /// Spawn `fut` as a registered stream. The registry lock is held across
    /// spawn+insert so a stream that ends instantly can't call `finish`
    /// before it is registered (which would leave a stale entry behind).
    pub fn spawn_registered<Fut>(
        &'static self,
        stream_id: String,
        container: String,
        boot: bool,
        emit: Emit,
        fut: Fut,
    ) where
        Fut: Future<Output = ()> + Send + 'static,
    {
        let sid = stream_id.clone();
        let mut map = self.lock();
        let task = tokio::spawn(async move {
            fut.await;
            self.finish(&sid);
        });
        Self::register(
            &mut map,
            stream_id,
            Entry {
                abort: task.abort_handle(),
                container,
                boot,
                started: Instant::now(),
                seen_running: false,
                emit,
            },
        );
    }
}

static LOG_STREAMS: LazyLock<LogStreams> = LazyLock::new(LogStreams::default);

pub fn registry() -> &'static LogStreams {
    &LOG_STREAMS
}

/// Kill every running log stream (app-exit hook).
pub fn stop_all() {
    LOG_STREAMS.stop_all();
}

pub fn stop(stream_id: &str) {
    LOG_STREAMS.stop(stream_id);
}

/// Poll-tick hook: end display streams of removed / stopped containers.
pub fn reconcile(containers: &[Container]) -> Vec<String> {
    LOG_STREAMS.reconcile(containers)
}

/// Spawn `container <argv>` (a `logs -f …` invocation) as the display stream
/// `stream_id` for `container`, emitting each line plus a final
/// "[follow ended]" when the child exits on its own. Needs a Tokio runtime.
pub fn spawn_follow(
    stream_id: String,
    container: String,
    boot: bool,
    argv: &[&str],
    emit: Emit,
) -> anyhow::Result<()> {
    spawn_follow_in(&LOG_STREAMS, stream_id, container, boot, argv, emit)
}

pub fn spawn_follow_in(
    reg: &'static LogStreams,
    stream_id: String,
    container: String,
    boot: bool,
    argv: &[&str],
    emit: Emit,
) -> anyhow::Result<()> {
    let child = runtime::spawn(argv)?;
    let line_emit = emit.clone();
    let end_emit = emit.clone();
    reg.spawn_registered(stream_id, container, boot, emit, async move {
        let _ = runtime::drain_lines(child, move |line| line_emit(line)).await;
        end_emit(MARKER_FOLLOW_ENDED.to_string());
    });
    Ok(())
}

/// Dev-only: replay `lines` with a small delay as a registered stream.
pub fn spawn_replay(stream_id: String, container: String, lines: Vec<String>, emit: Emit) {
    let e = emit.clone();
    LOG_STREAMS.spawn_registered(stream_id, container, false, emit, async move {
        for line in lines {
            e(line);
            tokio::time::sleep(Duration::from_millis(80)).await;
        }
    });
}

// ─── Pure reconcile decision ──────────────────────────────────────────

/// Snapshot of one registered display stream.
#[derive(Debug, Clone)]
pub struct StreamView {
    pub stream_id: String,
    pub container: String,
    /// `logs --boot`: the VM boot log of a stopped container is legitimate.
    pub boot: bool,
    pub age: Duration,
    /// The stream's container has been seen running since it started.
    pub seen_running: bool,
}

#[derive(Debug, PartialEq, Eq)]
pub struct EndDecision {
    pub stream_id: String,
    pub marker: &'static str,
}

/// Which streams to end given the current container states (id or name →
/// running). Decisions are sorted by stream id.
///
/// - Container absent from the list: ended with [`MARKER_REMOVED`] (boot
///   streams too).
/// - Container listed but not running: ended with [`MARKER_STOPPED`], except
///   boot streams, and except streams opened on an already-stopped
///   container, which get [`STOPPED_GRACE`] to finish replaying.
/// - Nothing ends before [`MIN_AGE`].
pub fn decide_ends(streams: &[StreamView], containers: &HashMap<&str, bool>) -> Vec<EndDecision> {
    let mut out: Vec<EndDecision> = streams
        .iter()
        .filter(|s| s.age >= MIN_AGE)
        .filter_map(|s| {
            let marker = match containers.get(s.container.as_str()) {
                None => MARKER_REMOVED,
                Some(true) => return None,
                Some(false) => {
                    if s.boot || (!s.seen_running && s.age < STOPPED_GRACE) {
                        return None;
                    }
                    MARKER_STOPPED
                }
            };
            Some(EndDecision {
                stream_id: s.stream_id.clone(),
                marker,
            })
        })
        .collect();
    out.sort_by(|a, b| a.stream_id.cmp(&b.stream_id));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pending() -> tokio::task::JoinHandle<()> {
        tokio::spawn(async { tokio::time::sleep(Duration::from_secs(3600)).await })
    }

    fn noop_emit() -> Emit {
        Arc::new(|_| {})
    }

    fn entry(t: &tokio::task::JoinHandle<()>, container: &str) -> Entry {
        Entry {
            abort: t.abort_handle(),
            container: container.into(),
            boot: false,
            started: Instant::now(),
            seen_running: false,
            emit: noop_emit(),
        }
    }

    #[tokio::test]
    async fn register_then_stop_aborts_task() {
        let reg = LogStreams::default();
        let t = pending();
        LogStreams::register(&mut reg.lock(), "a".into(), entry(&t, "c"));
        assert!(reg.stop("a"));
        assert!(t.await.unwrap_err().is_cancelled());
        assert!(!reg.stop("a"), "second stop is a no-op");
    }

    #[tokio::test]
    async fn stop_only_touches_the_named_stream() {
        let reg = LogStreams::default();
        let (a, b) = (pending(), pending());
        LogStreams::register(&mut reg.lock(), "a".into(), entry(&a, "c"));
        LogStreams::register(&mut reg.lock(), "b".into(), entry(&b, "c"));
        reg.stop("a");
        assert!(a.await.unwrap_err().is_cancelled());
        assert!(!b.is_finished());
        assert_eq!(reg.len(), 1);
        reg.stop_all();
        assert!(b.await.unwrap_err().is_cancelled());
        assert!(reg.is_empty());
    }

    #[tokio::test]
    async fn finish_removes_without_aborting() {
        let reg = LogStreams::default();
        let t = pending();
        LogStreams::register(&mut reg.lock(), "a".into(), entry(&t, "c"));
        reg.finish("a");
        assert!(reg.is_empty());
        assert!(!t.is_finished());
        t.abort();
    }

    #[tokio::test]
    async fn reregistering_an_id_aborts_the_old_task() {
        let reg = LogStreams::default();
        let (old, new) = (pending(), pending());
        LogStreams::register(&mut reg.lock(), "a".into(), entry(&old, "c"));
        LogStreams::register(&mut reg.lock(), "a".into(), entry(&new, "c"));
        assert!(old.await.unwrap_err().is_cancelled());
        assert!(!new.is_finished());
        new.abort();
    }

    fn view(id: &str, container: &str, age_s: u64, seen_running: bool, boot: bool) -> StreamView {
        StreamView {
            stream_id: id.into(),
            container: container.into(),
            boot,
            age: Duration::from_secs(age_s),
            seen_running,
        }
    }

    fn states(v: &[(&'static str, bool)]) -> HashMap<&'static str, bool> {
        v.iter().copied().collect()
    }

    fn d(id: &str, marker: &'static str) -> EndDecision {
        EndDecision {
            stream_id: id.into(),
            marker,
        }
    }

    #[test]
    fn running_container_keeps_its_stream() {
        let s = [view("a", "web", 60, true, false)];
        assert!(decide_ends(&s, &states(&[("web", true)])).is_empty());
    }

    #[test]
    fn missing_container_ends_with_removed_marker() {
        let s = [view("a", "web", 60, true, false)];
        assert_eq!(
            decide_ends(&s, &states(&[("db", true)])),
            vec![d("a", MARKER_REMOVED)]
        );
        assert_eq!(
            decide_ends(&s, &HashMap::new()),
            vec![d("a", MARKER_REMOVED)]
        );
    }

    #[test]
    fn stopped_container_ends_with_stopped_marker() {
        let s = [view("a", "web", 60, true, false)];
        assert_eq!(
            decide_ends(&s, &states(&[("web", false)])),
            vec![d("a", MARKER_STOPPED)]
        );
    }

    #[test]
    fn only_streams_of_affected_containers_end() {
        let s = [
            view("a", "web", 60, true, false),
            view("b", "db", 60, true, false),
            view("c", "gone", 60, true, false),
            view("d", "web", 60, true, false),
        ];
        let got = decide_ends(&s, &states(&[("web", false), ("db", true)]));
        assert_eq!(
            got,
            vec![
                d("a", MARKER_STOPPED),
                d("c", MARKER_REMOVED),
                d("d", MARKER_STOPPED)
            ]
        );
    }

    #[test]
    fn fresh_streams_are_left_alone() {
        let s = [view("a", "web", 1, false, false)];
        assert!(decide_ends(&s, &HashMap::new()).is_empty());
        assert!(decide_ends(&s, &states(&[("web", false)])).is_empty());
    }

    #[test]
    fn stream_on_already_stopped_container_gets_replay_grace() {
        let young = [view("a", "web", 10, false, false)];
        assert!(decide_ends(&young, &states(&[("web", false)])).is_empty());
        let old = [view("a", "web", 20, false, false)];
        assert_eq!(
            decide_ends(&old, &states(&[("web", false)])),
            vec![d("a", MARKER_STOPPED)]
        );
    }

    #[test]
    fn boot_streams_survive_stopped_but_not_removed() {
        let s = [view("a", "web", 60, true, true)];
        assert!(decide_ends(&s, &states(&[("web", false)])).is_empty());
        assert_eq!(
            decide_ends(&s, &HashMap::new()),
            vec![d("a", MARKER_REMOVED)]
        );
    }

    fn container(id: &str, status: &str) -> Container {
        Container {
            id: id.into(),
            name: id.into(),
            status: status.into(),
            ..Default::default()
        }
    }

    #[tokio::test]
    async fn reconcile_ends_aborts_emits_and_forgets() {
        let reg = LogStreams::default();
        let seen = Arc::new(Mutex::new(Vec::<String>::new()));
        let sink = seen.clone();
        let t = pending();
        let mut e = entry(&t, "web");
        e.started = Instant::now() - Duration::from_secs(30);
        e.emit = Arc::new(move |l| sink.lock().unwrap().push(l));
        LogStreams::register(&mut reg.lock(), "a".into(), e);
        let keep = pending();
        let mut k = entry(&keep, "db");
        k.started = Instant::now() - Duration::from_secs(30);
        LogStreams::register(&mut reg.lock(), "b".into(), k);

        let ended = reg.reconcile(&[container("web", "stopped"), container("db", "running")]);
        assert_eq!(ended, vec!["a".to_string()]);
        assert_eq!(*seen.lock().unwrap(), vec![MARKER_STOPPED.to_string()]);
        assert!(t.await.unwrap_err().is_cancelled());
        assert!(!keep.is_finished());
        // The frontend's later stop is harmless.
        assert!(!reg.stop("a"));
        assert_eq!(reg.len(), 1);
        keep.abort();
    }
}
