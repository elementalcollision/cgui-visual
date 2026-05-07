// Compose-style stacks loaded from ~/.config/cgui/stacks/*.toml.
// Schema mirrors cgui's stacks.rs. We parse the full schema (env, ports,
// volumes, network, depends_on, args, restart, cap_add/drop, healthcheck)
// so that up/down can build correct `container run` argvs and the
// healthcheck poller can probe each service.

use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::Duration;
use tokio::net::TcpStream;
use tokio::process::Command;

use crate::model::{Container, Service as UiService, Stack as UiStack};
use crate::runtime;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct StackToml {
    pub name: String,
    #[serde(rename = "service", default, skip_serializing_if = "Vec::is_empty")]
    pub services: Vec<ServiceToml>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct ServiceToml {
    pub name: String,
    pub image: String,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub env: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ports: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub volumes: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub network: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub depends_on: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub args: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub restart: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub cap_add: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub cap_drop: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub healthcheck: Option<Healthcheck>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct Healthcheck {
    /// "tcp" (default), "http", or "cmd".
    #[serde(default = "default_kind")]
    pub kind: String,
    /// For tcp: a port number ("5432") or "host:port".
    /// For http: a port ("8080" → http://127.0.0.1:8080/), a "port/path"
    /// ("8080/health"), or a full URL ("http://host:port/path").
    /// Ignored for cmd.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
    /// For cmd: argv passed to `container exec <stack>_<svc>`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub command: Vec<String>,
    /// HTTP only: success status range. Empty → 200..=399.
    /// Two entries `[lo, hi]` → range; one entry `[code]` → exact match.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub expect_status: Vec<u16>,
    #[serde(default = "default_interval")]
    pub interval_s: u64,
    /// Compose-style startup grace. Probes still run inside the period
    /// but failures surface as "starting" rather than "unhealthy". Counts
    /// from when the probe first sees the container exists.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub start_period_s: u64,
}

fn is_zero(n: &u64) -> bool {
    *n == 0
}

fn default_kind() -> String {
    "tcp".into()
}
fn default_interval() -> u64 {
    30
}

pub fn stacks_dir() -> Option<PathBuf> {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))?;
    Some(base.join("cgui").join("stacks"))
}

fn load_all_toml() -> Vec<(StackToml, PathBuf)> {
    let Some(dir) = stacks_dir() else {
        return vec![];
    };
    let Ok(rd) = std::fs::read_dir(&dir) else {
        return vec![];
    };
    let mut out = Vec::new();
    for entry in rd.flatten() {
        let p = entry.path();
        if p.extension().and_then(|x| x.to_str()) != Some("toml") {
            continue;
        }
        let Ok(s) = std::fs::read_to_string(&p) else {
            continue;
        };
        if let Ok(stack) = toml::from_str::<StackToml>(&s) {
            out.push((stack, p));
        }
    }
    out.sort_by(|a, b| a.0.name.cmp(&b.0.name));
    out
}

pub fn load_one(name: &str) -> Result<StackToml> {
    let stacks = load_all_toml();
    stacks
        .into_iter()
        .find_map(|(s, _)| (s.name == name).then_some(s))
        .ok_or_else(|| anyhow!("stack '{name}' not found in ~/.config/cgui/stacks/"))
}

pub async fn list_stacks() -> Vec<UiStack> {
    let stacks = load_all_toml();
    if stacks.is_empty() {
        return vec![];
    }

    let containers: Vec<Container> = match runtime::list_containers_raw().await {
        Ok((cs, _)) => cs,
        Err(_) => vec![],
    };

    stacks
        .into_iter()
        .map(|(s, path)| stack_to_ui(s, &path, &containers))
        .collect()
}

fn stack_to_ui(s: StackToml, path: &std::path::Path, containers: &[Container]) -> UiStack {
    let services: Vec<UiService> = s
        .services
        .iter()
        .map(|svc| {
            let want = format!("{}_{}", s.name, svc.name);
            let state = containers
                .iter()
                .find(|c| c.name == want)
                .map(|c| c.status.clone())
                .unwrap_or_else(|| "stopped".into());
            // Health placeholder — overlaid by health_check_stack at the
            // frontend before display when the user is on the Stacks tab.
            let health = match state.as_str() {
                "running" => "healthy",
                "paused" => "waiting",
                _ => "—",
            }
            .to_string();
            UiService {
                name: svc.name.clone(),
                image: svc.image.clone(),
                state,
                health,
            }
        })
        .collect();

    let running = services.iter().filter(|sv| sv.state == "running").count();
    let total = services.len();
    let stack_health = if total > 0 && running == total {
        format!("✓ healthy ({total})")
    } else if running > 0 {
        "partial".to_string()
    } else {
        "—".to_string()
    };
    let restart = first_restart(&s.services).unwrap_or_else(|| "—".into());

    UiStack {
        name: s.name,
        services,
        restart,
        health: stack_health,
        file: path.display().to_string(),
    }
}

fn first_restart(svcs: &[ServiceToml]) -> Option<String> {
    let first = svcs.iter().find_map(|s| s.restart.clone())?;
    if svcs
        .iter()
        .all(|s| s.restart.as_deref() == Some(&first) || s.restart.is_none())
    {
        Some(first)
    } else {
        Some("—".into())
    }
}

// ─── Topo order for up/down ───────────────────────────────────────────

// Kahn's algorithm. Cycles fall through with the still-pending services
// appended at the end (matches cgui's tolerant behavior — better than
// failing the whole stack on a self-edge typo).
pub fn topo_order(stack: &StackToml) -> Vec<&ServiceToml> {
    use std::collections::{HashMap, HashSet, VecDeque};
    let names: HashSet<&str> = stack.services.iter().map(|s| s.name.as_str()).collect();
    let mut in_deg: HashMap<&str, usize> = stack
        .services
        .iter()
        .map(|s| (s.name.as_str(), 0))
        .collect();
    let mut deps: HashMap<&str, Vec<&str>> = stack
        .services
        .iter()
        .map(|s| (s.name.as_str(), vec![]))
        .collect();
    for s in &stack.services {
        for d in &s.depends_on {
            if names.contains(d.as_str()) {
                *in_deg.get_mut(s.name.as_str()).unwrap() += 1;
                deps.get_mut(d.as_str()).unwrap().push(s.name.as_str());
            }
        }
    }
    let mut q: VecDeque<&str> = in_deg
        .iter()
        .filter(|(_, d)| **d == 0)
        .map(|(k, _)| *k)
        .collect();
    let mut out: Vec<&str> = Vec::new();
    while let Some(n) = q.pop_front() {
        out.push(n);
        for child in deps[n].clone() {
            let d = in_deg.get_mut(child).unwrap();
            *d -= 1;
            if *d == 0 {
                q.push_back(child);
            }
        }
    }
    // Append any cycle-stuck remainder so the user can still try.
    for s in &stack.services {
        if !out.contains(&s.name.as_str()) {
            out.push(s.name.as_str());
        }
    }
    let by_name: HashMap<&str, &ServiceToml> = stack
        .services
        .iter()
        .map(|s| (s.name.as_str(), s))
        .collect();
    out.into_iter().map(|n| by_name[n]).collect()
}

// ─── Up / Down ────────────────────────────────────────────────────────

// Build argv for `container run -d --name <stack>_<svc> ...`.
fn run_args(stack: &str, svc: &ServiceToml) -> Vec<String> {
    let mut argv: Vec<String> = vec!["run".into(), "-d".into()];
    argv.push("--name".into());
    argv.push(format!("{stack}_{}", svc.name));
    // Compose-project label so `list_containers` groups these with this
    // stack (matches the docker-compose convention cgui reads).
    argv.push("--label".into());
    argv.push(format!("com.docker.compose.project={stack}"));
    if let Some(rp) = svc.restart.as_deref() {
        if matches!(rp, "always" | "on-failure") {
            argv.push("--restart".into());
            argv.push(rp.into());
        }
    }
    if let Some(net) = svc.network.as_deref() {
        argv.push("--network".into());
        argv.push(net.into());
    }
    for p in &svc.ports {
        argv.push("-p".into());
        argv.push(p.clone());
    }
    for v in &svc.volumes {
        argv.push("-v".into());
        argv.push(v.clone());
    }
    for (k, v) in &svc.env {
        argv.push("-e".into());
        argv.push(format!("{k}={v}"));
    }
    for c in &svc.cap_add {
        argv.push("--cap-add".into());
        argv.push(c.clone());
    }
    for c in &svc.cap_drop {
        argv.push("--cap-drop".into());
        argv.push(c.clone());
    }
    argv.push(svc.image.clone());
    for a in &svc.args {
        argv.push(a.clone());
    }
    argv
}

pub async fn stack_up(name: &str) -> Result<Vec<String>> {
    let stack = load_one(name)?;
    let mut log: Vec<String> = Vec::new();
    for svc in topo_order(&stack) {
        let argv = run_args(&stack.name, svc);
        let argv_ref: Vec<&str> = argv.iter().map(String::as_str).collect();
        log.push(format!("$ container {}", argv.join(" ")));
        match runtime_run(&argv_ref).await {
            Ok(line) => log.push(line),
            Err(e) => {
                log.push(format!("✗ {}: {e:#}", svc.name));
                return Err(
                    anyhow!("stack '{name}' up aborted at service '{}': {e:#}", svc.name)
                        .context(log.join("\n")),
                );
            }
        }
    }
    log.push(format!("✓ stack '{name}' up"));
    Ok(log)
}

pub async fn stack_down(name: &str) -> Result<Vec<String>> {
    let stack = load_one(name)?;
    let mut log: Vec<String> = Vec::new();
    // Reverse topo: stop dependents before what they depend on.
    let order: Vec<_> = topo_order(&stack).into_iter().rev().collect();
    for svc in order {
        let cname = format!("{}_{}", stack.name, svc.name);
        log.push(format!("$ container stop {cname}"));
        let _ = runtime_run(&["stop", &cname]).await;
        log.push(format!("$ container delete {cname}"));
        let _ = runtime_run(&["delete", &cname]).await;
    }
    log.push(format!("✓ stack '{name}' down"));
    Ok(log)
}

// Thin wrapper so we can call the same `runtime::run`-style behavior
// without exposing it from runtime.rs.
async fn runtime_run(args: &[&str]) -> Result<String> {
    let out = Command::new("container")
        .args(args)
        .output()
        .await
        .with_context(|| format!("spawn container {}", args.join(" ")))?;
    if !out.status.success() {
        return Err(anyhow!(
            "container {} exited {}: {}",
            args.join(" "),
            out.status,
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

// ─── Healthchecks ─────────────────────────────────────────────────────

pub async fn health_check_stack(name: &str) -> Result<Vec<(String, String)>> {
    let stack = load_one(name)?;
    // Pull the live container list once so we can compute elapsed-since-
    // start for each service's grace window. Fetch failures fall back to
    // an empty list (probes still run; grace just won't activate).
    let containers: Vec<Container> = match runtime::list_containers_raw().await {
        Ok((cs, _)) => cs,
        Err(_) => vec![],
    };
    let now_unix = chrono::Utc::now().timestamp();

    let mut out = Vec::with_capacity(stack.services.len());
    for svc in &stack.services {
        let raw_state = match svc.healthcheck.as_ref() {
            None => "—".to_string(),
            Some(hc) => match hc.kind.as_str() {
                "tcp" => probe_tcp(hc.target.as_deref()).await,
                "http" => probe_http(hc.target.as_deref(), &hc.expect_status).await,
                "cmd" => probe_cmd(&stack.name, &svc.name, &hc.command).await,
                other => format!("unsupported:{other}"),
            },
        };
        let elapsed = elapsed_secs(&containers, &stack.name, &svc.name, now_unix);
        let grace = svc
            .healthcheck
            .as_ref()
            .map(|h| h.start_period_s)
            .unwrap_or(0);
        out.push((
            svc.name.clone(),
            classify_health(&raw_state, elapsed, grace),
        ));
    }
    Ok(out)
}

// Look up `<stack>_<svc>` in the runtime list; return seconds since its
// container started, or None if it isn't running yet.
fn elapsed_secs(containers: &[Container], stack: &str, svc: &str, now_unix: i64) -> Option<i64> {
    let want = format!("{stack}_{svc}");
    containers
        .iter()
        .find(|c| c.name == want)
        .and_then(|c| c.started_unix)
        .map(|t| (now_unix - t).max(0))
}

// Apply the start-period grace: a failing probe within the grace window
// surfaces as `starting (N/Ms)` instead of `unhealthy`. Healthy probes
// always pass through unchanged — the grace only suppresses the
// negative result so the user isn't alarmed by a service still booting.
fn classify_health(raw: &str, elapsed: Option<i64>, grace: u64) -> String {
    if raw != "unhealthy" || grace == 0 {
        return raw.to_string();
    }
    let Some(e) = elapsed else {
        return raw.to_string();
    };
    let g = grace as i64;
    if e < g {
        format!("starting ({e}/{g}s)")
    } else {
        raw.to_string()
    }
}

async fn probe_tcp(target: Option<&str>) -> String {
    let Some(target) = target else {
        return "—".into();
    };
    // Accept "5432" → 127.0.0.1:5432, or "host:port" verbatim.
    let addr = if target.contains(':') {
        target.to_string()
    } else {
        format!("127.0.0.1:{target}")
    };
    let fut = TcpStream::connect(&addr);
    match tokio::time::timeout(Duration::from_secs(2), fut).await {
        Ok(Ok(_)) => "healthy".into(),
        Ok(Err(_)) | Err(_) => "unhealthy".into(),
    }
}

// HTTP probe via curl. Issues a GET, takes the response status code, and
// matches against the configured range. We shell out to curl to avoid
// pulling reqwest+TLS into the build (same call style as updates.rs).
async fn probe_http(target: Option<&str>, expect: &[u16]) -> String {
    let Some(t) = target else { return "—".into() };
    let url = http_url_for(t);
    let fut = Command::new("curl")
        .args([
            "-s",
            "-S",
            "-o",
            "/dev/null",
            "-w",
            "%{http_code}",
            "--max-time",
            "3",
            "--connect-timeout",
            "2",
            &url,
        ])
        .output();
    let out = match tokio::time::timeout(Duration::from_secs(4), fut).await {
        Ok(Ok(o)) => o,
        _ => return "unhealthy".into(),
    };
    if !out.status.success() {
        return "unhealthy".into();
    }
    let code: u16 = String::from_utf8_lossy(&out.stdout)
        .trim()
        .parse()
        .unwrap_or(0);
    if status_matches(code, expect) {
        "healthy".into()
    } else {
        "unhealthy".into()
    }
}

// Build an http URL from the configured target form. Accepts:
//   "8080"            → http://127.0.0.1:8080/
//   "8080/health"     → http://127.0.0.1:8080/health
//   "host:8080/path"  → http://host:8080/path
//   "http(s)://..."   → passed through verbatim
fn http_url_for(target: &str) -> String {
    if target.starts_with("http://") || target.starts_with("https://") {
        return target.to_string();
    }
    // host:port[/path]
    if target.contains(':') {
        return format!("http://{target}");
    }
    // port[/path] — anchor to localhost
    if let Some((port, path)) = target.split_once('/') {
        format!("http://127.0.0.1:{port}/{path}")
    } else {
        format!("http://127.0.0.1:{target}/")
    }
}

// Status-range match. Empty `expect` means default 200..=399.
// Single entry → exact code. Two entries → inclusive [lo, hi] range.
// 3+ entries → exact-match against the set.
fn status_matches(code: u16, expect: &[u16]) -> bool {
    match expect.len() {
        0 => (200..=399).contains(&code),
        1 => code == expect[0],
        2 => {
            let (lo, hi) = (expect[0].min(expect[1]), expect[0].max(expect[1]));
            (lo..=hi).contains(&code)
        }
        _ => expect.contains(&code),
    }
}

// cmd probe: `container exec <stack>_<svc> <argv>`. Success iff exit 0.
// Empty command argv reports "—" (nothing to do). 3 s timeout.
async fn probe_cmd(stack: &str, svc: &str, command: &[String]) -> String {
    if command.is_empty() {
        return "—".into();
    }
    let cname = format!("{stack}_{svc}");
    let mut argv: Vec<String> = vec!["exec".into(), cname];
    for a in command {
        argv.push(a.clone());
    }
    let argv_ref: Vec<&str> = argv.iter().map(String::as_str).collect();
    let fut = Command::new("container").args(&argv_ref).output();
    match tokio::time::timeout(Duration::from_secs(3), fut).await {
        Ok(Ok(o)) if o.status.success() => "healthy".into(),
        Ok(Ok(_)) => "unhealthy".into(),
        Ok(Err(_)) | Err(_) => "unhealthy".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stack(deps: &[(&str, &[&str])]) -> StackToml {
        StackToml {
            name: "t".into(),
            services: deps
                .iter()
                .map(|(n, ds)| ServiceToml {
                    name: (*n).into(),
                    image: "x".into(),
                    depends_on: ds.iter().map(|s| (*s).into()).collect(),
                    ..Default::default()
                })
                .collect(),
        }
    }

    #[test]
    fn topo_orders_dependents_after_dependencies() {
        let s = stack(&[("a", &[]), ("b", &["a"]), ("c", &["b"])]);
        let order: Vec<_> = topo_order(&s)
            .into_iter()
            .map(|s| s.name.as_str())
            .collect();
        assert_eq!(order, vec!["a", "b", "c"]);
    }

    #[test]
    fn topo_handles_cycles_by_appending_remainder() {
        // a depends on b, b depends on a — neither has in-deg 0 ever.
        let s = stack(&[("a", &["b"]), ("b", &["a"])]);
        let order: Vec<_> = topo_order(&s)
            .into_iter()
            .map(|s| s.name.as_str())
            .collect();
        assert_eq!(order.len(), 2);
        assert!(order.contains(&"a") && order.contains(&"b"));
    }

    #[test]
    fn run_args_includes_compose_label_and_name() {
        let svc = ServiceToml {
            name: "db".into(),
            image: "pgvector:pg16".into(),
            ports: vec!["5432:5432".into()],
            env: [("POSTGRES_USER".to_string(), "x".to_string())]
                .into_iter()
                .collect(),
            ..Default::default()
        };
        let argv = run_args("myapp", &svc);
        assert!(argv.contains(&"--name".to_string()));
        assert!(argv.contains(&"myapp_db".to_string()));
        assert!(argv.contains(&"--label".to_string()));
        assert!(argv.contains(&"com.docker.compose.project=myapp".to_string()));
        assert!(argv.contains(&"-p".to_string()));
        assert!(argv.contains(&"5432:5432".to_string()));
        assert!(argv.contains(&"-e".to_string()));
        assert!(argv.contains(&"POSTGRES_USER=x".to_string()));
        assert!(argv.contains(&"pgvector:pg16".to_string()));
    }

    #[test]
    fn http_url_for_handles_each_target_form() {
        assert_eq!(http_url_for("8080"), "http://127.0.0.1:8080/");
        assert_eq!(http_url_for("8080/health"), "http://127.0.0.1:8080/health");
        assert_eq!(
            http_url_for("api:8080/v1/healthz"),
            "http://api:8080/v1/healthz"
        );
        assert_eq!(
            http_url_for("https://example.com/x"),
            "https://example.com/x"
        );
        assert_eq!(
            http_url_for("http://localhost:9090/"),
            "http://localhost:9090/"
        );
    }

    #[test]
    fn status_matches_default_range() {
        // Empty expect → 200..=399 inclusive.
        assert!(status_matches(200, &[]));
        assert!(status_matches(301, &[]));
        assert!(status_matches(399, &[]));
        assert!(!status_matches(199, &[]));
        assert!(!status_matches(400, &[]));
        assert!(!status_matches(500, &[]));
    }

    #[test]
    fn classify_health_passes_healthy_through_regardless_of_grace() {
        assert_eq!(classify_health("healthy", Some(0), 30), "healthy");
        assert_eq!(classify_health("healthy", Some(60), 30), "healthy");
        assert_eq!(classify_health("healthy", None, 30), "healthy");
    }

    #[test]
    fn classify_health_within_grace_reports_starting() {
        assert_eq!(
            classify_health("unhealthy", Some(0), 30),
            "starting (0/30s)"
        );
        assert_eq!(
            classify_health("unhealthy", Some(12), 30),
            "starting (12/30s)"
        );
        assert_eq!(
            classify_health("unhealthy", Some(29), 30),
            "starting (29/30s)"
        );
    }

    #[test]
    fn classify_health_at_or_past_grace_reports_unhealthy() {
        assert_eq!(classify_health("unhealthy", Some(30), 30), "unhealthy");
        assert_eq!(classify_health("unhealthy", Some(45), 30), "unhealthy");
    }

    #[test]
    fn classify_health_no_grace_or_no_start_falls_through() {
        // No grace configured → unhealthy is unhealthy.
        assert_eq!(classify_health("unhealthy", Some(5), 0), "unhealthy");
        // Container not running → no elapsed → can't apply grace, fall through.
        assert_eq!(classify_health("unhealthy", None, 30), "unhealthy");
    }

    #[test]
    fn status_matches_exact_and_range_forms() {
        // Single entry → exact match.
        assert!(status_matches(204, &[204]));
        assert!(!status_matches(200, &[204]));
        // Two entries → inclusive range, order-tolerant.
        assert!(status_matches(250, &[200, 299]));
        assert!(status_matches(299, &[299, 200])); // reversed
        assert!(!status_matches(300, &[200, 299]));
        // Three+ entries → set membership.
        assert!(status_matches(401, &[401, 403, 404]));
        assert!(!status_matches(402, &[401, 403, 404]));
    }
}
