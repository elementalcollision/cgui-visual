//! Opt-in live test suite: drives the app's own `runtime` functions against
//! a real Apple `container` CLI.
//!
//! Every test is `#[ignore]` AND returns early unless `CGUI_LIVE=1`, so
//! plain `cargo test` (and CI) never touch the runtime. Run with:
//!
//!   CGUI_LIVE=1 cargo test --test live_cli -- --ignored --nocapture
//!
//! Requirements: macOS with `container` on PATH, system services running
//! (`container system start`), and `alpine:latest` pullable. Total runtime
//! is roughly a minute.
//!
//! Safety: tests only create resources named `cgui-live-<pid>-<purpose>`
//! and a Drop guard force-deletes everything a test created, even on
//! panic. Nothing else (other containers, images, volumes, networks, the
//! system services) is touched. To run a compiled binary on another Mac:
//! `cargo test --test live_cli --no-run`, copy the executable, then run it
//! there with `CGUI_LIVE=1 ./live_cli-<hash> --ignored`.

use app_lib::runtime::{self, RunArgs};
use std::process::{Command, Output};
use std::sync::Once;
use std::time::Duration;

macro_rules! live_only {
    () => {
        if std::env::var("CGUI_LIVE").as_deref() != Ok("1") {
            eprintln!("CGUI_LIVE != 1; skipping live test");
            return;
        }
        ensure_alpine();
    };
}

fn ensure_alpine() {
    static PULL: Once = Once::new();
    PULL.call_once(|| {
        let _ = Command::new(runtime::current_bin())
            .args(["image", "pull", "alpine:latest"])
            .output();
    });
}

fn cli(args: &[&str]) -> Output {
    Command::new(runtime::current_bin())
        .args(args)
        .output()
        .unwrap_or_else(|e| panic!("spawn `container {}`: {e}", args.join(" ")))
}

fn cli_ok(args: &[&str]) -> String {
    let o = cli(args);
    assert!(
        o.status.success(),
        "`container {}` failed: {}",
        args.join(" "),
        String::from_utf8_lossy(&o.stderr)
    );
    String::from_utf8_lossy(&o.stdout).trim().to_string()
}

/// Unique resource name: `cgui-live-<pid>-<purpose>`.
fn name(purpose: &str) -> String {
    format!("cgui-live-{}-{purpose}", std::process::id())
}

/// Host port unlikely to collide, distinct per (pid, slot).
fn port(slot: u16) -> u16 {
    30_000 + (std::process::id() % 20_000) as u16 + slot
}

/// Force-deletes everything registered with it when dropped (also on panic).
#[derive(Default)]
struct Guard {
    containers: Vec<String>,
    volumes: Vec<String>,
    networks: Vec<String>,
    images: Vec<String>,
    machines: Vec<String>,
    files: Vec<std::path::PathBuf>,
}

impl Guard {
    /// Register a resource the test creates itself (via the app's own
    /// functions) so it is still cleaned up if the test panics.
    fn track_volume(&mut self, n: &str) -> String {
        self.volumes.push(n.to_string());
        n.to_string()
    }
    fn track_network(&mut self, n: &str) -> String {
        self.networks.push(n.to_string());
        n.to_string()
    }
    fn track_image(&mut self, r: &str) -> String {
        self.images.push(r.to_string());
        r.to_string()
    }
    #[allow(dead_code)]
    fn track_machine(&mut self, n: &str) -> String {
        self.machines.push(n.to_string());
        n.to_string()
    }
    fn container(&mut self, n: &str) -> String {
        self.containers.push(n.to_string());
        n.to_string()
    }
    fn volume(&mut self, n: &str) -> String {
        self.volumes.push(n.to_string());
        cli_ok(&["volume", "create", n]);
        n.to_string()
    }
    fn network(&mut self, n: &str) -> String {
        self.networks.push(n.to_string());
        cli_ok(&["network", "create", n]);
        n.to_string()
    }
    fn file(&mut self, p: std::path::PathBuf) -> std::path::PathBuf {
        self.files.push(p.clone());
        p
    }
}

impl Drop for Guard {
    fn drop(&mut self) {
        for c in &self.containers {
            let _ = cli(&["delete", "--force", c]);
        }
        for m in &self.machines {
            let _ = cli(&["machine", "delete", m]);
        }
        for i in &self.images {
            let _ = cli(&["image", "delete", i]);
        }
        for v in &self.volumes {
            let _ = cli(&["volume", "delete", v]);
        }
        for n in &self.networks {
            let _ = cli(&["network", "delete", n]);
        }
        for f in &self.files {
            let _ = std::fs::remove_file(f);
        }
    }
}

fn run_args(cname: &str) -> RunArgs {
    RunArgs {
        image: "alpine:latest".into(),
        name: Some(cname.into()),
        command: Some("sleep 300".into()),
        ..Default::default()
    }
}

async fn find(id: &str) -> app_lib::model::Container {
    let (cs, _) = runtime::list_containers_raw().await.expect("list");
    cs.into_iter()
        .find(|c| c.id == id)
        .unwrap_or_else(|| panic!("{id} not listed"))
}

#[tokio::test]
#[ignore]
async fn parse_containers_stats_and_resources() {
    live_only!();
    let mut g = Guard::default();
    let cname = g.container(&name("parse"));
    let vol = g.volume(&name("parse-vol"));
    let net = g.network(&name("parse-net"));
    let host_port = port(1);

    let args = RunArgs {
        ports: vec![format!("{host_port}:80")],
        env: vec!["CGUI_LIVE_ENV=yes".into()],
        volumes: vec![format!("{vol}:/data")],
        network: Some(net.clone()),
        ..run_args(&cname)
    };
    runtime::run_image(args).await.expect("run_image");

    // Container list: non-default fields.
    let (cs, stats) = runtime::list_containers_raw().await.expect("list");
    let c = cs
        .iter()
        .find(|c| c.id == cname)
        .unwrap_or_else(|| panic!("{cname} not listed"));
    assert_eq!(c.id, cname, "id is the full CLI id");
    assert_eq!(c.short_id, cname.chars().take(8).collect::<String>());
    assert!(cname.len() > 8, "name must exceed the 8-char display id");
    assert_eq!(c.status, "running");
    assert_eq!(c.image, "docker.io/library/alpine:latest");
    assert_eq!(c.name, cname);
    assert_eq!(c.ports, vec![format!("{host_port}:80")]);
    assert_eq!(c.cmd, vec!["sleep", "300"]);
    assert!(c.created.starts_with("20"), "created: {}", c.created);
    assert_ne!(c.uptime, "—");
    assert!(c.started_unix.is_some_and(|t| t > 1_700_000_000));
    assert!(c.mem.limit > 0.0, "memory limit parsed");

    // Stats for the running container (retry: first sample can lag).
    let mut row = runtime::match_stat(&stats, &cname).cloned();
    for _ in 0..10 {
        if row.is_some() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
        let s = runtime::stats_snapshot().await.expect("stats");
        row = runtime::match_stat(&s, &cname).cloned();
    }
    let row = row.expect("stats row for running container");
    assert!(row.memory_usage_bytes > 0, "memory usage parsed");
    assert!(row.memory_limit_bytes > 0, "memory limit parsed");
    assert!(row.cpu_usage_usec > 0, "cpu usage parsed");

    // Images / volumes / networks.
    let imgs = runtime::list_images().await.expect("images");
    let alpine = imgs
        .iter()
        .find(|i| i.reference == "docker.io/library/alpine:latest")
        .expect("alpine image listed");
    assert!(alpine.size > 0.0);
    assert!(alpine.id.starts_with("sha256:"));
    assert!(alpine.created.starts_with("20"));

    let vols = runtime::list_volumes().await.expect("volumes");
    let v = vols.iter().find(|v| v.name == vol).expect("volume listed");
    assert_eq!(v.driver, "local");
    assert!(v.size > 0.0);
    assert!(v.mountpoint.ends_with("volume.img"));
    assert_eq!(v.refs, 1, "one container mounts the volume");

    let nets = runtime::list_networks().await.expect("networks");
    let n = nets.iter().find(|n| n.name == net).expect("network listed");
    assert_eq!(n.mode, "nat");
    assert_eq!(n.state, "active");
    assert!(n.subnet.contains('/'), "subnet: {}", n.subnet);
    assert_ne!(n.gateway, "—");
    assert_eq!(n.containers, 1, "one container attached");
    let default = nets.iter().find(|n| n.name == "default").expect("default");
    assert_ne!(default.subnet, "—");

    // system df / system status / machines / builder / registries.
    let df = runtime::system_df().await.expect("system_df");
    assert!(df.images.total > 0 && df.images.size_in_bytes > 0);
    assert!(df.containers.active >= 1);
    assert!(df.volumes.total >= 1);
    assert!(runtime::system_running().await.expect("system status"));
    runtime::machine_list().await.expect("machine list parses");
    runtime::builder_status()
        .await
        .expect("builder status parses");
    runtime::registry_list()
        .await
        .expect("registry list parses");
}

#[tokio::test]
#[ignore]
async fn lifecycle_run_stop_start_kill_clean_delete() {
    live_only!();
    let mut g = Guard::default();
    let cname = g.container(&name("life"));
    let vol = g.volume(&name("life-vol"));
    let net = g.network(&name("life-net"));

    let args = RunArgs {
        ports: vec![format!("{}:80", port(2))],
        env: vec!["A=1".into(), "B=two".into()],
        volumes: vec![format!("{vol}:/data")],
        network: Some(net),
        ..run_args(&cname)
    };
    let out = runtime::run_image(args).await.expect("run_image");
    assert!(out.contains(&cname), "run prints the id: {out}");
    // The CLI matches ids exactly: the full id must work for every action.
    let id = find(&cname).await.id;
    assert_eq!(id, cname);

    // Env reached the container.
    let env = cli_ok(&["exec", &id, "sh", "-c", "echo $A-$B"]);
    assert_eq!(env, "1-two");

    runtime::stop(&id).await.expect("stop");
    assert_eq!(find(&id).await.status, "exited");

    runtime::start(&id).await.expect("start");
    assert_eq!(find(&id).await.status, "running");

    // SIGUSR1 is ignored by PID 1, so the container keeps running.
    runtime::kill(&id, Some("SIGUSR1")).await.expect("kill -s");
    assert_eq!(find(&id).await.status, "running");

    runtime::clean_containers(std::slice::from_ref(&id))
        .await
        .expect("clean");

    runtime::restart(&id).await.expect("restart");
    assert_eq!(find(&id).await.status, "running");

    runtime::kill(&id, None).await.expect("kill (SIGKILL)");
    for _ in 0..20 {
        if find(&id).await.status != "running" {
            break;
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
    assert_ne!(find(&id).await.status, "running");

    runtime::delete(&id).await.expect("delete");
    let (cs, _) = runtime::list_containers_raw().await.expect("list");
    assert!(cs.iter().all(|c| c.id != id), "container gone");
}

#[tokio::test]
#[ignore]
async fn delete_volume_and_network() {
    live_only!();
    let mut g = Guard::default();
    let vol = g.volume(&name("del-vol"));
    let net = g.network(&name("del-net"));

    assert!(runtime::list_volumes()
        .await
        .unwrap()
        .iter()
        .any(|v| v.name == vol));
    assert!(runtime::list_networks()
        .await
        .unwrap()
        .iter()
        .any(|n| n.name == net));

    runtime::delete_volume(&vol).await.expect("delete volume");
    runtime::delete_network(&net).await.expect("delete network");

    assert!(runtime::list_volumes()
        .await
        .unwrap()
        .iter()
        .all(|v| v.name != vol));
    assert!(runtime::list_networks()
        .await
        .unwrap()
        .iter()
        .all(|n| n.name != net));
}

#[tokio::test]
#[ignore]
async fn export_running_container_and_copy_in() {
    live_only!();
    let mut g = Guard::default();
    let cname = g.container(&name("fs"));
    runtime::run_image(run_args(&cname)).await.expect("run");

    let tmp = std::env::temp_dir();
    let tar = g.file(tmp.join(format!("{cname}.tar")));
    let src = g.file(tmp.join(format!("{cname}.txt")));
    std::fs::write(&src, "hello from cgui live test\n").unwrap();

    runtime::copy_path(src.to_str().unwrap(), &format!("{cname}:/tmp/hello.txt"))
        .await
        .expect("copy into container");
    let got = cli_ok(&["exec", &cname, "cat", "/tmp/hello.txt"]);
    assert_eq!(got, "hello from cgui live test");

    runtime::export_container(&cname, tar.to_str().unwrap())
        .await
        .expect("export running container");
    let len = std::fs::metadata(&tar).expect("tar written").len();
    assert!(len > 1024 * 1024, "export looks like a rootfs: {len} bytes");
    // A tar archive: the first member name is readable, `tar -t` succeeds.
    let listing = Command::new("tar").arg("-tf").arg(&tar).output().unwrap();
    assert!(listing.status.success(), "valid tar archive");
    assert!(
        String::from_utf8_lossy(&listing.stdout).contains("tmp/hello.txt"),
        "export contains the copied file"
    );
}

fn pgrep_logs(cname: &str) -> bool {
    Command::new("pgrep")
        .args(["-f", &format!("logs -f {cname}")])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

#[tokio::test]
#[ignore]
async fn aborting_logs_stream_kills_its_process() {
    live_only!();
    let mut g = Guard::default();
    let cname = g.container(&name("logs"));
    // `logs -f` returns immediately for a container that prints nothing, so
    // run something that keeps emitting lines.
    let args = RunArgs {
        command: Some("ping 127.0.0.1".into()),
        ..run_args(&cname)
    };
    runtime::run_image(args).await.expect("run");

    // Same recipe as commands.rs: spawn, drain on a task, abort the task.
    let child = runtime::spawn(&["logs", "-f", &cname]).expect("spawn logs -f");
    let task = tokio::spawn(async move {
        let _ = runtime::drain_lines(child, |_| {}).await;
    });

    let mut seen = false;
    for _ in 0..20 {
        if pgrep_logs(&cname) {
            seen = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
    assert!(seen, "`logs -f {cname}` process should be running");

    task.abort();
    let _ = task.await;

    let mut gone = false;
    for _ in 0..40 {
        if !pgrep_logs(&cname) {
            gone = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
    assert!(gone, "`logs -f {cname}` survived task abort");
}

/// F18: a quoted `sh -c "..."` command must reach the container as one
/// script argument (whitespace splitting used to break it and the
/// container exited immediately); flag-like ids never reach the CLI.
#[tokio::test]
#[ignore]
async fn quoted_command_and_flag_like_id() {
    live_only!();
    let mut g = Guard::default();
    let cname = g.container(&name("quoted"));
    let args = RunArgs {
        command: Some(r#"sh -c "echo hi && sleep 300""#.into()),
        ..run_args(&cname)
    };
    runtime::run_image(args).await.expect("run_image");
    tokio::time::sleep(Duration::from_secs(2)).await;
    assert_eq!(find(&cname).await.status, "running");

    let bad = RunArgs {
        command: Some("sh -c \"unterminated".into()),
        ..run_args(&name("quoted-bad"))
    };
    let e = runtime::run_image(bad).await.unwrap_err();
    assert!(format!("{e:#}").contains("unterminated"), "{e:#}");

    assert!(runtime::stop("--all").await.is_err());
    assert!(runtime::delete("-f").await.is_err());
}

// ─── Background log capture (logpersist supervisor) ───────────────────
//
// These tests drive `logpersist::Supervisor` the way the app's poll tick
// does (`reconcile` with a fresh container list) and read the result back
// through the history DB. History lives in a temp XDG_CONFIG_HOME shared by
// the tests of this process (history::init opens once per process); every
// container has its own unique name, so the tests stay independent.

use app_lib::logpersist::Supervisor;
use std::sync::Arc;

fn init_history() {
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        let tmp = std::env::temp_dir();
        // Leftovers of earlier (killed) runs.
        if let Ok(rd) = std::fs::read_dir(&tmp) {
            for e in rd.flatten() {
                if e.file_name()
                    .to_string_lossy()
                    .starts_with("cgui-live-hist-")
                {
                    let _ = std::fs::remove_dir_all(e.path());
                }
            }
        }
        let dir = tmp.join(format!("cgui-live-hist-{}", std::process::id()));
        // history::init reads XDG_CONFIG_HOME; set once, before any log test
        // spawns work that reads it.
        std::env::set_var("XDG_CONFIG_HOME", &dir);
        app_lib::history::init().expect("history init");
    });
}

/// One poll tick: list containers and reconcile, like lib.rs does. Only
/// this test's containers (`cgui-live-<pid>-<tag>*`) are passed on: the
/// tests run in parallel in one process, and two supervisors must not
/// follow the same container (the app has exactly one).
async fn tick(sup: &Arc<Supervisor>, tag: &str) {
    let prefix = name(tag);
    let (cs, _) = runtime::list_containers_raw().await.expect("list");
    let mine: Vec<_> = cs
        .into_iter()
        .filter(|c| c.id.starts_with(&prefix))
        .collect();
    sup.reconcile(&mine);
}

/// Persisted lines of `cid` (oldest first).
fn rows(cid: &str) -> Vec<String> {
    app_lib::history::flush_logs();
    app_lib::history::load_logs(cid, 5000, None)
        .into_iter()
        .map(|l| l.line)
        .collect()
}

/// `n1`, `n2`, ... -> 1, 2, ...
fn counters(lines: &[String]) -> Vec<i64> {
    lines
        .iter()
        .filter_map(|l| l.strip_prefix('n').and_then(|d| d.parse().ok()))
        .collect()
}

/// Split a counter sequence into runs (each restarts at 1) and assert that
/// every run is exactly 1..=k: no gap and no duplicate.
fn assert_contiguous_runs(seq: &[i64], expected_runs: usize) -> Vec<usize> {
    let mut runs: Vec<Vec<i64>> = Vec::new();
    for &n in seq {
        if n == 1 {
            runs.push(Vec::new());
        }
        runs.last_mut().expect("sequence starts at 1").push(n);
    }
    assert_eq!(runs.len(), expected_runs, "runs in {seq:?}");
    for r in &runs {
        let want: Vec<i64> = (1..=r.len() as i64).collect();
        assert_eq!(r, &want, "gap or duplicate in run: {r:?}");
    }
    runs.iter().map(Vec::len).collect()
}

const COUNTER_CMD: &str = "sh -c \"i=0; while true; do i=$((i+1)); echo n$i; sleep 0.2; done\"";

fn counter_args(cname: &str) -> RunArgs {
    RunArgs {
        command: Some(COUNTER_CMD.into()),
        ..run_args(cname)
    }
}

/// Tick every 500 ms until `f` holds or `secs` pass.
async fn tick_until(sup: &Arc<Supervisor>, tag: &str, secs: u64, f: impl Fn() -> bool) -> bool {
    for _ in 0..secs * 2 {
        tick(sup, tag).await;
        if f() {
            return true;
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
    f()
}

async fn wait_logs_follower(cname: &str, present: bool, secs: u64) -> bool {
    for _ in 0..secs * 4 {
        if pgrep_logs(cname) == present {
            return true;
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
    false
}

/// (a) A container's output is captured with no Logs view open at all, and
/// the follower is one `logs -f` process.
#[tokio::test]
#[ignore]
async fn background_capture_without_any_view() {
    live_only!();
    init_history();
    let mut g = Guard::default();
    let cname = g.container(&name("bg"));
    runtime::run_image(counter_args(&cname)).await.expect("run");
    let sup = Arc::new(Supervisor::new());

    assert!(
        tick_until(&sup, "bg", 20, || rows(&cname).len() >= 8).await,
        "captured only {:?}",
        rows(&cname)
    );
    assert!(pgrep_logs(&cname), "a follower process should be running");
    assert_eq!(sup.status().followers, 1);
    sup.stop_all();
    assert!(wait_logs_follower(&cname, false, 10).await);
    let seq = counters(&rows(&cname));
    assert_contiguous_runs(&seq, 1);
}

/// (b) "App restart": followers are killed, the container keeps printing,
/// a fresh supervisor resumes from the stored count. Contiguous counter,
/// no duplicates, no gap. Done twice, with a pause that spans many lines.
#[tokio::test]
#[ignore]
async fn app_restart_resumes_without_gap_or_duplicate() {
    live_only!();
    init_history();
    let mut g = Guard::default();
    let cname = g.container(&name("resume"));
    runtime::run_image(counter_args(&cname)).await.expect("run");

    let mut last = 0usize;
    for round in 0..3 {
        // A brand-new supervisor each time = a new app process.
        let sup = Arc::new(Supervisor::new());
        let want = last + 6;
        assert!(
            tick_until(&sup, "resume", 25, || rows(&cname).len() >= want).await,
            "round {round}: only {:?}",
            rows(&cname)
        );
        sup.stop_all();
        assert!(wait_logs_follower(&cname, false, 10).await, "killed");
        last = rows(&cname).len();
        // The app is "closed": the container keeps writing unobserved.
        tokio::time::sleep(Duration::from_secs(3)).await;
        let seq = counters(&rows(&cname));
        assert_contiguous_runs(&seq, 1);
    }
    // Final restart: everything that was written during the pauses is there.
    let sup = Arc::new(Supervisor::new());
    assert!(tick_until(&sup, "resume", 25, || rows(&cname).len() >= last + 12).await);
    sup.stop_all();
    let seq = counters(&rows(&cname));
    let lens = assert_contiguous_runs(&seq, 1);
    assert!(lens[0] >= last + 12, "pauses were back-filled: {lens:?}");
    let _ = wait_logs_follower(&cname, false, 10).await;
}

/// (c) Restarting a container starts a new run: the new run's lines are
/// captured, the old run's stay.
#[tokio::test]
#[ignore]
async fn container_restart_captures_new_run_and_keeps_old() {
    live_only!();
    init_history();
    let mut g = Guard::default();
    let cname = g.container(&name("rerun"));
    runtime::run_image(counter_args(&cname)).await.expect("run");
    let sup = Arc::new(Supervisor::new());

    assert!(tick_until(&sup, "rerun", 20, || rows(&cname).len() >= 6).await);
    let run1 = find(&cname).await.started_unix.expect("run 1 start");

    runtime::stop(&cname).await.expect("stop");
    // Stopping (not restarting yet): follower is killed, the run's tail is
    // caught up and flagged complete.
    assert!(
        tick_until(&sup, "rerun", 20, || {
            app_lib::history::run_progress(&cname, run1).is_some_and(|p| p.complete)
        })
        .await,
        "run 1 completed"
    );
    assert!(wait_logs_follower(&cname, false, 10).await);
    let run1_lines = rows(&cname).len();

    tokio::time::sleep(Duration::from_millis(1200)).await; // distinct start second
    runtime::start(&cname).await.expect("start");
    let run2 = find(&cname).await.started_unix.expect("run 2 start");
    assert!(run2 > run1, "a restart is a new run");
    assert!(
        tick_until(&sup, "rerun", 25, || rows(&cname).len() >= run1_lines + 6).await,
        "new run captured: {:?}",
        rows(&cname)
    );
    sup.stop_all();

    let p1 = app_lib::history::run_progress(&cname, run1).unwrap();
    let p2 = app_lib::history::run_progress(&cname, run2).unwrap();
    assert_eq!(p1.persisted_lines as usize, run1_lines, "old run untouched");
    assert!(p2.persisted_lines >= 6);
    let lens = assert_contiguous_runs(&counters(&rows(&cname)), 2);
    assert_eq!(lens[0], run1_lines);
    let _ = wait_logs_follower(&cname, false, 10).await;
}

/// (d) A container that runs and exits while no supervisor is running is
/// caught up on the next reconcile — exactly once. Also covers a container
/// that exits while being followed (its unread tail is completed).
#[tokio::test]
#[ignore]
async fn exited_container_is_caught_up_once() {
    live_only!();
    init_history();
    let mut g = Guard::default();

    // d1: ran and exited with no supervisor.
    let c1 = g.container(&name("gone"));
    let args = RunArgs {
        command: Some("sh -c \"for i in 1 2 3 4 5 6 7 8 9 10; do echo n$i; done\"".into()),
        ..run_args(&c1)
    };
    runtime::run_image(args).await.expect("run");
    tokio::time::sleep(Duration::from_secs(3)).await;
    assert_eq!(find(&c1).await.status, "exited");
    assert!(rows(&c1).is_empty(), "nothing captured yet");

    // d2: starts printing, exits while the supervisor is following it.
    let c2 = g.container(&name("gone-follow"));
    let args = RunArgs {
        command: Some(
            "sh -c \"i=0; while [ $i -lt 30 ]; do i=$((i+1)); echo n$i; sleep 0.2; done\"".into(),
        ),
        ..run_args(&c2)
    };
    runtime::run_image(args).await.expect("run");

    let sup = Arc::new(Supervisor::new());
    assert!(tick_until(&sup, "gone", 20, || rows(&c1).len() == 10).await);
    assert_eq!(counters(&rows(&c1)), (1..=10).collect::<Vec<_>>());
    assert!(
        tick_until(&sup, "gone", 40, || rows(&c2).len() == 30).await,
        "followed container fully captured: {:?}",
        rows(&c2)
    );
    assert_eq!(counters(&rows(&c2)), (1..=30).collect::<Vec<_>>());

    // More ticks, and a whole new supervisor: no re-read, no duplicates.
    for s in [sup.clone(), Arc::new(Supervisor::new())] {
        for _ in 0..4 {
            tick(&s, "gone").await;
            tokio::time::sleep(Duration::from_millis(500)).await;
        }
        s.stop_all();
    }
    assert_eq!(rows(&c1).len(), 10);
    assert_eq!(rows(&c2).len(), 30);
    let started = find(&c1).await.started_unix.unwrap();
    assert!(
        app_lib::history::run_progress(&c1, started)
            .unwrap()
            .complete
    );
}

/// (e) Stopping or deleting a container kills its `logs -f` process, and
/// disabling the supervisor kills all of them.
#[tokio::test]
#[ignore]
async fn stopping_container_or_supervisor_kills_follower_process() {
    live_only!();
    init_history();
    let mut g = Guard::default();
    let (a, b, c) = (
        g.container(&name("kill-a")),
        g.container(&name("kill-b")),
        g.container(&name("kill-c")),
    );
    for n in [&a, &b, &c] {
        runtime::run_image(counter_args(n)).await.expect("run");
    }
    let sup = Arc::new(Supervisor::new());
    assert!(
        tick_until(&sup, "kill", 20, || pgrep_logs(&a)
            && pgrep_logs(&b)
            && pgrep_logs(&c))
        .await
    );
    assert_eq!(sup.status().followers, 3);

    // Container stopped -> its follower dies, the others stay.
    runtime::stop(&a).await.expect("stop");
    assert!(
        tick_until(&sup, "kill", 20, || !pgrep_logs(&a)).await,
        "follower of stopped container survived"
    );
    assert!(pgrep_logs(&b) && pgrep_logs(&c));

    // Container deleted -> follower dies.
    cli_ok(&["delete", "--force", &b]);
    assert!(
        tick_until(&sup, "kill", 20, || !pgrep_logs(&b)).await,
        "follower of deleted container survived"
    );

    // Setting switched off -> everything dies; on again -> resumes.
    sup.set_enabled(false);
    assert!(wait_logs_follower(&c, false, 10).await);
    assert_eq!(sup.status().followers, 0);
    tick(&sup, "kill").await;
    tokio::time::sleep(Duration::from_secs(1)).await;
    assert!(!pgrep_logs(&c), "disabled supervisor must not follow");
    sup.set_enabled(true);
    assert!(tick_until(&sup, "kill", 20, || pgrep_logs(&c)).await);
    sup.stop_all();
    assert!(wait_logs_follower(&c, false, 10).await);
}

/// Commit (1.6+): works on a running and on a stopped container, and the
/// new image shows up in `image ls`.
#[tokio::test]
#[ignore]
async fn commit_running_and_stopped_container() {
    live_only!();
    let mut g = Guard::default();
    let cname = g.container(&name("commit"));
    runtime::run_image(run_args(&cname)).await.expect("run");
    tokio::time::sleep(Duration::from_secs(2)).await;

    let live_ref = g.track_image(&format!("{}:live", name("commit-img")));
    runtime::commit_container(&cname, &live_ref)
        .await
        .expect("commit running");
    runtime::stop(&cname).await.expect("stop");
    let stopped_ref = g.track_image(&format!("{}:stopped", name("commit-img")));
    runtime::commit_container(&cname, &stopped_ref)
        .await
        .expect("commit stopped");

    let images = runtime::list_images().await.unwrap();
    for r in [&live_ref, &stopped_ref] {
        assert!(
            images.iter().any(|i| i.reference.ends_with(r.as_str())),
            "{r} missing from {:?}",
            images.iter().map(|i| &i.reference).collect::<Vec<_>>()
        );
    }

    // Bad references never reach the CLI.
    assert!(runtime::commit_container(&cname, "Bad Ref").await.is_err());
    assert!(runtime::commit_container(&cname, "-x").await.is_err());
}

/// Create a volume (with size + label) and a network (with subnet + label)
/// through the app's own argv builders; both show up in the list views.
#[tokio::test]
#[ignore]
async fn create_volume_and_network_with_options() {
    live_only!();
    let mut g = Guard::default();

    let vol = g.track_volume(&name("mk-vol"));
    runtime::create_volume(runtime::VolumeCreateArgs {
        name: vol.clone(),
        size: Some("64M".into()),
        labels: vec!["team=live".into()],
    })
    .await
    .expect("create volume");
    let listed = runtime::list_volumes().await.unwrap();
    let v = listed
        .iter()
        .find(|v| v.name == vol)
        .expect("volume listed");
    assert!((v.size - 64.0 / 1024.0).abs() < 1e-6, "size {}", v.size);
    let inspected = runtime::inspect_volume(&vol).await.unwrap();
    assert!(
        inspected.contains("team") && inspected.contains("live"),
        "{inspected}"
    );
    // Creating it again is a CLI error that reaches the caller.
    assert!(runtime::create_volume(runtime::VolumeCreateArgs {
        name: vol.clone(),
        ..Default::default()
    })
    .await
    .is_err());

    let net = g.track_network(&name("mk-net"));
    // A pid-derived 192.168.X.0/24 keeps parallel/previous runs from colliding.
    let octet = 100 + (std::process::id() % 100);
    let subnet = format!("192.168.{octet}.0/24");
    runtime::create_network(runtime::NetworkCreateArgs {
        name: net.clone(),
        subnet: Some(subnet.clone()),
        labels: vec!["team=live".into()],
        ..Default::default()
    })
    .await
    .expect("create network");
    let nets = runtime::list_networks().await.unwrap();
    let n = nets.iter().find(|n| n.name == net).expect("network listed");
    assert_eq!(n.subnet, subnet);

    // Bad input never reaches the CLI.
    assert!(runtime::create_volume(runtime::VolumeCreateArgs {
        name: "-x".into(),
        ..Default::default()
    })
    .await
    .is_err());
    assert!(runtime::create_network(runtime::NetworkCreateArgs {
        name: "Bad_Upper".into(),
        ..Default::default()
    })
    .await
    .is_err());
}

/// Machines (1.6+ user options): create a small alpine machine with a custom
/// account, then read it back via inspect, fetch both log kinds, delete it.
/// Alpine is already pulled by `ensure_alpine`, so nothing large downloads.
#[tokio::test]
#[ignore]
async fn machine_user_options_inspect_and_logs() {
    live_only!();
    let mut g = Guard::default();
    let m = g.track_machine(&name("mach"));
    let opts = runtime::MachineUserOpts {
        user: Some("cguilive".into()),
        uid: Some(1234),
        gid: Some(1234),
        home: Some("/home/cguilive".into()),
    };
    runtime::machine_create(&m, "alpine:latest", None, None, false, &opts)
        .await
        .expect("machine create");

    let listed = runtime::machine_list().await.unwrap();
    assert!(listed.iter().any(|x| x.id == m), "{listed:?}");

    let inspected = runtime::machine_inspect(&m).await.expect("inspect");
    // `machine inspect` prints a one-element array.
    let v: serde_json::Value = serde_json::from_str(&inspected).expect("json");
    let v = &v[0];
    assert_eq!(v["id"], m.as_str());
    assert_eq!(v["userSetup"]["username"], "cguilive");
    assert_eq!(v["userSetup"]["uid"], 1234);
    assert_eq!(v["userSetup"]["home"], "/home/cguilive");

    let boot = runtime::machine_logs(&m, true, Some(20))
        .await
        .expect("boot log");
    assert!(!boot.trim().is_empty(), "boot log empty");
    assert!(boot.lines().count() <= 20, "-n not honoured");
    // stdio may legitimately be empty, but the call must succeed once booted.
    runtime::machine_logs(&m, false, Some(5))
        .await
        .expect("stdio log");

    runtime::machine_delete(&m).await.expect("delete");
    let after = runtime::machine_list().await.unwrap();
    assert!(after.iter().all(|x| x.id != m));
}

/// DNS aliases (1.6+): run with `--network net,alias=...` and read them back
/// from `ls` (`configuration.networks[].options.aliases`) and `inspect`.
#[tokio::test]
#[ignore]
async fn run_with_dns_aliases_reads_back() {
    live_only!();
    let mut g = Guard::default();
    let net = g.network(&name("al-net"));
    let cname = g.container(&name("alias"));
    let args = RunArgs {
        network: Some(net.clone()),
        aliases: vec!["web.example.test".into(), "www.example.test".into()],
        ..run_args(&cname)
    };
    runtime::run_image(args).await.expect("run with aliases");

    let ls: serde_json::Value =
        serde_json::from_str(&cli_ok(&["ls", "--all", "--format", "json"])).unwrap();
    let mine = ls
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["configuration"]["id"] == cname.as_str())
        .expect("container in ls");
    let nets = mine["configuration"]["networks"].as_array().unwrap();
    assert_eq!(nets[0]["network"], net.as_str());
    let aliases: Vec<&str> = nets[0]["options"]["aliases"]
        .as_array()
        .expect("aliases in ls json")
        .iter()
        .map(|a| a.as_str().unwrap())
        .collect();
    assert_eq!(aliases, ["web.example.test", "www.example.test"]);

    // The same data is in `inspect` (what the detail modal shows).
    let inspected = runtime::inspect(&cname).await.expect("inspect");
    assert!(inspected.contains("web.example.test"), "{inspected}");

    // An alias without a dot needs a default DNS domain; the CLI says so
    // and we surface its message rather than hiding it.
    let single = RunArgs {
        network: Some(net.clone()),
        aliases: vec!["justone".into()],
        ..run_args(&g.container(&name("alias-bad")))
    };
    if let Err(e) = runtime::run_image(single).await {
        assert!(format!("{e:#}").contains("fully qualified"), "{e:#}");
    }
    // Invalid names never reach the CLI.
    let bad = RunArgs {
        aliases: vec!["a,alias=b".into()],
        ..run_args(&name("alias-inj"))
    };
    assert!(runtime::run_image(bad).await.is_err());
}

/// Stats (1.7.1+): `cpuUtilization` is a fraction of ONE core, and the
/// first poll already reports it (no baseline poll needed).
#[tokio::test]
#[ignore]
async fn stats_native_cpu_utilization() {
    live_only!();
    let mut g = Guard::default();
    let cname = g.container(&name("cpu"));
    let busy = RunArgs {
        command: Some("sh -c 'while :; do :; done'".into()),
        ..run_args(&cname)
    };
    runtime::run_image(busy).await.expect("run");
    tokio::time::sleep(Duration::from_secs(3)).await;

    let version = runtime::cli_version().await.expect("version");
    // `stats --no-stream` samples every running container for ~2 s; when other
    // live tests create/delete containers in that window the CLI call can fail,
    // so retry a few times instead of flaking.
    let mut last_err = None;
    let mut rows = Vec::new();
    for _ in 0..5 {
        match runtime::stats_snapshot().await {
            Ok(r) if r.iter().any(|r| r.id == cname) => {
                rows = r;
                break;
            }
            Ok(_) => last_err = Some("row missing".to_string()),
            Err(e) => last_err = Some(format!("{e:#}")),
        }
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
    let row = rows
        .iter()
        .find(|r| r.id == cname)
        .unwrap_or_else(|| panic!("stat row for {cname}: {last_err:?}"));
    let Some(frac) = row.cpu_utilization else {
        eprintln!("CLI {version} has no cpuUtilization (needs 1.7.1+); skipping");
        return;
    };
    assert!(
        (0.5..=4.2).contains(&frac),
        "a busy loop should use about one core, got {frac}"
    );

    // The UI value (percent of one core) from a single poll with no delta
    // baseline, so it can only come from cpuUtilization. A poll's stats call
    // can miss while other live tests churn containers (the app then shows
    // 0 for that tick), so allow a few fresh attempts — each with a NEW
    // History, i.e. still no baseline.
    let mut last = 0.0;
    for _ in 0..5 {
        let history = app_lib::state::History::new();
        let cs = app_lib::state::poll_once(&history).await.expect("poll");
        let c = cs.iter().find(|c| c.id == cname).expect("listed");
        last = c.cpu;
        if (50.0..=420.0).contains(&last) {
            break;
        }
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
    assert!(
        (50.0..=420.0).contains(&last),
        "first-poll cpu% should come from cpuUtilization, got {last}"
    );
}

/// Capability gate: the cached CLI version parses and matches `--version`.
#[tokio::test]
#[ignore]
async fn cli_version_is_detected_and_cached() {
    live_only!();
    let v = runtime::cli_version().await.expect("cli version");
    assert!(v.chars().next().unwrap().is_ascii_digit(), "{v}");
    let raw = cli_ok(&["--version"]);
    assert!(raw.contains(&v), "`{raw}` should contain {v}");
    assert_eq!(runtime::cli_version().await, Some(v));
}
