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
    files: Vec<std::path::PathBuf>,
}

impl Guard {
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
