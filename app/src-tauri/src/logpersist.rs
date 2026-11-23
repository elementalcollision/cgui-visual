//! Background log capture (B12): every running container's stdio log is
//! saved to the local history DB whether or not a Logs view is open.
//!
//! Verified against Apple container 1.7.1:
//!   * `container logs [-f] <id>` prints the CURRENT RUN's stdio log only.
//!     After `container start` of a stopped container the old run's output
//!     is gone from the CLI; deleting the container makes `logs` fail. The
//!     history DB is the only record of earlier runs.
//!   * `logs -f` never exits when the container stops (it hangs forever),
//!     so followers must be stopped by us.
//!   * `ls` reports `status.startedDate` for a running container and keeps
//!     the LAST run's start date after it stops (absent if never started).
//!
//! # Run identity
//!
//! A run is `(container id, started_at)`, `started_at` being the start time
//! from `ls` (unix seconds). `history::log_runs` stores, per run, how many
//! lines of that run's log have been persisted.
//!
//! # Why resuming is exact
//!
//! The stdio log of one run is APPEND-ONLY, and `container logs` always
//! replays it from the start. So "the first N lines" is the same N lines
//! every time we ask, and the lines we already hold are precisely lines
//! `0..N` of the replay. A follower therefore starts `logs -f <id>` (full
//! replay, no `-n`), drops the first N streamed lines, and persists the
//! rest. There is no race with lines written while no follower ran (they are
//! part of the replay), none with lines written while the follower starts
//! (they follow the replay in the same stream), and nothing depends on
//! timestamps. The counter N is updated by the history writer in the SAME
//! transaction as the rows (see `history::write_log_batch`), so a crash can
//! neither lose nor repeat a line. Both the Logs view's display stream and
//! these followers are independent readers; only the supervisor persists.
//!
//! The same argument covers a stopped container: a one-shot `logs <id>`
//! (no `-f`) replays the run to its end; skipping N lines and persisting
//! the rest completes it (`log_runs.complete` records that, so it is done
//! once per run, even across restarts of cgui).
//!
//! Limits (inherent to the CLI): a restart that happens between two polls
//! destroys the previous run's unread tail, and a log line that is only
//! partially written when read is counted as a (short) line.
//!
//! # Supervisor
//!
//! `reconcile` is driven by the existing poll tick with the container list
//! it already fetched (no extra `ls`). Each tick it diffs "desired" (running
//! containers, newest first, up to `MAX_FOLLOWERS`) against the active
//! tasks (`plan`, a pure function) and starts/stops tasks accordingly.
//! Tasks are tokio futures that own their child with `kill_on_drop`, so
//! aborting a task kills its `logs` process. A task that ends by itself is
//! retried after an exponential backoff.

use crate::history;
use crate::model::Container;
use crate::runtime;
use anyhow::{Context, Result};
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, LazyLock, Mutex, MutexGuard};
use std::time::{Duration, Instant};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, BufReader};
use tokio::sync::Semaphore;
use tokio::task::JoinHandle;

/// Upper bound on concurrent `container logs -f` processes.
pub const MAX_FOLLOWERS: usize = 32;
/// Concurrent one-shot catch-up reads.
const MAX_CATCHUPS: usize = 3;
/// First retry delay after a task fails; doubles per consecutive failure.
const BACKOFF_BASE: Duration = Duration::from_secs(4);
const BACKOFF_MAX: Duration = Duration::from_secs(300);
/// A follower that lived this long was healthy: its failure count resets.
const HEALTHY_AFTER: Duration = Duration::from_secs(30);
/// Catch-up gives up for this session after this many consecutive failures.
const CATCHUP_GIVE_UP: u32 = 5;

// ─── Planning (pure) ──────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Run {
    pub id: String,
    pub started_at: i64,
}

impl Run {
    #[cfg(test)]
    fn new(id: &str, started_at: i64) -> Self {
        Run {
            id: id.into(),
            started_at,
        }
    }
}

/// What the poll tick knows about one container.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Obs {
    pub id: String,
    pub running: bool,
    pub started_at: Option<i64>,
}

impl From<&Container> for Obs {
    fn from(c: &Container) -> Self {
        Obs {
            id: c.id.clone(),
            running: c.status.eq_ignore_ascii_case("running"),
            started_at: c.started_unix,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Kind {
    /// `logs -f`, for a running container.
    Follow,
    /// One-shot `logs`, for a stopped container.
    CatchUp,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActiveTask {
    pub run: Run,
    pub kind: Kind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    /// Abort whatever task runs for this container id.
    Stop(String),
    Follow(Run),
    CatchUp(Run),
}

/// Decide what to start/stop. Pure.
///
/// * `caught_up`: stopped runs already fully captured (never re-read);
/// * `cooling`: tasks in failure backoff (not restarted yet);
/// * all `Stop`s come before any start so the supervisor can chain a
///   restart's new task behind the old one.
pub fn plan(
    obs: &[Obs],
    active: &[ActiveTask],
    caught_up: &HashSet<Run>,
    cooling: &HashSet<(Run, Kind)>,
    cap: usize,
) -> Vec<Action> {
    // Desired followers: running with a known run identity, newest first.
    let mut running: Vec<(&Obs, i64)> = obs
        .iter()
        .filter(|o| o.running)
        .filter_map(|o| o.started_at.map(|s| (o, s)))
        .collect();
    running.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.id.cmp(&b.0.id)));
    running.truncate(cap);
    let desired: HashMap<&str, i64> = running.iter().map(|(o, s)| (o.id.as_str(), *s)).collect();
    let by_id: HashMap<&str, &Obs> = obs.iter().map(|o| (o.id.as_str(), o)).collect();

    let mut stops: Vec<Action> = Vec::new();
    let mut kept: HashSet<(&str, Kind, i64)> = HashSet::new();
    for t in active {
        let id = t.run.id.as_str();
        let keep = match t.kind {
            Kind::Follow => desired.get(id) == Some(&t.run.started_at),
            // A catch-up is only valid while the container stays stopped in
            // the very run it was planned for.
            Kind::CatchUp => by_id
                .get(id)
                .is_some_and(|o| !o.running && o.started_at == Some(t.run.started_at)),
        };
        if keep {
            kept.insert((id, t.kind, t.run.started_at));
        } else {
            stops.push(Action::Stop(t.run.id.clone()));
        }
    }

    let mut starts: Vec<Action> = Vec::new();
    for (o, s) in &running {
        let run = Run {
            id: o.id.clone(),
            started_at: *s,
        };
        if !kept.contains(&(o.id.as_str(), Kind::Follow, *s))
            && !cooling.contains(&(run.clone(), Kind::Follow))
        {
            starts.push(Action::Follow(run));
        }
    }
    for o in obs {
        let (false, Some(s)) = (o.running, o.started_at) else {
            continue; // running (handled above) or never started
        };
        let run = Run {
            id: o.id.clone(),
            started_at: s,
        };
        if caught_up.contains(&run)
            || kept.contains(&(o.id.as_str(), Kind::CatchUp, s))
            || cooling.contains(&(run.clone(), Kind::CatchUp))
        {
            continue;
        }
        starts.push(Action::CatchUp(run));
    }
    stops.extend(starts);
    stops
}

/// Drops the first `n` lines of a replay (those are already stored).
#[derive(Debug, Clone, Copy)]
pub struct Skipper {
    remaining: i64,
}

impl Skipper {
    pub fn new(n: i64) -> Self {
        Skipper {
            remaining: n.max(0),
        }
    }
    /// True if the next line must be persisted.
    pub fn keep_next(&mut self) -> bool {
        if self.remaining > 0 {
            self.remaining -= 1;
            false
        } else {
            true
        }
    }
}

/// Delay before retrying after `failures` consecutive failures (>= 1).
pub fn backoff(failures: u32) -> Duration {
    let exp = failures.saturating_sub(1).min(16);
    BACKOFF_BASE.saturating_mul(1u32 << exp).min(BACKOFF_MAX)
}

/// Strip the line terminator (`\n` or `\r\n`) and decode lossily, so a
/// stray non-UTF-8 byte can never stop a follower or shift its line count.
fn decode_line(raw: &[u8]) -> String {
    let mut end = raw.len();
    if raw.ends_with(b"\n") {
        end -= 1;
        if raw[..end].ends_with(b"\r") {
            end -= 1;
        }
    }
    String::from_utf8_lossy(&raw[..end]).into_owned()
}

// ─── Tasks ────────────────────────────────────────────────────────────

#[derive(Debug)]
enum Outcome {
    /// Catch-up finished (or there was nothing left to do).
    Done,
    /// The container/run is gone; nothing more to capture.
    Gone,
    /// The task ended without being asked to (retry with backoff).
    Failed(String),
    /// Persistence is unavailable (no DB); do not retry noisily.
    Unavailable,
}

enum PumpEnd {
    /// The process ended by itself.
    Eof { success: bool, stderr: String },
    /// The container is no longer in the run the read was planned for.
    Mismatch,
}

/// Confirm that the container is in the run we planned for and in the
/// expected running state. Called AFTER spawning the reader: runs only move
/// forward, so if `inspect` still shows `run` then the log opened a moment
/// earlier was this run's log too.
async fn verify_run(run: &Run, expect_running: bool) -> Result<bool> {
    let (running, started) = runtime::run_info(&run.id).await?;
    Ok(running == expect_running && started == Some(run.started_at))
}

async fn progress_after_flush(run: &Run) -> Option<history::RunProgress> {
    let (id, s) = (run.id.clone(), run.started_at);
    tokio::task::spawn_blocking(move || {
        // Everything a previous task of this run queued must be on disk
        // before its count is read.
        history::flush_logs();
        history::run_progress(&id, s)
    })
    .await
    .ok()
    .flatten()
}

/// Run `argv`, drop the first `skip` lines of stdout and persist the rest
/// as lines of `run`. Output is held back until `verify_run` confirms the
/// run, so a container that restarted since the poll can never put another
/// run's lines under this run's key. Only stdout is the container's log;
/// stderr is the CLI's own diagnostics.
async fn pump(run: &Run, argv: &[&str], skip: i64, expect_running: bool) -> Result<PumpEnd> {
    let mut child = runtime::spawn(argv)?;
    let stdout = child.stdout.take().context("no stdout")?;
    let mut stderr = child.stderr.take().context("no stderr")?;
    let mut out = BufReader::new(stdout);

    let verify = verify_run(run, expect_running);
    tokio::pin!(verify);
    let mut verified = false;
    let mut pending: Vec<String> = Vec::new();
    let mut skipper = Skipper::new(skip);
    let mut raw: Vec<u8> = Vec::new();
    let mut err_tail: Vec<u8> = Vec::new();
    let mut err_done = false;
    let mut err_buf = [0u8; 1024];

    let emit = |line: String, verified: bool, pending: &mut Vec<String>| {
        if verified {
            history::record_run_log(&run.id, run.started_at, &line);
        } else {
            pending.push(line);
        }
    };

    loop {
        tokio::select! {
            r = out.read_until(b'\n', &mut raw) => {
                let n = r?;
                if !raw.is_empty() && skipper.keep_next() {
                    emit(decode_line(&raw), verified, &mut pending);
                }
                raw.clear();
                if n == 0 {
                    break;
                }
            }
            r = stderr.read(&mut err_buf), if !err_done => match r {
                Ok(0) | Err(_) => err_done = true,
                Ok(k) => {
                    err_tail.extend_from_slice(&err_buf[..k]);
                    if err_tail.len() > 2048 {
                        let cut = err_tail.len() - 2048;
                        err_tail.drain(..cut);
                    }
                }
            },
            v = &mut verify, if !verified => {
                if !v? {
                    return Ok(PumpEnd::Mismatch);
                }
                verified = true;
                for l in pending.drain(..) {
                    history::record_run_log(&run.id, run.started_at, &l);
                }
            }
        }
    }
    if !verified {
        if !verify.await? {
            return Ok(PumpEnd::Mismatch);
        }
        for l in pending.drain(..) {
            history::record_run_log(&run.id, run.started_at, &l);
        }
    }
    // Drain what stderr still holds, then reap the process.
    let mut rest = Vec::new();
    let _ = stderr.read_to_end(&mut rest).await;
    err_tail.extend_from_slice(&rest);
    let status = child.wait().await?;
    Ok(PumpEnd::Eof {
        success: status.success(),
        stderr: String::from_utf8_lossy(&err_tail).trim().to_string(),
    })
}

async fn follow_task(run: Run, prev: Option<JoinHandle<()>>) -> Outcome {
    if let Some(p) = prev {
        let _ = p.await; // the previous task of this container is dead
    }
    let Some(progress) = progress_after_flush(&run).await else {
        return Outcome::Unavailable;
    };
    let argv = ["logs", "-f", run.id.as_str()];
    match pump(&run, &argv, progress.persisted_lines, true).await {
        Ok(PumpEnd::Mismatch) => Outcome::Failed("container is no longer in this run".into()),
        Ok(PumpEnd::Eof { success, stderr }) => Outcome::Failed(if success {
            "log follower exited".into()
        } else {
            format!("log follower failed: {stderr}")
        }),
        Err(e) => Outcome::Failed(format!("{e:#}")),
    }
}

async fn catchup_task(run: Run, prev: Option<JoinHandle<()>>, sem: Arc<Semaphore>) -> Outcome {
    if let Some(p) = prev {
        let _ = p.await;
    }
    let Ok(_permit) = sem.acquire_owned().await else {
        return Outcome::Unavailable;
    };
    let Some(progress) = progress_after_flush(&run).await else {
        return Outcome::Unavailable;
    };
    if progress.complete {
        return Outcome::Done;
    }
    let argv = ["logs", run.id.as_str()];
    match pump(&run, &argv, progress.persisted_lines, false).await {
        // Restarted/removed in the meantime: nothing of this run is
        // readable any more; a follower takes over (or it is gone).
        Ok(PumpEnd::Mismatch) => Outcome::Gone,
        Ok(PumpEnd::Eof { success: true, .. }) => {
            // Only mark the run complete if it is still the same, stopped
            // run after the read.
            match verify_run(&run, false).await {
                Ok(true) => {
                    history::mark_run_complete(&run.id, run.started_at);
                    Outcome::Done
                }
                Ok(false) => Outcome::Gone,
                Err(e) => Outcome::Failed(format!("{e:#}")),
            }
        }
        Ok(PumpEnd::Eof { stderr, .. }) => {
            if stderr.contains("not found") || stderr.contains("notFound") {
                Outcome::Gone
            } else {
                Outcome::Failed(format!("logs failed: {stderr}"))
            }
        }
        Err(e) => {
            // `inspect` of a deleted container fails here as well.
            let msg = format!("{e:#}");
            if msg.contains("not found") || msg.contains("notFound") {
                Outcome::Gone
            } else {
                Outcome::Failed(msg)
            }
        }
    }
}

// ─── Supervisor ───────────────────────────────────────────────────────

struct Entry {
    gen: u64,
    run: Run,
    kind: Kind,
    handle: JoinHandle<()>,
}

#[derive(Clone, Copy)]
struct Fail {
    count: u32,
    until: Instant,
}

struct State {
    enabled: bool,
    gen: u64,
    /// One task per container id.
    tasks: HashMap<String, Entry>,
    /// Aborted tasks whose successor must wait for them to be gone.
    dying: HashMap<String, JoinHandle<()>>,
    caught_up: HashSet<Run>,
    fails: HashMap<(Run, Kind), Fail>,
}

pub struct Supervisor {
    state: Mutex<State>,
    catchups: Arc<Semaphore>,
}

/// Snapshot for the UI/Doctor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Status {
    pub enabled: bool,
    pub followers: usize,
    pub catching_up: usize,
}

impl Default for Supervisor {
    fn default() -> Self {
        Self::new()
    }
}

impl Supervisor {
    pub fn new() -> Self {
        Supervisor {
            state: Mutex::new(State {
                enabled: true,
                gen: 0,
                tasks: HashMap::new(),
                dying: HashMap::new(),
                caught_up: HashSet::new(),
                fails: HashMap::new(),
            }),
            catchups: Arc::new(Semaphore::new(MAX_CATCHUPS)),
        }
    }

    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|p| p.into_inner())
    }

    /// Abort every task (kills their `logs` processes) and remember
    /// nothing about them. The next `reconcile` starts fresh tasks that
    /// resume from the DB.
    pub fn stop_all(&self) {
        let mut st = self.lock();
        for (id, e) in st.tasks.drain().collect::<Vec<_>>() {
            e.handle.abort();
            st.dying.insert(id, e.handle);
        }
    }

    /// Runtime on/off switch (Settings). Off stops every task.
    pub fn set_enabled(&self, on: bool) {
        self.lock().enabled = on;
        if !on {
            self.stop_all();
        }
    }

    pub fn status(&self) -> Status {
        let st = self.lock();
        Status {
            enabled: st.enabled,
            followers: st.tasks.values().filter(|e| e.kind == Kind::Follow).count(),
            catching_up: st
                .tasks
                .values()
                .filter(|e| e.kind == Kind::CatchUp)
                .count(),
        }
    }

    /// Bring the tasks in line with the container list from this poll.
    /// Cheap and synchronous; must run inside a tokio runtime.
    pub fn reconcile(self: &Arc<Self>, containers: &[Container]) {
        if !history::ready() {
            return;
        }
        let obs: Vec<Obs> = containers.iter().map(Obs::from).collect();
        let now = Instant::now();
        let mut st = self.lock();
        if !st.enabled {
            return;
        }
        // Forget bookkeeping for containers that no longer exist.
        let ids: HashSet<&str> = obs.iter().map(|o| o.id.as_str()).collect();
        st.caught_up.retain(|r| ids.contains(r.id.as_str()));
        st.fails.retain(|(r, _), _| ids.contains(r.id.as_str()));
        st.dying.retain(|_, h| !h.is_finished());

        let active: Vec<ActiveTask> = st
            .tasks
            .values()
            .map(|e| ActiveTask {
                run: e.run.clone(),
                kind: e.kind,
            })
            .collect();
        let cooling: HashSet<(Run, Kind)> = st
            .fails
            .iter()
            .filter(|(_, f)| f.until > now)
            .map(|(k, _)| k.clone())
            .collect();
        let actions = plan(&obs, &active, &st.caught_up, &cooling, MAX_FOLLOWERS);
        for a in actions {
            match a {
                Action::Stop(id) => {
                    if let Some(e) = st.tasks.remove(&id) {
                        e.handle.abort();
                        st.dying.insert(id, e.handle);
                    }
                }
                Action::Follow(run) => self.launch(&mut st, run, Kind::Follow),
                Action::CatchUp(run) => self.launch(&mut st, run, Kind::CatchUp),
            }
        }
    }

    fn launch(self: &Arc<Self>, st: &mut State, run: Run, kind: Kind) {
        st.gen += 1;
        let gen = st.gen;
        let prev = st.dying.remove(&run.id);
        let me = Arc::clone(self);
        let sem = Arc::clone(&self.catchups);
        let r2 = run.clone();
        let handle = tokio::spawn(async move {
            let started = Instant::now();
            let outcome = match kind {
                Kind::Follow => follow_task(r2.clone(), prev).await,
                Kind::CatchUp => catchup_task(r2.clone(), prev, sem).await,
            };
            me.finish(gen, &r2, kind, outcome, started.elapsed());
        });
        st.tasks.insert(
            run.id.clone(),
            Entry {
                gen,
                run,
                kind,
                handle,
            },
        );
    }

    /// A task ended by itself: deregister it and schedule any retry.
    fn finish(&self, gen: u64, run: &Run, kind: Kind, outcome: Outcome, lived: Duration) {
        let mut st = self.lock();
        if st.tasks.get(&run.id).is_some_and(|e| e.gen == gen) {
            st.tasks.remove(&run.id);
        }
        let key = (run.clone(), kind);
        match outcome {
            Outcome::Done | Outcome::Gone => {
                st.fails.remove(&key);
                if kind == Kind::CatchUp {
                    st.caught_up.insert(run.clone());
                }
            }
            Outcome::Unavailable => {
                // No DB: retry rarely rather than spin.
                st.fails.insert(
                    key,
                    Fail {
                        count: 8,
                        until: Instant::now() + BACKOFF_MAX,
                    },
                );
            }
            Outcome::Failed(msg) => {
                let prev = st.fails.get(&key).map_or(0, |f| f.count);
                let count = if lived >= HEALTHY_AFTER { 1 } else { prev + 1 };
                if kind == Kind::CatchUp && count >= CATCHUP_GIVE_UP {
                    eprintln!("log capture: giving up catch-up of {}: {msg}", run.id);
                    st.caught_up.insert(run.clone());
                    st.fails.remove(&key);
                } else {
                    if count == 1 || count.is_power_of_two() {
                        eprintln!("log capture for {} stopped: {msg}", run.id);
                    }
                    st.fails.insert(
                        key,
                        Fail {
                            count,
                            until: Instant::now() + backoff(count),
                        },
                    );
                }
            }
        }
    }
}

// ─── App-wide instance ────────────────────────────────────────────────

static SUPERVISOR: LazyLock<Arc<Supervisor>> = LazyLock::new(|| Arc::new(Supervisor::new()));

/// Called from the poll tick with the list it just fetched.
pub fn reconcile(containers: &[Container]) {
    SUPERVISOR.reconcile(containers);
}

/// Apply the "Save container logs in the background" setting live.
pub fn set_enabled(on: bool) {
    SUPERVISOR.set_enabled(on);
}

/// App exit: kill every `logs` process.
pub fn stop_all() {
    SUPERVISOR.stop_all();
}

pub fn status() -> Status {
    SUPERVISOR.status()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn o(id: &str, running: bool, started: Option<i64>) -> Obs {
        Obs {
            id: id.into(),
            running,
            started_at: started,
        }
    }
    fn follow_active(id: &str, s: i64) -> ActiveTask {
        ActiveTask {
            run: Run::new(id, s),
            kind: Kind::Follow,
        }
    }
    fn none() -> (HashSet<Run>, HashSet<(Run, Kind)>) {
        (HashSet::new(), HashSet::new())
    }

    #[test]
    fn starts_a_follower_for_each_running_container() {
        let (cu, co) = none();
        let a = plan(
            &[o("a", true, Some(10)), o("b", true, Some(20))],
            &[],
            &cu,
            &co,
            32,
        );
        assert_eq!(
            a,
            vec![
                Action::Follow(Run::new("b", 20)),
                Action::Follow(Run::new("a", 10))
            ]
        );
    }

    #[test]
    fn steady_state_plans_nothing() {
        let (cu, co) = none();
        let a = plan(
            &[o("a", true, Some(10))],
            &[follow_active("a", 10)],
            &cu,
            &co,
            32,
        );
        assert!(a.is_empty());
    }

    #[test]
    fn restart_changes_run_so_old_follower_stops_and_new_one_starts() {
        let (cu, co) = none();
        let a = plan(
            &[o("a", true, Some(99))],
            &[follow_active("a", 10)],
            &cu,
            &co,
            32,
        );
        assert_eq!(
            a,
            vec![Action::Stop("a".into()), Action::Follow(Run::new("a", 99))]
        );
    }

    #[test]
    fn stopped_container_stops_follower_and_gets_a_catch_up() {
        let (cu, co) = none();
        let a = plan(
            &[o("a", false, Some(10))],
            &[follow_active("a", 10)],
            &cu,
            &co,
            32,
        );
        assert_eq!(
            a,
            vec![Action::Stop("a".into()), Action::CatchUp(Run::new("a", 10))]
        );
    }

    #[test]
    fn deleted_container_stops_its_task_and_plans_nothing_else() {
        let (cu, co) = none();
        let a = plan(&[], &[follow_active("gone", 10)], &cu, &co, 32);
        assert_eq!(a, vec![Action::Stop("gone".into())]);
    }

    #[test]
    fn catch_up_is_planned_once_per_run() {
        let (mut cu, co) = none();
        let obs = [o("a", false, Some(10))];
        assert_eq!(
            plan(&obs, &[], &cu, &co, 32),
            vec![Action::CatchUp(Run::new("a", 10))]
        );
        // In flight: not planned again.
        let active = [ActiveTask {
            run: Run::new("a", 10),
            kind: Kind::CatchUp,
        }];
        assert!(plan(&obs, &active, &cu, &co, 32).is_empty());
        // Done: never again for this run...
        cu.insert(Run::new("a", 10));
        assert!(plan(&obs, &[], &cu, &co, 32).is_empty());
        // ...but a later run of the same container, once stopped, is new.
        assert_eq!(
            plan(&[o("a", false, Some(11))], &[], &cu, &co, 32),
            vec![Action::CatchUp(Run::new("a", 11))]
        );
    }

    #[test]
    fn never_started_containers_have_no_run_and_are_ignored() {
        let (cu, co) = none();
        assert!(plan(&[o("new", false, None)], &[], &cu, &co, 32).is_empty());
        // A running container with an unparseable start time cannot be
        // given a run identity either.
        assert!(plan(&[o("odd", true, None)], &[], &cu, &co, 32).is_empty());
    }

    #[test]
    fn catch_up_is_cancelled_when_the_container_starts_again() {
        let (cu, co) = none();
        let active = [ActiveTask {
            run: Run::new("a", 10),
            kind: Kind::CatchUp,
        }];
        let a = plan(&[o("a", true, Some(50))], &active, &cu, &co, 32);
        assert_eq!(
            a,
            vec![Action::Stop("a".into()), Action::Follow(Run::new("a", 50))]
        );
    }

    #[test]
    fn cap_keeps_the_most_recently_started_and_drops_the_rest() {
        let (cu, co) = none();
        let obs: Vec<Obs> = (0..5)
            .map(|i| o(&format!("c{i}"), true, Some(100 + i)))
            .collect();
        let a = plan(&obs, &[], &cu, &co, 3);
        assert_eq!(
            a,
            vec![
                Action::Follow(Run::new("c4", 104)),
                Action::Follow(Run::new("c3", 103)),
                Action::Follow(Run::new("c2", 102)),
            ]
        );
        // A follower that fell out of the top `cap` is stopped.
        let a = plan(&obs, &[follow_active("c0", 100)], &cu, &co, 3);
        assert_eq!(a[0], Action::Stop("c0".into()));
        assert_eq!(a.len(), 4);
    }

    #[test]
    fn cooling_tasks_are_not_restarted_yet() {
        let (cu, mut co) = none();
        co.insert((Run::new("a", 10), Kind::Follow));
        assert!(plan(&[o("a", true, Some(10))], &[], &cu, &co, 32).is_empty());
        // Cooling a catch-up does not block following a later run.
        co.insert((Run::new("b", 5), Kind::CatchUp));
        assert!(plan(&[o("b", false, Some(5))], &[], &cu, &co, 32).is_empty());
    }

    #[test]
    fn skipper_drops_exactly_n_lines() {
        let mut s = Skipper::new(3);
        let kept: Vec<bool> = (0..6).map(|_| s.keep_next()).collect();
        assert_eq!(kept, [false, false, false, true, true, true]);
        let mut z = Skipper::new(0);
        assert!(z.keep_next());
        let mut neg = Skipper::new(-4);
        assert!(neg.keep_next());
    }

    #[test]
    fn resuming_with_the_stored_count_yields_each_line_once() {
        // Model the exactness argument: the replay is append-only, the
        // first `stored` lines are already in the DB.
        let log: Vec<String> = (0..10).map(|i| format!("l{i}")).collect();
        let mut stored: Vec<String> = Vec::new();
        for upto in [4usize, 4, 7, 10] {
            let mut sk = Skipper::new(stored.len() as i64);
            for l in &log[..upto] {
                if sk.keep_next() {
                    stored.push(l.clone());
                }
            }
        }
        assert_eq!(stored, log);
    }

    #[test]
    fn backoff_doubles_and_caps() {
        assert_eq!(backoff(1), Duration::from_secs(4));
        assert_eq!(backoff(2), Duration::from_secs(8));
        assert_eq!(backoff(3), Duration::from_secs(16));
        assert_eq!(backoff(40), BACKOFF_MAX);
    }

    #[test]
    fn decode_line_strips_terminators_and_survives_bad_utf8() {
        assert_eq!(decode_line(b"hi\n"), "hi");
        assert_eq!(decode_line(b"hi\r\n"), "hi");
        assert_eq!(decode_line(b"partial"), "partial");
        assert_eq!(decode_line(b"\n"), "");
        assert!(decode_line(b"a\xffb\n").contains('\u{fffd}'));
    }

    #[tokio::test]
    async fn set_enabled_false_aborts_every_task() {
        let sup = Arc::new(Supervisor::new());
        let h = tokio::spawn(async { tokio::time::sleep(Duration::from_secs(3600)).await });
        let probe = h.abort_handle();
        sup.lock().tasks.insert(
            "a".into(),
            Entry {
                gen: 1,
                run: Run::new("a", 1),
                kind: Kind::Follow,
                handle: h,
            },
        );
        assert_eq!(sup.status().followers, 1);
        sup.set_enabled(false);
        assert_eq!(sup.status().followers, 0);
        assert!(!sup.status().enabled);
        tokio::time::sleep(Duration::from_millis(50)).await;
        assert!(probe.is_finished(), "task aborted");
        sup.set_enabled(true);
        assert!(sup.status().enabled);
    }

    #[test]
    fn failures_back_off_and_catch_up_gives_up() {
        // `finish` only touches bookkeeping, so it can run without tokio.
        let sup = Supervisor::new();
        let run = Run::new("a", 1);
        for n in 1..=3u32 {
            sup.finish(
                n as u64,
                &run,
                Kind::Follow,
                Outcome::Failed("x".into()),
                Duration::from_millis(10),
            );
            assert_eq!(sup.lock().fails[&(run.clone(), Kind::Follow)].count, n);
        }
        // A long-lived follower that then fails starts over at 1.
        sup.finish(
            9,
            &run,
            Kind::Follow,
            Outcome::Failed("x".into()),
            HEALTHY_AFTER,
        );
        assert_eq!(sup.lock().fails[&(run.clone(), Kind::Follow)].count, 1);

        for n in 1..=CATCHUP_GIVE_UP {
            assert!(!sup.lock().caught_up.contains(&run), "attempt {n}");
            sup.finish(
                10,
                &run,
                Kind::CatchUp,
                Outcome::Failed("x".into()),
                Duration::ZERO,
            );
        }
        assert!(
            sup.lock().caught_up.contains(&run),
            "gave up for this session"
        );

        // Success is remembered so the run is never re-read.
        let r2 = Run::new("b", 2);
        sup.finish(11, &r2, Kind::CatchUp, Outcome::Done, Duration::ZERO);
        assert!(sup.lock().caught_up.contains(&r2));
    }
}
