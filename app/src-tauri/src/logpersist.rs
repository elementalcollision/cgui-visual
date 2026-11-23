//! Background log capture (B12): every running container's stdio log is
//! saved to the local history DB whether or not a Logs view is open.
//!
//! Verified against Apple container 1.7.1:
//!   * `container logs [-f] <id>` prints the CURRENT RUN's stdio log only.
//!     After `container start` of a stopped container the old run's output
//!     is gone from the CLI; deleting the container makes `logs` fail. The
//!     history DB is the only record of earlier runs.
//!   * `logs -f` never exits when the container stops (it hangs forever),
//!     so followers must be stopped by us. If the container is started
//!     again, it streams the NEW run's lines after the old run's.
//!   * What `logs -f` streams while the log is being written is not always
//!     the log's lines. When its read of the log ends inside a line, it
//!     prints that much as a line and the rest as the next one (a burst of
//!     long lines can come out with dozens of lines split in two). And it
//!     leaves blank lines out. The text is intact. What it replays (the
//!     part written before it started), and what a one-shot `logs` prints,
//!     are the log's lines exactly.
//!   * `ls` and `inspect` report `status.startedDate`, in whole seconds,
//!     for a running container and keep the LAST run's start date after it
//!     stops (absent if never started). Nothing finer identifies a run, and
//!     a container that exits at once can be started again within the same
//!     second: both runs then have the same start date.
//!   * `container start` empties the log about 0.3 s before `inspect`
//!     reports the new run; in between, `logs` returns the new run's empty
//!     log while `inspect` still shows the old run, stopped. (The new run
//!     printed nothing before `inspect` showed it running.)
//!
//! # Position in a run's log
//!
//! The rows stored for a run are therefore its log's lines, except that a
//! line `logs -f` split is two rows and a blank line it left out is
//! missing. So the row count is not a position in the next replay (a split
//! makes it one too high, a left-out blank line one too low), and resuming
//! by it would lose or repeat a line for each. The position is the run's
//! TEXT BYTES instead: the raw bytes of its rows, each without its final
//! `\n`, before UTF-8 decoding (a split can fall inside a character, or
//! between a `\r` and its `\n`). Splits and left-out blank lines leave them
//! unchanged. Blank lines have no text bytes, so the position also keeps how
//! many blank rows the stored rows end with; those can only come from a
//! replay, so they are the log's blank lines at that point.
//!
//! # Run identity
//!
//! The poll knows a run as `(container id, started_at)`, `started_at` being
//! the start date (unix seconds). That key is ambiguous only for runs that
//! start within the same second, so the history DB stores a run as
//! `(container id, started_at, seq)`, seq 0 being the first run with that
//! key, 1 the next, and so on (`history::log_runs`). Per stored run it
//! keeps the position (text bytes P, trailing blank rows) and a fingerprint
//! of the text bytes, line breaks left out, so splits do not change it
//! either (`history::RunDigest`). Which stored run a replay of the CLI's log
//! belongs to is decided from its content:
//!
//!   * A task continues the latest stored run of its key. While it drops
//!     the replay's stored part, it checks it against the fingerprint
//!     (`Resume`: after text bytes 1, 2, 4, 8, … and at P). A replay that
//!     differs, that ends before P (but is not empty), or that goes on past
//!     the end of a run already read to its end (`complete`), is a later
//!     run with the same key: it becomes the next seq and is read again
//!     from its first line. Nothing is persisted from a replay before it
//!     has passed the check, so a mismatch stores nothing twice. A follower
//!     acts on a mismatch only with the container confirmed running in the
//!     planned run, a catch-up only with it confirmed still stopped in it
//!     after the read (a read that overlapped a start may have seen the
//!     next run's log, see above).
//!   * A later run can take a run's key only while that second lasts. A
//!     follower therefore waits until the key is settled
//!     (`KEY_SETTLE_SECS`) before it opens `logs -f`: one opened earlier
//!     could stream on into a same-second successor (see above) and store
//!     it as the old run. Nothing is lost by waiting: the replay starts at
//!     line 1.
//!   * A catch-up reads a stopped run at once (starting the container
//!     again would destroy its log). If that read began before the key was
//!     settled, a same-second successor could still run and stop between
//!     two polls, unseen, so the run is marked `recheck` and its log read
//!     once more after the key has settled.
//!
//! Known limit: a later run whose log begins with exactly the text we hold
//! for the earlier run (and has nothing more, if the earlier run is
//! complete) cannot be told from it; line breaks are not compared. It is
//! then stored as the same run: the text is all there, but what both runs
//! printed identically is stored once rather than twice. Runs saved before
//! text bytes were recorded (`text_bytes` NULL) are resumed by row count,
//! as before.
//!
//! # Why resuming is exact
//!
//! The stdio log of one run is APPEND-ONLY, and `container logs` always
//! replays it from the start, unsplit. So "the log up to text byte P" is the
//! same every time we ask, and the rows we hold are precisely that part of
//! the replay (the fingerprint check confirms that it is the same run). A
//! follower therefore starts `logs -f <id>` (full replay, no `-n`), drops
//! the replay up to text byte P and then as many blank lines as the stored
//! rows end with, and persists the rest. If P falls inside a line (the
//! stored rows ended with the first piece of a split line), the rest of
//! that line is persisted as a row of its own. There is no race with lines
//! written while no follower ran (they are part of the replay), none with
//! lines written while the follower starts (they follow the replay in the
//! same stream), and nothing depends on timestamps beyond telling runs apart
//! as above. The position and fingerprint are updated by the history writer
//! in the SAME transaction as the rows (see `history::write_log_batch`), so
//! a crash can neither lose nor repeat a line. Both the Logs view's display
//! stream and these followers are independent readers; only the supervisor
//! persists.
//!
//! The same argument covers a stopped container: a one-shot `logs <id>`
//! (no `-f`) replays the run to its end; skipping to P and persisting the
//! rest completes it (`log_runs.complete` records that, so it is done once
//! per run, even across restarts of cgui).
//!
//! # Restarts between polls
//!
//! A follower's `logs -f` streams on into the next run after a restart (see
//! above), and nothing tells the follower: if the container is stopped and
//! started again between two polls (the Restart button does exactly that),
//! the new run's first lines reach the old run's follower before the next
//! poll stops it. Stored as they came, they would be stored twice: under
//! the old run, and again by the new run's follower, which replays its run
//! from line 1.
//!
//! So a follower holds what it reads (`Unconfirmed`) and stores a row only
//! once a check that BEGAN AFTER the row was read has found the container
//! still running in the follower's run. The checks are the polls themselves
//! (the supervisor passes each poll's start to the followers it keeps, so
//! no extra process runs), and an `inspect` of the follower's own when it
//! holds a lot (`CONFIRM_SIZE`). The key is settled by the time a follower
//! starts, so "still in this run" can only mean the same run, and every row
//! read before the check came from it. Stored rows therefore lag the log by
//! up to one poll interval.
//!
//! When a poll finds the container out of the run, the supervisor tells the
//! follower to stop instead of killing it, and the follower wraps up
//! (`wrap_up`) before the container's next task starts:
//!   * if `inspect` still shows its run (stopped), the rows it holds are all
//!     the run's (a starting run prints nothing before `inspect` shows it,
//!     see above): it stores them;
//!   * if the container has moved on to a later run, the rows it holds are
//!     the old run's last lines (say, its shutdown messages) followed by the
//!     beginning of the new run. It reads the current log once, drops the
//!     held rows that are its beginning (`old_run_rows`, by text bytes: the
//!     stream it read the new run from may have split lines), and stores the
//!     rest.
//!
//! Limits (inherent to the CLI):
//!   * a restart that happens between two polls destroys the previous
//!     run's unread tail;
//!   * if the old run's last lines and the new run's first lines are the
//!     same text, the wrap-up may count some of the old run's as the new
//!     run's: that text is then stored once, under the new run;
//!   * rows a follower holds when cgui quits or capture is switched off are
//!     dropped; the next follower replays them from the log, unless the
//!     container is restarted before then;
//!   * a line `logs -f` split while following stays two rows (together,
//!     the line), and a blank line it left out stays missing;
//!   * a follower leaves an unterminated last line for the next read (it
//!     may still grow); a catch-up of a stopped container stores it as is.
//!
//! # Supervisor
//!
//! `reconcile` is driven by the existing poll tick with the container list
//! it already fetched (no extra `ls`). Each tick it diffs "desired" (running
//! containers, newest first, up to `MAX_FOLLOWERS`) against the active
//! tasks (`plan`, a pure function) and starts/stops tasks accordingly, and
//! confirms the followers it keeps (see above). Tasks are tokio futures that
//! own their child with `kill_on_drop`: a catch-up is stopped by aborting
//! it, which kills its `logs` process; a follower is told to stop and wraps
//! up first. A container's next task waits for the previous one to end. A
//! task that ends by itself is retried after an exponential backoff.

use crate::history;
use crate::model::Container;
use crate::runtime;
use anyhow::{Context, Result};
use std::collections::{HashMap, HashSet, VecDeque};
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, LazyLock, Mutex, MutexGuard};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, BufReader};
use tokio::sync::{watch, Semaphore};
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
/// A later run can share a run's key (its start second) only by starting
/// within that second; once it is over, the key is settled. One more
/// second covers rounding of `startedDate`.
const KEY_SETTLE_SECS: i64 = 2;
/// Reads per task: one, plus one per later run found with the same key,
/// plus one recheck. More means the history DB is not taking writes.
const MAX_PASSES: usize = 4;
/// A follower holding this much unconfirmed (text bytes plus one per row)
/// checks its run with `inspect` itself instead of waiting for the next
/// poll (see "Restarts between polls").
const CONFIRM_SIZE: usize = 1 << 20;
/// Bound on a follower's wrap-up (one `inspect`, maybe one `logs`).
const WRAP_UP_TIMEOUT: Duration = Duration::from_secs(15);

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

/// What to do with the next line of a replay.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    /// Already stored: drop it.
    Skip,
    /// Not stored yet: persist its text from this byte on (0 but for the
    /// line the stored rows end inside of).
    Keep(usize),
    /// The replay is not the stored run but a later run with the same key.
    Diverged,
}

/// Matches a replay of the CLI's log against the stored run it should
/// continue: drops what is already stored after checking that it is what
/// was stored (see "Run identity" and "Why resuming is exact").
#[derive(Debug, Clone)]
pub struct Resume {
    /// Where the stored rows end: text bytes, or, for a run saved before
    /// those were recorded, rows (counted as replay lines, as before).
    stored: Stored,
    /// Blank lines still to drop once the stored text is passed: the
    /// stored rows end with this many.
    empties: i64,
    /// The stored run was read to its end: it has no further line.
    complete: bool,
    /// Text bytes and fingerprint of the replay so far, up to the stored
    /// end.
    seen: history::RunDigest,
    /// Replay lines so far.
    lines: i64,
    /// Past the stored end: everything from here on is new.
    past: bool,
}

#[derive(Debug, Clone)]
enum Stored {
    Text(history::RunDigest),
    Rows(i64),
}

impl Resume {
    pub fn new(p: &history::RunProgress) -> Self {
        Resume {
            stored: match &p.digest {
                Some(d) => Stored::Text(d.clone()),
                None => Stored::Rows(p.persisted_lines.max(0)),
            },
            empties: p.trailing_empty,
            complete: p.complete,
            seen: history::RunDigest::default(),
            lines: 0,
            past: false,
        }
    }

    /// Feed the next replay line's raw text (without its `\n`).
    pub fn next(&mut self, text: &[u8]) -> Step {
        self.lines += 1;
        if !self.past {
            match &self.stored {
                Stored::Rows(n) => {
                    if self.lines <= *n {
                        return Step::Skip;
                    }
                }
                Stored::Text(d) => {
                    let left = (d.bytes - self.seen.bytes).max(0) as usize;
                    if left > 0 {
                        let take = left.min(text.len());
                        for &b in &text[..take] {
                            self.seen.push(&[b]);
                            let n = self.seen.bytes;
                            let checkpoint = if (n as u64).is_power_of_two() {
                                d.checkpoints.get(n.trailing_zeros() as usize)
                            } else {
                                None
                            };
                            if checkpoint.is_some_and(|&c| c != self.seen.hash)
                                || (n == d.bytes && d.hash != self.seen.hash)
                            {
                                return Step::Diverged;
                            }
                        }
                        if take == text.len() {
                            return Step::Skip;
                        }
                        // The stored rows end inside this line (the CLI
                        // split it): the rest of it is new.
                        self.past = true;
                        return if self.complete {
                            Step::Diverged
                        } else {
                            Step::Keep(take)
                        };
                    }
                    if self.empties > 0 && text.is_empty() {
                        self.empties -= 1;
                        return Step::Skip;
                    }
                }
            }
            self.past = true;
        }
        if self.complete {
            Step::Diverged
        } else {
            Step::Keep(0)
        }
    }

    /// For a replay that has ended (a stopped container's log): it is
    /// shorter than the stored run, so it is a later run (a run's log only
    /// grows). An empty replay does not count: while `container start` is
    /// under way, `logs` already returns the new run's (empty) log but
    /// `inspect` still shows the old run as stopped. It has nothing to
    /// store either way.
    pub fn ended_short(&self) -> bool {
        self.lines > 0
            && match &self.stored {
                Stored::Text(d) => self.seen.bytes < d.bytes,
                Stored::Rows(n) => self.lines < *n,
            }
    }
}

/// The raw text of a streamed line: without its final `\n`. A `\r` before
/// it stays: `logs -f` can split a line between the two, and the pieces
/// must add up to the line.
fn text_of(raw: &[u8]) -> &[u8] {
    raw.strip_suffix(b"\n").unwrap_or(raw)
}

/// How many of `rows` (a follower's unconfirmed rows, oldest first) belong
/// to its run, given `now`, the text of the container's current log (line
/// breaks left out) after it moved on to a later run: the rows after that
/// point are the beginning of `now`, which `logs -f` streamed on into (see
/// "Restarts between polls"). Text bytes are compared, line breaks left
/// out, since `logs -f` may split lines. The longest such ending that
/// starts at a row wins; blank rows right at that point count as the old
/// run's. If no ending matches, all rows are the old run's.
fn old_run_rows(rows: &[&[u8]], now: &[u8]) -> usize {
    let held: Vec<u8> = rows.concat();
    let now = &now[..now.len().min(held.len())];
    if now.is_empty() {
        return rows.len();
    }
    // KMP: `border[i]` is the longest proper prefix of now[..=i] that is
    // also its suffix.
    let mut border = vec![0usize; now.len()];
    for i in 1..now.len() {
        let mut k = border[i - 1];
        while k > 0 && now[i] != now[k] {
            k = border[k - 1];
        }
        if now[i] == now[k] {
            k += 1;
        }
        border[i] = k;
    }
    // The longest prefix of `now` that `held` ends with.
    let mut k = 0;
    for &b in &held {
        while k > 0 && (k == now.len() || b != now[k]) {
            k = border[k - 1];
        }
        if k < now.len() && b == now[k] {
            k += 1;
        }
    }
    // Shorter matches are its borders; take the longest starting at a row.
    let mut starts = Vec::with_capacity(rows.len() + 1);
    let mut at = 0;
    starts.push(0);
    for r in rows {
        at += r.len();
        starts.push(at);
    }
    while k > 0 {
        let cut = held.len() - k;
        // The most rows whose text ends at `cut`.
        let n = starts.partition_point(|&s| s <= cut) - 1;
        if starts[n] == cut {
            return n;
        }
        k = border[k - 1];
    }
    rows.len()
}

/// Whether, at unix time `at`, no later run can take the key `started_at`.
fn key_settled(started_at: i64, at: i64) -> bool {
    at >= started_at.saturating_add(KEY_SETTLE_SECS)
}

/// Time until the key `started_at` is settled (zero if it is). Capped, in
/// case the start date is ahead of our clock.
fn settle_delay(started_at: i64) -> Duration {
    let settled = started_at.saturating_add(KEY_SETTLE_SECS).max(0) as u64;
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    Duration::from_secs(settled)
        .saturating_sub(now)
        .min(Duration::from_secs(KEY_SETTLE_SECS as u64 + 1))
}

/// Delay before retrying after `failures` consecutive failures (>= 1).
pub fn backoff(failures: u32) -> Duration {
    let exp = failures.saturating_sub(1).min(16);
    BACKOFF_BASE.saturating_mul(1u32 << exp).min(BACKOFF_MAX)
}

// ─── Tasks ────────────────────────────────────────────────────────────

#[derive(Debug)]
enum Outcome {
    /// Catch-up finished (or there was nothing left to do).
    Done,
    /// The container/run is gone; nothing more to capture.
    Gone,
    /// The container left the run the task was planned for (it stopped,
    /// or was started again). The next poll plans what is needed; a later
    /// run may have the same key, so this run is not "caught up".
    Moved,
    /// The task ended without being asked to (retry with backoff).
    Failed(String),
    /// Persistence is unavailable (no DB); do not retry noisily.
    Unavailable,
}

enum PumpEnd {
    /// The process ended by itself. `short`: before the stored text did.
    Eof {
        success: bool,
        stderr: String,
        short: bool,
    },
    /// The container is no longer in the run the read was planned for.
    Mismatch,
    /// The replay is a later run with the same key than the stored one
    /// (the container is confirmed in the planned run). Nothing was stored.
    Diverged,
    /// A follower was told to stop, or found the container out of its run;
    /// it has wrapped up.
    Ended,
}

/// Confirm that the container is in the run we planned for and in the
/// expected running state. Called AFTER spawning the reader: runs only move
/// forward, so if `inspect` still shows `run` then the log opened a moment
/// earlier was this run's log too. The exception is a start under way (see
/// the module notes): the log may then already be the next run's, still
/// empty, which `Resume::ended_short` and `catchup_task` allow for.
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

/// One-shot `logs` of a stopped container, for a catch-up: match it against
/// stored run `seq` with `resume`, and persist what it keeps as rows of
/// (`run`, `seq`). Output is held back until `verify_run` confirms the run,
/// so a container that restarted since the poll can never put another run's
/// lines under this run's key. Only stdout is the container's log; stderr is
/// the CLI's own diagnostics.
async fn pump(run: &Run, seq: i64, mut resume: Resume) -> Result<PumpEnd> {
    let mut child = runtime::spawn(&["logs", run.id.as_str()])?;
    let stdout = child.stdout.take().context("no stdout")?;
    let mut stderr = child.stderr.take().context("no stderr")?;
    let mut out = BufReader::new(stdout);

    let verify = verify_run(run, false);
    tokio::pin!(verify);
    let mut verified = false;
    let mut diverged = false;
    let mut pending: Vec<Vec<u8>> = Vec::new();
    let mut raw: Vec<u8> = Vec::new();
    let mut err_tail: Vec<u8> = Vec::new();
    let mut err_done = false;
    let mut err_buf = [0u8; 1024];

    let emit = |text: Vec<u8>, verified: bool, pending: &mut Vec<Vec<u8>>| {
        if verified {
            history::record_run_log(&run.id, run.started_at, seq, text);
        } else {
            pending.push(text);
        }
    };

    loop {
        tokio::select! {
            r = out.read_until(b'\n', &mut raw) => {
                let n = r?;
                // The log of a stopped container is final: an unterminated
                // last line is complete too.
                if !raw.is_empty() {
                    let text = text_of(&raw);
                    match resume.next(text) {
                        Step::Skip => {}
                        Step::Keep(from) => emit(text[from..].to_vec(), verified, &mut pending),
                        Step::Diverged => {
                            diverged = true;
                            break;
                        }
                    }
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
                for t in pending.drain(..) {
                    history::record_run_log(&run.id, run.started_at, seq, t);
                }
            }
        }
    }
    if !verified {
        if !verify.await? {
            return Ok(PumpEnd::Mismatch);
        }
        for t in pending.drain(..) {
            history::record_run_log(&run.id, run.started_at, seq, t);
        }
    }
    if diverged {
        return Ok(PumpEnd::Diverged); // the child is killed on drop
    }
    // Drain what stderr still holds, then reap the process.
    let mut rest = Vec::new();
    let _ = stderr.read_to_end(&mut rest).await;
    err_tail.extend_from_slice(&rest);
    let status = child.wait().await?;
    Ok(PumpEnd::Eof {
        success: status.success(),
        stderr: String::from_utf8_lossy(&err_tail).trim().to_string(),
        short: resume.ended_short(),
    })
}

/// What the supervisor tells a follower (see "Restarts between polls").
#[derive(Debug, Clone, Copy, Default)]
struct FollowCtl {
    /// A poll that began at this instant found the container still running
    /// in the follower's run: every line read before then is the run's.
    confirmed: Option<Instant>,
    /// Stop following and wrap up.
    stop: bool,
}

/// Rows a follower has read but not stored yet, oldest first, each with
/// when it was read. A row is stored once a check that began after it was
/// read has found the container still in the run (see "Restarts between
/// polls"). Rows still held when the task is aborted (cgui quits, capture
/// is switched off) are dropped; the run's log still has them, and the next
/// follower replays them from the stored position.
struct Unconfirmed {
    run: Run,
    seq: i64,
    rows: VecDeque<(Instant, Vec<u8>)>,
    /// Text bytes plus one per row.
    size: usize,
}

impl Unconfirmed {
    fn new(run: &Run, seq: i64) -> Self {
        Unconfirmed {
            run: run.clone(),
            seq,
            rows: VecDeque::new(),
            size: 0,
        }
    }

    fn push(&mut self, text: Vec<u8>) {
        self.size += text.len() + 1;
        self.rows.push_back((Instant::now(), text));
    }

    fn store_front(&mut self) {
        if let Some((_, text)) = self.rows.pop_front() {
            self.size -= text.len() + 1;
            history::record_run_log(&self.run.id, self.run.started_at, self.seq, text);
        }
    }

    /// Store every row read before `at`.
    fn confirm(&mut self, at: Instant) {
        while self.rows.front().is_some_and(|(t, _)| *t < at) {
            self.store_front();
        }
    }

    /// Store the first `n` rows and drop the rest.
    fn keep_first(&mut self, n: usize) {
        for _ in 0..n {
            self.store_front();
        }
        self.rows.clear();
        self.size = 0;
    }
}

/// After a follower stops reading (told to, or its own check found the
/// container out of its run), store the rows it holds that are its run's.
/// If `inspect` still shows the run (stopped), that is all of them. If the
/// container has moved on to a later run, the held rows are the run's last
/// lines followed by what `logs -f` streamed on into: the beginning of the
/// current log. Those are dropped (the new run's task stores them) and the
/// rest stored. If neither can be told (no answer, container deleted), all
/// are stored, as before this check.
async fn wrap_up(run: &Run, un: &mut Unconfirmed) {
    if un.rows.is_empty() {
        return;
    }
    let rows: Vec<&[u8]> = un.rows.iter().map(|(_, t)| t.as_slice()).collect();
    let keep = tokio::time::timeout(WRAP_UP_TIMEOUT, async {
        let (_, started) = runtime::run_info(&run.id).await?;
        if started == Some(run.started_at) {
            return anyhow::Ok(rows.len());
        }
        let log = runtime::run_with_timeout(&["logs", run.id.as_str()], WRAP_UP_TIMEOUT).await?;
        let now: Vec<u8> = log.into_iter().filter(|&b| b != b'\n').collect();
        Ok(old_run_rows(&rows, &now))
    })
    .await;
    let n = match keep {
        Ok(Ok(n)) => n,
        _ => un.rows.len(),
    };
    un.keep_first(n);
}

/// Resolves once the supervisor tells the follower to stop.
async fn told_to_stop(ctl: &mut watch::Receiver<FollowCtl>) {
    loop {
        if ctl.borrow_and_update().stop {
            return;
        }
        if ctl.changed().await.is_err() {
            return;
        }
    }
}

/// `logs -f` of a running container, for a follower: like `pump`, but a
/// row is stored only once confirmed (`Unconfirmed`), and the follower stops
/// when the supervisor says so, then wraps up (`wrap_up`). `Ended`: it was
/// told to stop, or found the container out of its run.
async fn follow_pump(
    run: &Run,
    seq: i64,
    mut resume: Resume,
    ctl: &mut watch::Receiver<FollowCtl>,
) -> Result<PumpEnd> {
    if ctl.borrow().stop {
        return Ok(PumpEnd::Ended);
    }
    let mut child = runtime::spawn(&["logs", "-f", run.id.as_str()])?;
    let stdout = child.stdout.take().context("no stdout")?;
    let mut stderr = child.stderr.take().context("no stderr")?;
    let mut out = BufReader::new(stdout);

    // Confirms the log just opened is this run's (a mismatch drops all).
    let verify = verify_run(run, true);
    tokio::pin!(verify);
    let mut verified = false;
    let mut un = Unconfirmed::new(run, seq);
    // An `inspect` of our own, begun at the instant it holds.
    type Check<'a> = (
        Instant,
        Pin<Box<dyn Future<Output = Result<bool>> + Send + 'a>>,
    );
    let mut check: Option<Check> = None;
    let mut raw: Vec<u8> = Vec::new();
    let mut err_tail: Vec<u8> = Vec::new();
    let mut err_done = false;
    let mut err_buf = [0u8; 1024];

    enum Why {
        Eof,
        Ended,
        Diverged,
    }
    let why = loop {
        tokio::select! {
            r = out.read_until(b'\n', &mut raw) => {
                let n = r?;
                // `logs -f` output only ends when its process does; an
                // unterminated last line may still grow, so it is left to
                // the next read.
                if raw.ends_with(b"\n") {
                    let text = text_of(&raw);
                    match resume.next(text) {
                        Step::Skip => {}
                        Step::Keep(from) => {
                            un.push(text[from..].to_vec());
                            if un.size >= CONFIRM_SIZE && check.is_none() {
                                check = Some((Instant::now(), Box::pin(verify_run(run, true))));
                            }
                        }
                        Step::Diverged => break Why::Diverged,
                    }
                }
                raw.clear();
                if n == 0 {
                    break Why::Eof;
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
            }
            c = ctl.changed() => {
                let Ok(()) = c else { break Why::Ended };
                let c = *ctl.borrow_and_update();
                if let Some(at) = c.confirmed {
                    un.confirm(at);
                }
                if c.stop {
                    break Why::Ended;
                }
            }
            v = async {
                match check.as_mut() {
                    Some((_, f)) => f.await,
                    None => std::future::pending().await,
                }
            }, if check.is_some() => {
                let at = check.take().map(|(t, _)| t).expect("check in flight");
                if !v? {
                    break Why::Ended; // the container left the run
                }
                un.confirm(at);
            }
        }
    };
    match why {
        Why::Diverged => {
            // Only while matching the stored text: nothing is held.
            if !verified && !verify.await? {
                return Ok(PumpEnd::Mismatch);
            }
            Ok(PumpEnd::Diverged) // the child is killed on drop
        }
        Why::Ended => {
            drop(out);
            drop(child); // stop reading: kills `logs -f`
            wrap_up(run, &mut un).await;
            Ok(PumpEnd::Ended)
        }
        Why::Eof => {
            wrap_up(run, &mut un).await;
            let mut rest = Vec::new();
            let _ = stderr.read_to_end(&mut rest).await;
            err_tail.extend_from_slice(&rest);
            let status = child.wait().await?;
            Ok(PumpEnd::Eof {
                success: status.success(),
                stderr: String::from_utf8_lossy(&err_tail).trim().to_string(),
                short: false,
            })
        }
    }
}

async fn follow_task(
    run: Run,
    prev: Option<JoinHandle<()>>,
    mut ctl: watch::Receiver<FollowCtl>,
) -> Outcome {
    if let Some(p) = prev {
        let _ = p.await; // the previous task of this container is done
    }
    // Only open `logs -f` once no later run can take this key, or it could
    // stream on into such a run (see "Run identity").
    tokio::select! {
        _ = tokio::time::sleep(settle_delay(run.started_at)) => {}
        _ = told_to_stop(&mut ctl) => return Outcome::Moved,
    }
    for _ in 0..MAX_PASSES {
        let Some(progress) = progress_after_flush(&run).await else {
            return Outcome::Unavailable;
        };
        match follow_pump(&run, progress.seq, Resume::new(&progress), &mut ctl).await {
            // A later run with this key (the container was confirmed
            // running in it): store it as the next seq, from its first line.
            Ok(PumpEnd::Diverged) => history::start_run(&run.id, run.started_at, progress.seq + 1),
            Ok(PumpEnd::Mismatch) | Ok(PumpEnd::Ended) => return Outcome::Moved,
            Ok(PumpEnd::Eof {
                success, stderr, ..
            }) => {
                return Outcome::Failed(if success {
                    "log follower exited".into()
                } else {
                    format!("log follower failed: {stderr}")
                })
            }
            Err(e) => return Outcome::Failed(format!("{e:#}")),
        }
    }
    Outcome::Failed("could not record a new run".into())
}

async fn catchup_task(run: Run, prev: Option<JoinHandle<()>>, sem: Arc<Semaphore>) -> Outcome {
    if let Some(p) = prev {
        let _ = p.await;
    }
    for _ in 0..MAX_PASSES {
        let Some(progress) = progress_after_flush(&run).await else {
            return Outcome::Unavailable;
        };
        if progress.complete {
            if !progress.recheck {
                return Outcome::Done;
            }
            // Read it once more when no later run can take its key.
            tokio::time::sleep(settle_delay(run.started_at)).await;
        }
        let Ok(_permit) = Arc::clone(&sem).acquire_owned().await else {
            return Outcome::Unavailable;
        };
        let read_at = crate::timeutil::now_unix();
        let end = pump(&run, progress.seq, Resume::new(&progress)).await;
        // Act on what was read only if the container is still in the same,
        // stopped run after the read: a read that overlapped a start may
        // have seen the next run's log before `inspect` did.
        let later_run = match end {
            // Restarted/removed in the meantime.
            Ok(PumpEnd::Mismatch) | Ok(PumpEnd::Ended) => return Outcome::Moved,
            Ok(PumpEnd::Diverged) => true,
            Ok(PumpEnd::Eof {
                success: true,
                short,
                ..
            }) => short,
            Ok(PumpEnd::Eof { stderr, .. }) => {
                return if stderr.contains("not found") || stderr.contains("notFound") {
                    Outcome::Gone
                } else {
                    Outcome::Failed(format!("logs failed: {stderr}"))
                };
            }
            Err(e) => {
                // `inspect` of a deleted container fails here as well.
                let msg = format!("{e:#}");
                return if msg.contains("not found") || msg.contains("notFound") {
                    Outcome::Gone
                } else {
                    Outcome::Failed(msg)
                };
            }
        };
        match verify_run(&run, false).await {
            Ok(true) => {}
            Ok(false) => return Outcome::Moved,
            Err(e) => return Outcome::Failed(format!("{e:#}")),
        }
        if later_run {
            // A later run with this key: store it as the next seq, from its
            // first line.
            history::start_run(&run.id, run.started_at, progress.seq + 1);
            continue;
        }
        let recheck = !progress.complete && !key_settled(run.started_at, read_at);
        history::mark_run_complete(&run.id, run.started_at, progress.seq, recheck);
        if !recheck {
            return Outcome::Done;
        }
    }
    Outcome::Failed("could not record a new run".into())
}

// ─── Supervisor ───────────────────────────────────────────────────────

struct Entry {
    gen: u64,
    run: Run,
    kind: Kind,
    handle: JoinHandle<()>,
    /// Followers only: confirmations and the stop signal.
    ctl: Option<watch::Sender<FollowCtl>>,
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
    /// Stopped tasks whose successor must wait for them to be gone:
    /// aborted catch-ups, followers wrapping up.
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

    /// Abort every task, followers wrapping up included (kills their
    /// `logs` processes), and remember nothing about them. The next
    /// `reconcile` starts fresh tasks that resume from the DB.
    pub fn stop_all(&self) {
        let mut st = self.lock();
        for h in st.dying.values() {
            h.abort();
        }
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

    /// Bring the tasks in line with the container list from this poll,
    /// whose listing began at `polled_at`. Cheap and synchronous; must run
    /// inside a tokio runtime.
    pub fn reconcile(self: &Arc<Self>, containers: &[Container], polled_at: Instant) {
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
                Action::Stop(id) => stop_task(&mut st, &id),
                Action::Follow(run) => self.launch(&mut st, run, Kind::Follow),
                Action::CatchUp(run) => self.launch(&mut st, run, Kind::CatchUp),
            }
        }
        confirm_followers(&st.tasks, &obs, polled_at);
    }

    fn launch(self: &Arc<Self>, st: &mut State, run: Run, kind: Kind) {
        st.gen += 1;
        let gen = st.gen;
        let prev = st.dying.remove(&run.id);
        let me = Arc::clone(self);
        let sem = Arc::clone(&self.catchups);
        let (ctl, ctl_rx) = watch::channel(FollowCtl::default());
        let r2 = run.clone();
        let handle = tokio::spawn(async move {
            let started = Instant::now();
            let outcome = match kind {
                Kind::Follow => follow_task(r2.clone(), prev, ctl_rx).await,
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
                ctl: (kind == Kind::Follow).then_some(ctl),
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
            Outcome::Moved => {
                // Not a failure, but do not plan the same read again on the
                // very next tick.
                let count = st.fails.get(&key).map_or(0, |f| f.count);
                st.fails.insert(
                    key,
                    Fail {
                        count,
                        until: Instant::now() + BACKOFF_BASE,
                    },
                );
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

/// Stop the task of container `id`. A follower is told to stop and wraps up
/// first (see "Restarts between polls"); a catch-up is aborted. Either way
/// its successor waits for it.
fn stop_task(st: &mut State, id: &str) {
    if let Some(e) = st.tasks.remove(id) {
        match &e.ctl {
            Some(ctl) => ctl.send_modify(|c| c.stop = true),
            None => e.handle.abort(),
        }
        st.dying.insert(id.to_string(), e.handle);
    }
}

/// Tell each follower whose container this poll found still running in its
/// run that what it read before the poll began (`polled_at`) is the run's.
fn confirm_followers(tasks: &HashMap<String, Entry>, obs: &[Obs], polled_at: Instant) {
    let by_id: HashMap<&str, &Obs> = obs.iter().map(|o| (o.id.as_str(), o)).collect();
    for e in tasks.values() {
        let Some(ctl) = &e.ctl else { continue };
        let same = by_id
            .get(e.run.id.as_str())
            .is_some_and(|o| o.running && o.started_at == Some(e.run.started_at));
        if same {
            ctl.send_modify(|c| c.confirmed = Some(polled_at));
        }
    }
}

// ─── App-wide instance ────────────────────────────────────────────────

static SUPERVISOR: LazyLock<Arc<Supervisor>> = LazyLock::new(|| Arc::new(Supervisor::new()));

/// Called from the poll tick with the list it just fetched; `polled_at`:
/// when that listing began.
pub fn reconcile(containers: &[Container], polled_at: Instant) {
    SUPERVISOR.reconcile(containers, polled_at);
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

    // ── Resume: matching a replay against the stored run ──

    fn lines(tag: &str, n: usize) -> Vec<String> {
        (1..=n).map(|i| format!("{tag}{i}")).collect()
    }

    /// What the history writer stores after persisting rows with these raw
    /// texts (see `history::write_log_batch`).
    fn stored_rows<T: AsRef<[u8]>>(rows: &[T], complete: bool) -> history::RunProgress {
        let mut d = history::RunDigest::default();
        let mut trailing_empty = 0;
        for r in rows {
            let r = r.as_ref();
            d.push(r);
            trailing_empty = if r.is_empty() { trailing_empty + 1 } else { 0 };
        }
        history::RunProgress {
            persisted_lines: rows.len() as i64,
            trailing_empty,
            complete,
            digest: Some(d),
            ..Default::default()
        }
    }

    fn stored(ls: &[String], complete: bool) -> history::RunProgress {
        stored_rows(ls, complete)
    }

    /// Feed `replay` until it diverges; the steps taken.
    fn feed<T: AsRef<[u8]>>(r: &mut Resume, replay: &[T]) -> Vec<Step> {
        let mut steps = Vec::new();
        for l in replay {
            let s = r.next(l.as_ref());
            steps.push(s);
            if s == Step::Diverged {
                break;
            }
        }
        steps
    }

    /// What resuming `p` with `replay` stores (`None`: it diverged).
    fn kept(p: &history::RunProgress, replay: &[&str]) -> Option<Vec<String>> {
        let mut r = Resume::new(p);
        let mut out = Vec::new();
        for l in replay {
            match r.next(l.as_bytes()) {
                Step::Skip => {}
                Step::Keep(from) => out.push(l[from..].to_string()),
                Step::Diverged => return None,
            }
        }
        Some(out)
    }

    #[test]
    fn resume_skips_exactly_the_stored_text_of_the_same_run() {
        let log = lines("l", 10);
        let mut r = Resume::new(&stored(&log[..3], false));
        let steps = feed(&mut r, &log);
        assert_eq!(&steps[..3], [Step::Skip; 3]);
        assert_eq!(&steps[3..], [Step::Keep(0); 7]);
        assert!(!r.ended_short());
        // Nothing stored: everything is kept.
        let mut r = Resume::new(&history::RunProgress::default());
        assert_eq!(feed(&mut r, &log), [Step::Keep(0); 10]);
    }

    #[test]
    fn a_line_the_follower_got_in_two_pieces_counts_as_the_line() {
        // `logs -f` printed "hello" as "hel" + "lo"; the replay does not.
        let p = stored_rows(&["one", "hel", "lo"], false);
        assert_eq!(kept(&p, &["one", "hello", "two"]), Some(vec!["two".into()]));
        // By row count (3) the resume would have skipped "two".
    }

    #[test]
    fn stored_rows_ending_inside_a_line_keep_its_rest() {
        let p = stored_rows(&["one", "hel"], false);
        assert_eq!(
            kept(&p, &["one", "hello", "two"]),
            Some(vec!["lo".into(), "two".into()])
        );
    }

    #[test]
    fn blank_lines_left_out_by_the_follower_are_not_stored_again() {
        // `logs -f` streamed "d", "", "e" live as "d", "e".
        let p = stored_rows(&["d", "e"], false);
        assert_eq!(kept(&p, &["d", "", "e", "f"]), Some(vec!["f".into()]));
        // By row count (2) the resume would have stored "e" again.
    }

    #[test]
    fn blank_lines_at_the_resume_point() {
        let replay = ["a", "", "", "b", ""];
        let s = |v: &[&str]| Some(v.iter().map(|x| x.to_string()).collect::<Vec<_>>());
        // Stored "a" and one blank line: the second blank line is new.
        assert_eq!(
            kept(&stored_rows(&["a", ""], false), &replay),
            s(&["", "b", ""])
        );
        // Stored "a" only.
        assert_eq!(
            kept(&stored_rows(&["a"], false), &replay),
            s(&["", "", "b", ""])
        );
        // Stored everything.
        assert_eq!(kept(&stored_rows(&replay, false), &replay), s(&[]));
        // Blank lines inside the stored text are never kept.
        assert_eq!(kept(&stored_rows(&["a", "b"], false), &replay), s(&[""]));
        // A run that so far printed only blank lines.
        assert_eq!(
            kept(&stored_rows(&["", ""], false), &replay[1..]),
            s(&["b", ""])
        );
    }

    #[test]
    fn resuming_with_the_stored_position_yields_each_line_once() {
        // Model the exactness argument: the replay is append-only, the
        // stored text is a prefix of it, the fingerprint follows it.
        let log = lines("l", 10);
        let mut db: Vec<String> = Vec::new();
        for upto in [4usize, 4, 7, 10] {
            let mut r = Resume::new(&stored(&db, false));
            for l in &log[..upto] {
                match r.next(l.as_bytes()) {
                    Step::Keep(0) => db.push(l.clone()),
                    Step::Skip => {}
                    other => panic!("{other:?} for {l}"),
                }
            }
        }
        assert_eq!(db, log);
    }

    #[test]
    fn a_later_run_is_caught_at_the_first_checkpoint_past_the_difference() {
        let run1 = lines("a", 100);
        // Differs from byte 1: caught at once.
        let mut r = Resume::new(&stored(&run1, false));
        assert_eq!(feed(&mut r, &lines("b", 100)), [Step::Diverged]);
        // Same first 5 lines (say, a banner), then different from byte 11:
        // caught at byte 16, in line 8; nothing was kept before that.
        let mut run2 = run1[..5].to_vec();
        run2.extend(lines("b", 95));
        let mut r = Resume::new(&stored(&run1, false));
        let steps = feed(&mut r, &run2);
        assert_eq!(steps.len(), 8);
        assert!(steps[..7].iter().all(|s| *s == Step::Skip));
        assert_eq!(steps.last(), Some(&Step::Diverged));
    }

    #[test]
    fn a_later_run_differing_only_after_the_last_checkpoint_is_caught_at_the_stored_end() {
        // 100 lines, 292 text bytes; the last checkpoint is at byte 256.
        let run1 = lines("a", 100);
        let mut run2 = run1.clone();
        run2[98] = "a9X".into(); // bytes 285..288
        run2.push("more".into());
        let mut r = Resume::new(&stored(&run1, false));
        let steps = feed(&mut r, &run2);
        assert_eq!(steps.len(), 100);
        assert_eq!(steps.last(), Some(&Step::Diverged));
    }

    #[test]
    fn a_replay_shorter_than_the_stored_run_is_a_later_run() {
        let run1 = lines("a", 10);
        let mut r = Resume::new(&stored(&run1, false));
        let steps = feed(&mut r, &run1[..4]);
        assert_eq!(steps, [Step::Skip; 4], "a prefix is not caught by content");
        assert!(r.ended_short(), "but a final log cannot be shorter");
        // Blank lines are lines: a replay of only those is short too.
        let mut r = Resume::new(&stored(&run1, false));
        feed(&mut r, &["", ""]);
        assert!(r.ended_short());
        // An empty replay proves nothing (it is what `logs` returns while
        // a start is under way) and has nothing to store.
        let r = Resume::new(&stored(&run1, false));
        assert!(!r.ended_short());
    }

    #[test]
    fn a_complete_run_has_no_further_line() {
        let run1 = lines("a", 3);
        // The same run, read again (recheck): matches to the end.
        let mut r = Resume::new(&stored(&run1, true));
        assert_eq!(feed(&mut r, &run1), [Step::Skip; 3]);
        assert!(!r.ended_short());
        // A later run that starts with the whole complete run.
        let mut run2 = run1.clone();
        run2.push("more".into());
        let mut r = Resume::new(&stored(&run1, true));
        assert_eq!(
            feed(&mut r, &run2),
            [Step::Skip, Step::Skip, Step::Skip, Step::Diverged]
        );
        // Or one whose last line goes on.
        let mut r = Resume::new(&stored(&run1, true));
        assert_eq!(
            feed(&mut r, &["a1", "a2", "a3 and more"]),
            [Step::Skip, Step::Skip, Step::Diverged]
        );
        // Its own blank lines at the end are part of it; one more is not.
        let p = stored_rows(&["a", ""], true);
        assert_eq!(kept(&p, &["a", ""]), Some(vec![]));
        assert_eq!(kept(&p, &["a", "", ""]), None);
        // Even an empty complete run.
        let mut r = Resume::new(&stored(&[], true));
        assert_eq!(feed(&mut r, &run1), [Step::Diverged]);
    }

    #[test]
    fn identical_output_cannot_be_told_apart_and_is_kept_once() {
        // The documented limit: a later run that begins with exactly the
        // stored text of an incomplete run looks like that run going on.
        let run1 = lines("a", 4);
        let mut run2 = run1.clone();
        run2.extend(lines("b", 2));
        let mut r = Resume::new(&stored(&run1, false));
        let steps = feed(&mut r, &run2);
        assert_eq!(&steps[..4], [Step::Skip; 4]);
        assert_eq!(&steps[4..], [Step::Keep(0); 2]);
    }

    #[test]
    fn a_run_saved_before_text_bytes_were_recorded_is_resumed_by_row_count() {
        let mut p = stored(&lines("a", 3), false);
        p.digest = None;
        let mut r = Resume::new(&p);
        assert_eq!(
            feed(&mut r, &lines("z", 4)),
            [Step::Skip, Step::Skip, Step::Skip, Step::Keep(0)]
        );
        // Completeness and length still count.
        p.complete = true;
        let mut r = Resume::new(&p);
        assert_eq!(feed(&mut r, &lines("z", 4)).last(), Some(&Step::Diverged));
        let mut r = Resume::new(&p);
        feed(&mut r, &lines("z", 2));
        assert!(r.ended_short());
    }

    #[test]
    fn same_second_rerun_is_stored_as_a_separate_run_from_its_first_line() {
        // Model of a task over the history DB: resume the latest run of
        // the key; on divergence open the next one and read again.
        fn capture(db: &mut Vec<(Vec<String>, bool)>, replay: &[String], ends: bool) {
            for _ in 0..MAX_PASSES {
                let (got, complete) = db.last().cloned().unwrap_or_default();
                let mut r = Resume::new(&stored(&got, complete));
                let mut kept = Vec::new();
                let mut diverged = false;
                for l in replay {
                    match r.next(l.as_bytes()) {
                        Step::Skip => {}
                        Step::Keep(from) => kept.push(l[from..].to_string()),
                        Step::Diverged => {
                            diverged = true;
                            break;
                        }
                    }
                }
                if diverged || (ends && r.ended_short()) {
                    db.push((Vec::new(), false)); // start_run(seq + 1)
                    continue;
                }
                if db.is_empty() {
                    db.push((Vec::new(), false));
                }
                let last = db.last_mut().unwrap();
                last.0.extend(kept);
                last.1 |= ends;
                return;
            }
            panic!("did not settle");
        }

        let mut db = Vec::new();
        let run1 = lines("a", 6);
        // Run 1 followed for a while, then caught up after it stopped.
        capture(&mut db, &run1[..4], false);
        capture(&mut db, &run1, true);
        assert_eq!(db, [(run1.clone(), true)]);
        // Run 2 starts in the same second, so its replay comes under the
        // same key. It is longer, shorter, or just different: always a
        // run of its own, read from its first line.
        let mut longer = run1.clone();
        longer.push("and more".into());
        for run2 in [lines("b", 9), lines("b", 2), longer] {
            let mut d = db.clone();
            capture(&mut d, &run2[..run2.len() / 2], false);
            capture(&mut d, &run2, true);
            assert_eq!(d, [(run1.clone(), true), (run2.clone(), true)], "{run2:?}");
        }
        // A short run 2 of a still-incomplete run 1 (caught when its log
        // turns out shorter than what we hold).
        let mut d = vec![(run1.clone(), false)];
        capture(&mut d, &lines("a", 3), true);
        assert_eq!(d, [(run1.clone(), false), (lines("a", 3), true)]);
    }

    // ── Simulation: `logs -f` splitting lines and dropping blank ones ──
    //
    // A log is written while followers come and go (app restarts). Each
    // follower's stream is a replay of what was written when it started,
    // then the rest of the log as `logs -f` streams it live: a line is
    // split wherever the CLI's read ended inside it (never into an empty
    // piece), and blank lines are left out. Each follower stops after a
    // random number of rows; a last catch-up reads everything. Every
    // stored row is tagged with the part of the true line it holds, so the
    // result can be checked exactly: each line with text covered once, in
    // order, with no gap or overlap; each blank line at most once.
    // (After the burst session's prototype, `logsplit.rs`.)

    struct Rng(u64);
    impl Rng {
        fn next(&mut self) -> u64 {
            // xorshift64*
            self.0 ^= self.0 >> 12;
            self.0 ^= self.0 << 25;
            self.0 ^= self.0 >> 27;
            self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
        }
        fn below(&mut self, n: u64) -> u64 {
            self.next() % n
        }
    }

    /// True log lines (text without `\n`): blank lines, short and long
    /// lines, multi-byte characters, `\r\n` endings.
    fn gen_lines(rng: &mut Rng, n: usize) -> Vec<Vec<u8>> {
        let pieces: [&[u8]; 6] = [b"a", b"b", b"7", " é".as_bytes(), "€".as_bytes(), b" "];
        (0..n)
            .map(|_| {
                let len = match rng.below(10) {
                    0 | 1 => 0,
                    2..=6 => 1 + rng.below(6),
                    _ => 10 + rng.below(40),
                };
                let mut l = Vec::new();
                for _ in 0..len {
                    l.extend_from_slice(pieces[rng.below(pieces.len() as u64) as usize]);
                }
                if !l.is_empty() && rng.below(8) == 0 {
                    l.push(b'\r');
                }
                l
            })
            .collect()
    }

    /// A streamed line: part `[from, to)` of true line `line`.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    struct Piece {
        line: usize,
        from: usize,
        to: usize,
    }

    /// The stream of a follower that starts when `start` bytes of the log
    /// are written.
    fn stream(rng: &mut Rng, lines: &[Vec<u8>], start: usize) -> Vec<Piece> {
        let mut out = Vec::new();
        let mut at = 0; // log offset of the current line
        for (i, l) in lines.iter().enumerate() {
            let end = at + l.len(); // offset of its '\n'
            if end < start {
                // Written before the follower started: replayed whole.
                out.push(Piece {
                    line: i,
                    from: 0,
                    to: l.len(),
                });
            } else if !l.is_empty() {
                // Streamed live; a blank line would be left out. The
                // replay's read ends at `start`, later reads at random.
                let mut cuts: Vec<usize> = Vec::new();
                if start > at {
                    cuts.push(start - at);
                }
                for _ in 0..rng.below(3) {
                    cuts.push(1 + rng.below(l.len() as u64) as usize);
                }
                cuts.retain(|&c| c < l.len());
                cuts.sort_unstable();
                cuts.dedup();
                let mut from = 0;
                for c in cuts.into_iter().chain([l.len()]) {
                    out.push(Piece {
                        line: i,
                        from,
                        to: c,
                    });
                    from = c;
                }
            }
            at = end + 1;
        }
        out
    }

    fn simulate(seed: u64) {
        let mut rng = Rng(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1);
        let n = 1 + rng.below(120) as usize;
        let lines = gen_lines(&mut rng, n);
        let log_len: usize = lines.iter().map(|l| l.len() + 1).sum();
        let text = |p: &Piece| lines[p.line][p.from..p.to].to_vec();
        let mut stored: Vec<Piece> = Vec::new();
        let mut start = 0usize;
        for round in 0.. {
            let last = round >= 8;
            start = if last {
                log_len + 1 // a catch-up: the whole log, as a replay
            } else {
                (start + rng.below(log_len as u64 / 3 + 2) as usize).min(log_len)
            };
            let rows: Vec<Vec<u8>> = stored.iter().map(text).collect();
            let mut resume = Resume::new(&stored_rows(&rows, false));
            let budget = if last {
                usize::MAX
            } else {
                rng.below(40) as usize
            };
            let mut taken = 0;
            for p in stream(&mut rng, &lines, start) {
                if taken == budget {
                    break;
                }
                match resume.next(&text(&p)) {
                    Step::Skip => {}
                    Step::Keep(k) => {
                        stored.push(Piece {
                            from: p.from + k,
                            ..p
                        });
                        taken += 1;
                    }
                    Step::Diverged => panic!("seed {seed}: the same run taken for another"),
                }
            }
            if last {
                assert!(!resume.ended_short(), "seed {seed}");
                break;
            }
        }
        // Each line with text covered once, in order; blank lines at most
        // once.
        let mut it = stored.iter().peekable();
        for (i, l) in lines.iter().enumerate() {
            let mut covered = 0;
            let mut rows = 0;
            while let Some(p) = it.next_if(|p| p.line == i) {
                assert_eq!(p.from, covered, "seed {seed}: gap or overlap in line {i}");
                covered = p.to;
                rows += 1;
            }
            if l.is_empty() {
                assert!(rows <= 1, "seed {seed}: blank line {i} stored {rows} times");
            } else {
                assert!(rows > 0, "seed {seed}: line {i} missing");
                assert_eq!(covered, l.len(), "seed {seed}: line {i} incomplete");
            }
        }
        assert!(it.next().is_none(), "seed {seed}: rows out of order");
    }

    #[test]
    fn simulated_splits_left_out_blank_lines_and_restarts_store_every_line_once() {
        for seed in 0..3000 {
            simulate(seed);
        }
    }

    #[test]
    fn simulated_resume_by_row_count_loses_or_repeats_lines() {
        // The same simulation catches the old resume (by row count).
        assert!(
            (0..200).any(|seed| !simulate_by_rows(seed)),
            "resuming by row count passed the simulation"
        );
    }

    /// `simulate`, but resuming as before this fix: skip as many replay
    /// lines as there are stored rows. Whether the stored text came out
    /// right.
    fn simulate_by_rows(seed: u64) -> bool {
        let mut rng = Rng(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1);
        let lines = gen_lines(&mut rng, 80);
        let log_len: usize = lines.iter().map(|l| l.len() + 1).sum();
        let mut stored: Vec<Piece> = Vec::new();
        let mut start = 0usize;
        for round in 0..6 {
            start = if round == 5 {
                log_len + 1
            } else {
                (start + rng.below(log_len as u64 / 3 + 2) as usize).min(log_len)
            };
            let skip = stored.len();
            let budget = if round == 5 {
                usize::MAX
            } else {
                rng.below(40) as usize
            };
            for p in stream(&mut rng, &lines, start)
                .into_iter()
                .skip(skip)
                .take(budget)
            {
                stored.push(p);
            }
        }
        let joined: Vec<u8> = stored
            .iter()
            .flat_map(|p| lines[p.line][p.from..p.to].to_vec())
            .collect();
        joined == lines.concat()
    }

    #[test]
    fn keys_settle_after_their_second_plus_margin() {
        assert!(!key_settled(100, 100));
        assert!(!key_settled(100, 101));
        assert!(key_settled(100, 102));
        let now = crate::timeutil::now_unix();
        assert_eq!(settle_delay(now - 10), Duration::ZERO);
        assert!(settle_delay(now) <= Duration::from_secs(2));
        assert!(settle_delay(now) > Duration::from_secs(1));
        assert_eq!(settle_delay(now + 3600), Duration::from_secs(3), "capped");
    }

    #[test]
    fn backoff_doubles_and_caps() {
        assert_eq!(backoff(1), Duration::from_secs(4));
        assert_eq!(backoff(2), Duration::from_secs(8));
        assert_eq!(backoff(3), Duration::from_secs(16));
        assert_eq!(backoff(40), BACKOFF_MAX);
    }

    #[test]
    fn text_of_drops_only_the_final_newline() {
        assert_eq!(text_of(b"hi\n"), b"hi");
        assert_eq!(text_of(b"hi\r\n"), b"hi\r");
        assert_eq!(text_of(b"partial"), b"partial");
        assert_eq!(text_of(b"\n"), b"");
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
                ctl: None,
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

    // ── Restarts between polls ──

    fn split(rows: &[&str], now: &str) -> usize {
        let rows: Vec<&[u8]> = rows.iter().map(|r| r.as_bytes()).collect();
        old_run_rows(&rows, now.as_bytes())
    }

    #[test]
    fn the_new_runs_first_lines_are_found_at_the_end_of_the_held_rows() {
        // The old run's last lines (a shutdown message), then what
        // `logs -f` streamed on into: the new run's first lines.
        assert_eq!(split(&["n57", "n58", "bye", "n1", "n2"], "n1n2n3"), 3);
        // Nothing of the new run was read yet.
        assert_eq!(split(&["n57", "bye"], "n1n2"), 2);
        // Only the new run's lines.
        assert_eq!(split(&["n1", "n2"], "n1n2n3"), 0);
        // The new run's log is empty so far.
        assert_eq!(split(&["a", "b"], ""), 2);
    }

    #[test]
    fn split_lines_on_either_side_are_matched_by_text() {
        // Old "abc" streamed as "ab" + "c", new "xyz" as "xy" + "z".
        assert_eq!(split(&["ab", "c", "xy", "z"], "xyzw"), 2);
    }

    #[test]
    fn the_new_run_starts_at_a_held_row_never_inside_one() {
        // "xy" ends the held text and begins the new log, but it would
        // start inside "qx": not the new run's.
        assert_eq!(split(&["qx", "y"], "xyz"), 2);
        // The longest match starts inside "xa"; a shorter one ("ab") starts
        // at a row, so that is where the new run begins.
        assert_eq!(split(&["xa", "b", "ab"], "ababab"), 2);
    }

    #[test]
    fn blank_rows_at_the_cut_stay_with_the_old_run() {
        assert_eq!(split(&["a", "", "n1"], "n1n2"), 2);
    }

    #[test]
    fn identical_text_on_both_sides_is_taken_as_the_new_runs() {
        // The documented limit: 3 old "tick" lines, then 1 new; the new
        // run's log has 2 by the time it is read. The longest match is
        // taken, so one old "tick" counts as the new run's: it is stored
        // once (under the new run) rather than twice.
        assert_eq!(split(&["tick", "tick", "tick", "tick"], "ticktick"), 2);
    }

    #[test]
    fn held_rows_are_stored_once_a_check_begun_after_them_confirms_them() {
        let mut un = Unconfirmed::new(&Run::new("a", 1), 0);
        un.push(b"x".to_vec());
        std::thread::sleep(Duration::from_millis(2));
        let check_began = Instant::now();
        std::thread::sleep(Duration::from_millis(2));
        un.push(b"yz".to_vec());
        un.confirm(check_began);
        let left: Vec<&[u8]> = un.rows.iter().map(|(_, t)| t.as_slice()).collect();
        assert_eq!(left, [b"yz".as_slice()], "read after the check began");
        assert_eq!(un.size, 3);
        un.keep_first(0);
        assert!(un.rows.is_empty());
        assert_eq!(un.size, 0);
    }

    fn sleeper() -> JoinHandle<()> {
        tokio::spawn(async { tokio::time::sleep(Duration::from_secs(3600)).await })
    }

    fn entry(run: Run, kind: Kind, ctl: Option<watch::Sender<FollowCtl>>) -> Entry {
        Entry {
            gen: 1,
            run,
            kind,
            handle: sleeper(),
            ctl,
        }
    }

    #[tokio::test]
    async fn a_stopped_follower_is_told_to_wrap_up_not_killed() {
        let sup = Supervisor::new();
        let (tx, rx) = watch::channel(FollowCtl::default());
        let (f, c);
        {
            let mut st = sup.lock();
            st.tasks
                .insert("a".into(), entry(Run::new("a", 1), Kind::Follow, Some(tx)));
            st.tasks
                .insert("b".into(), entry(Run::new("b", 1), Kind::CatchUp, None));
            f = st.tasks["a"].handle.abort_handle();
            c = st.tasks["b"].handle.abort_handle();
            stop_task(&mut st, "a");
            stop_task(&mut st, "b");
            assert!(st.tasks.is_empty());
            assert_eq!(st.dying.len(), 2, "successors wait for both");
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
        assert!(rx.borrow().stop, "the follower was told to stop");
        assert!(!f.is_finished(), "and left to wrap up");
        assert!(c.is_finished(), "a catch-up is aborted");
        // Quitting does not wait for a wrap-up.
        sup.stop_all();
        tokio::time::sleep(Duration::from_millis(50)).await;
        assert!(f.is_finished());
    }

    #[tokio::test]
    async fn a_poll_confirms_only_followers_still_in_their_run() {
        let mut tasks = HashMap::new();
        let mut rxs = Vec::new();
        for id in ["same", "restarted", "stopped", "gone"] {
            let (tx, rx) = watch::channel(FollowCtl::default());
            tasks.insert(
                id.to_string(),
                entry(Run::new(id, 10), Kind::Follow, Some(tx)),
            );
            rxs.push(rx);
        }
        let obs = [
            o("same", true, Some(10)),
            o("restarted", true, Some(12)),
            o("stopped", false, Some(10)),
        ];
        let at = Instant::now();
        confirm_followers(&tasks, &obs, at);
        let got: Vec<Option<Instant>> = rxs.iter().map(|r| r.borrow().confirmed).collect();
        assert_eq!(got, [Some(at), None, None, None]);
        for e in tasks.values() {
            e.handle.abort();
        }
    }

    #[test]
    fn a_task_whose_container_moved_on_cools_down_without_failing() {
        let sup = Supervisor::new();
        let run = Run::new("a", 1);
        let key = (run.clone(), Kind::CatchUp);
        sup.finish(1, &run, Kind::CatchUp, Outcome::Moved, Duration::ZERO);
        let f = sup.lock().fails[&key];
        assert_eq!(f.count, 0, "not counted towards giving up");
        assert!(
            f.until > Instant::now(),
            "but not re-planned on the next tick"
        );
        // Not caught up: a later run with the same start second may need it.
        assert!(!sup.lock().caught_up.contains(&run));
        // A real failure afterwards still counts from 1.
        sup.finish(
            2,
            &run,
            Kind::CatchUp,
            Outcome::Failed("x".into()),
            Duration::ZERO,
        );
        assert_eq!(sup.lock().fails[&key].count, 1);
    }
}
