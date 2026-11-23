use super::*;

#[derive(Debug, Default, Clone, Deserialize)]
#[serde(default)]
pub struct StatRow {
    pub id: String,
    // Apple's `container stats` reports cumulative CPU time in microseconds.
    // CPU% is computed in state.rs via deltas across polls.
    #[serde(rename = "cpuUsageUsec")]
    pub cpu_usage_usec: u64,
    #[serde(rename = "memoryUsageBytes")]
    pub memory_usage_bytes: u64,
    #[serde(rename = "memoryLimitBytes")]
    pub memory_limit_bytes: u64,
    #[serde(rename = "networkRxBytes", default)]
    pub network_rx_bytes: u64,
    #[serde(rename = "networkTxBytes", default)]
    pub network_tx_bytes: u64,
    #[serde(rename = "blockReadBytes", default)]
    pub block_read_bytes: u64,
    #[serde(rename = "blockWriteBytes", default)]
    pub block_write_bytes: u64,
    /// CPU use as a fraction of ONE core (1.0 = one core fully busy),
    /// sampled by the CLI over ~2 s. Present since container 1.7.1; older
    /// CLIs omit it and state.rs falls back to `cpu_usage_usec` deltas.
    #[serde(rename = "cpuUtilization", default)]
    pub cpu_utilization: Option<f64>,
}

pub(super) fn parse_container(v: Value) -> Container {
    let cfg = v.get("configuration").cloned().unwrap_or(Value::Null);
    let id = cfg
        .get("id")
        .and_then(Value::as_str)
        .unwrap_or("?")
        .to_string();
    let image = cfg
        .get("image")
        .and_then(|i| i.get("reference"))
        .and_then(Value::as_str)
        .unwrap_or("?")
        .to_string();
    // `status` is `{"state": "running", "startedDate": "...", "networks": [...]}`.
    let status = v
        .get("status")
        .and_then(|s| s.get("state"))
        .and_then(Value::as_str)
        .unwrap_or("unknown")
        .to_string();
    let memory_bytes = cfg
        .get("resources")
        .and_then(|r| r.get("memoryInBytes"))
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let ports = cfg
        .get("publishedPorts")
        .and_then(Value::as_array)
        .map(|arr| arr.iter().map(format_published_port).collect())
        .unwrap_or_default();

    let mem_limit_gib = bytes_to_gib(memory_bytes);
    let stack = compose_project(&cfg);

    // Display-only short id; `Container.id` stays the full CLI id.
    let short_id = short_container_id(&id);

    // `status.startedDate` (RFC3339) is present while running; the creation
    // time is `configuration.creationDate`.
    let started_at = v
        .get("status")
        .and_then(|s| s.get("startedDate"))
        .and_then(Value::as_str)
        .and_then(crate::timeutil::parse_rfc3339_unix);
    let started_unix = started_at;
    let started_iso = v
        .get("status")
        .and_then(|s| s.get("startedDate"))
        .or_else(|| cfg.get("creationDate"))
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();

    let normalized = normalize_status(&status);
    let uptime = if normalized == "running" {
        match started_unix {
            Some(t) => format_uptime_from_unix(t),
            None => "—".into(),
        }
    } else {
        "—".into()
    };

    // Hostname lives at `networks[0].options.hostname`.
    let name = cfg
        .get("hostname")
        .and_then(Value::as_str)
        .or_else(|| {
            cfg.get("networks")
                .and_then(|n| n.get(0))
                .and_then(|n| n.get("options"))
                .and_then(|o| o.get("hostname"))
                .and_then(Value::as_str)
        })
        .unwrap_or(&id)
        .to_string();

    let cmd = cfg
        .get("initProcess")
        .map(|p| {
            let exe = p.get("executable").and_then(Value::as_str).unwrap_or("");
            let args = p
                .get("arguments")
                .and_then(Value::as_array)
                .map(|a| {
                    a.iter()
                        .filter_map(|x| x.as_str().map(String::from))
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            let mut out = Vec::with_capacity(1 + args.len());
            if !exe.is_empty() {
                out.push(exe.to_string());
            }
            out.extend(args);
            out
        })
        .unwrap_or_default();

    Container {
        id,
        short_id,
        name,
        image,
        status: normalized,
        uptime,
        exit_code: None,
        cpu: 0.0,
        mem: MemUsage {
            used: 0.0,
            limit: mem_limit_gib,
            unit: "GiB".into(),
            pct: 0.0,
        },
        ports,
        stack,
        created: started_iso,
        cpu_history: vec![0.0; 24],
        cmd,
        net_io_bps: 0.0,
        disk_io_bps: 0.0,
        started_unix,
    }
}

/// One `configuration.publishedPorts[]` entry as `host:container`, the
/// `-p` order. `count` > 1 publishes consecutive ports, shown as a range:
/// `{hostPort: 9000, containerPort: 7000, count: 3}` → `9000-9002:7000-7002`.
fn format_published_port(p: &Value) -> String {
    let host = p.get("hostPort").and_then(Value::as_u64).unwrap_or(0);
    let cont = p.get("containerPort").and_then(Value::as_u64).unwrap_or(0);
    let count = p.get("count").and_then(Value::as_u64).unwrap_or(1);
    if count > 1 {
        let last = count - 1;
        format!(
            "{host}-{}:{cont}-{}",
            host.saturating_add(last),
            cont.saturating_add(last)
        )
    } else {
        format!("{host}:{cont}")
    }
}

fn format_uptime_from_unix(unix_secs: i64) -> String {
    let secs = (crate::timeutil::now_unix() - unix_secs).max(0);
    format_duration(secs)
}

fn format_duration(secs: i64) -> String {
    let d = secs / 86_400;
    let h = (secs % 86_400) / 3600;
    let m = (secs % 3600) / 60;
    if d > 0 {
        format!("{d}d {h:02}h")
    } else if h > 0 {
        format!("{h}h {m:02}m")
    } else if m > 0 {
        format!("{m}m")
    } else {
        format!("{secs}s")
    }
}

fn bytes_to_gib(bytes: u64) -> f64 {
    (bytes as f64) / (1024.0 * 1024.0 * 1024.0)
}

// `container` returns lowercase status strings already; normalize a few
// variants so the UI's status-pill switch hits.
fn normalize_status(s: &str) -> String {
    let lower = s.to_ascii_lowercase();
    match lower.as_str() {
        "up" | "running" => "running".into(),
        "paused" => "paused".into(),
        "exited" | "stopped" | "created" => "exited".into(),
        other => other.into(),
    }
}

// Compose project label, mirrors what `cgui` reads for stack grouping.
fn compose_project(cfg: &Value) -> Option<String> {
    cfg.get("labels")
        .and_then(Value::as_object)
        .and_then(|m| m.get("com.docker.compose.project"))
        .and_then(Value::as_str)
        .map(String::from)
}

/// Pure core of `list_images_checked`. A non-zero exit is accepted only
/// when stdout is a JSON array (the 1.6+ partial-output behaviour);
/// anything else (services down, garbage) stays an error.
pub(super) fn parse_image_ls(
    success: bool,
    stdout: &[u8],
    stderr: &str,
    status: &dyn std::fmt::Display,
) -> Result<ImageListing> {
    let parsed = serde_json::from_slice::<Vec<Value>>(stdout);
    match (success, parsed) {
        (true, parsed) => {
            let raw = parsed.context("parse `image ls` json")?;
            Ok(ImageListing {
                images: raw.into_iter().map(parse_image).collect(),
                warning: None,
            })
        }
        (false, Ok(raw)) => {
            let detail = stderr.trim();
            let lead = match failed_read_count(detail) {
                Some(1) => "1 image couldn't be read".to_string(),
                Some(n) => format!("{n} images couldn't be read"),
                None => "Some images couldn't be read".to_string(),
            };
            let warning = if detail.is_empty() {
                lead
            } else {
                format!("{lead}: {detail}")
            };
            Ok(ImageListing {
                images: raw.into_iter().map(parse_image).collect(),
                warning: Some(warning),
            })
        }
        (false, Err(_)) => Err(anyhow!("exited {status}: {}", stderr.trim())),
    }
}

/// N from the CLI's "failed to read N image(s)" summary line.
fn failed_read_count(stderr: &str) -> Option<usize> {
    let marker = "failed to read ";
    let rest = &stderr[stderr.find(marker)? + marker.len()..];
    let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
    digits.parse().ok()
}

pub(super) fn parse_image(v: Value) -> Image {
    // Metadata is nested under `configuration` (`name`, `creationDate`,
    // `descriptor`) with per-platform byte sizes in `variants[].size`.
    let cfg = v.get("configuration");
    let reference = cfg
        .and_then(|c| c.get("name"))
        .and_then(Value::as_str)
        .unwrap_or("?")
        .to_string();
    let bytes: u64 = v
        .get("variants")
        .and_then(Value::as_array)
        .map(|vs| {
            vs.iter()
                .filter_map(|p| p.get("size").and_then(Value::as_u64))
                .sum()
        })
        .unwrap_or(0);
    let size = bytes_to_gib(bytes);
    let digest = cfg
        .and_then(|c| c.get("descriptor"))
        .and_then(|d| d.get("digest"))
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let id = digest
        .split(':')
        .nth(1)
        .map(|h| format!("sha256:{}", &h[..h.len().min(8)]))
        .unwrap_or_else(|| digest.clone());
    Image {
        id,
        reference,
        size,
        size_unit: "GiB".into(),
        created: cfg
            .and_then(|c| c.get("creationDate"))
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string(),
        tags: v
            .get("tags")
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .filter_map(|x| x.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default(),
        digest,
        // `image ls` doesn't expose layer count directly; surface 0 for
        // now (a follow-up can call `image inspect` per row to get it).
        layers: 0,
    }
}

pub(super) fn parse_volume(v: Value, refs: &std::collections::HashMap<String, u32>) -> Volume {
    // `name`/`driver`/`source`/`sizeInBytes` live under `configuration`.
    let cfg = v.get("configuration").unwrap_or(&Value::Null);
    let name = cfg
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or("?")
        .to_string();
    let driver = cfg
        .get("driver")
        .and_then(Value::as_str)
        .unwrap_or("local")
        .to_string();
    let source = cfg
        .get("source")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let capacity_bytes = cfg.get("sizeInBytes").and_then(Value::as_u64).unwrap_or(0);
    let used_bytes = if source.is_empty() {
        0
    } else {
        std::fs::metadata(&source).map(|m| m.len()).unwrap_or(0)
    };
    let ref_count = refs.get(name.as_str()).copied().unwrap_or(0);
    Volume {
        name,
        driver,
        mountpoint: source,
        size: bytes_to_gib(capacity_bytes),
        used: bytes_to_gib(used_bytes),
        unit: "GiB".into(),
        refs: ref_count,
    }
}

// Scan container ls output for volume mounts and return a name → count map.
// Each volume mount is `{"type": {"volume": {"name": "<name>", ...}}, ...}`
// inside `configuration.mounts`; other mount kinds are skipped.
pub(super) fn volume_ref_counts(ls_bytes: &[u8]) -> std::collections::HashMap<String, u32> {
    use std::collections::HashMap;
    let raw: Vec<Value> = serde_json::from_slice(ls_bytes).unwrap_or_default();
    let mut out: HashMap<String, u32> = HashMap::new();
    for c in raw {
        let mounts = c
            .get("configuration")
            .and_then(|cfg| cfg.get("mounts"))
            .and_then(Value::as_array);
        let Some(mounts) = mounts else { continue };
        for m in mounts {
            // Mounts are `"type": {"volume": {"name": "...", ...}}`; other
            // kinds (`tmpfs`, `virtiofs`, ...) are skipped. `source` holds
            // the backing image path, not the volume name.
            if let Some(name) = m
                .get("type")
                .and_then(|t| t.get("volume"))
                .and_then(|vol| vol.get("name"))
                .and_then(Value::as_str)
            {
                *out.entry(name.to_string()).or_insert(0) += 1;
            }
        }
    }
    out
}

pub(super) fn parse_network(v: Value, refs: &std::collections::HashMap<String, u32>) -> Network {
    let cfg = v.get("configuration").cloned().unwrap_or(Value::Null);
    let name = cfg
        .get("name")
        .or_else(|| v.get("id"))
        .and_then(Value::as_str)
        .unwrap_or("?")
        .to_string();
    let mode = cfg
        .get("mode")
        .and_then(Value::as_str)
        .unwrap_or("?")
        .to_string();
    // Apple's CLI returns "running" for an up network; the UI treats
    // "active" as the up state. Normalize so the status dot lights up.
    let raw_state = v.get("state").and_then(Value::as_str).unwrap_or("active");
    let state = if matches!(raw_state, "running" | "active" | "up") {
        "active"
    } else {
        "stopped"
    }
    .to_string();
    let status = v.get("status");
    let subnet = status
        .and_then(|s| s.get("ipv4Subnet").or_else(|| s.get("ipv6Subnet")))
        .and_then(Value::as_str)
        .unwrap_or("—")
        .to_string();
    let gateway = status
        .and_then(|s| s.get("ipv4Gateway"))
        .and_then(Value::as_str)
        .unwrap_or("—")
        .to_string();
    let container_count = refs.get(name.as_str()).copied().unwrap_or(0);
    Network {
        id: name.clone(),
        name,
        mode,
        state,
        subnet,
        gateway,
        dns: vec![],
        containers: container_count,
    }
}

// Scan container ls output for attached networks and return a
// name → running-container count map.
//
// `status.networks[]` is the source of truth: each entry is `{"network":
// "<name>", "hostname": ..., "ipv4Address": ..., ...}` for a container that
// is actually attached right now (`[]` when stopped). `configuration.networks[]`
// lists the networks a container *would* attach to on start, so counting it
// would count stopped containers as attached.
pub(super) fn network_ref_counts(ls_bytes: &[u8]) -> std::collections::HashMap<String, u32> {
    use std::collections::HashMap;
    let raw: Vec<Value> = serde_json::from_slice(ls_bytes).unwrap_or_default();
    let mut out: HashMap<String, u32> = HashMap::new();
    for c in raw {
        let Some(nets) = c
            .get("status")
            .and_then(|s| s.get("networks"))
            .and_then(Value::as_array)
        else {
            continue;
        };
        for n in nets {
            if let Some(name) = n.get("network").and_then(Value::as_str) {
                *out.entry(name.to_string()).or_insert(0) += 1;
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn normalize_status_buckets_variants() {
        assert_eq!(normalize_status("running"), "running");
        assert_eq!(normalize_status("Up"), "running");
        assert_eq!(normalize_status("Paused"), "paused");
        assert_eq!(normalize_status("exited"), "exited");
        assert_eq!(normalize_status("created"), "exited");
        assert_eq!(normalize_status("dead"), "dead");
    }

    #[test]
    fn format_duration_picks_largest_unit() {
        assert_eq!(format_duration(45), "45s");
        assert_eq!(format_duration(125), "2m");
        assert_eq!(format_duration(3725), "1h 02m");
        assert_eq!(format_duration(90061), "1d 01h");
    }

    #[test]
    fn parse_container_1_0_shape() {
        // Verbatim structure from `container ls --format json` on 1.0.0:
        // status is an object, startedDate is RFC3339 under status, the
        // hostname moved to networks[0].options, and mount types are
        // objects keyed by kind.
        let v = json!({
            "id": "cgui-smoke",
            "status": {
                "state": "running",
                "startedDate": "2026-06-12T13:00:26Z",
                "networks": [{ "hostname": "cgui-smoke", "ipv4Address": "192.168.64.2/24" }]
            },
            "configuration": {
                "id": "cgui-smoke",
                "creationDate": "2026-06-12T13:00:24Z",
                "image": { "reference": "docker.io/library/alpine:latest" },
                "resources": { "cpus": 4, "memoryInBytes": 1073741824u64 },
                "publishedPorts": [],
                "networks": [{ "network": "default", "options": { "hostname": "cgui-smoke", "mtu": 1280 } }],
                "initProcess": { "executable": "sleep", "arguments": ["120"] },
                "labels": {}
            }
        });
        let c = parse_container(v);
        assert_eq!(c.status, "running");
        assert_eq!(c.name, "cgui-smoke");
        assert_eq!(c.image, "docker.io/library/alpine:latest");
        // RFC3339 startedDate must produce a real uptime, not "—".
        assert_ne!(c.uptime, "—");
        assert_eq!(c.cmd, vec!["sleep", "120"]);
    }

    #[test]
    fn published_port_ranges_show_in_full() {
        // `container create -p 127.0.0.1:9000-9002:7000-7002/udp -p 8082:81`
        // on 1.7.1 (configuration.publishedPorts, verbatim).
        let v = json!({
            "id": "ranges",
            "status": { "state": "stopped" },
            "configuration": {
                "id": "ranges",
                "publishedPorts": [
                    { "containerPort": 7000, "count": 3, "hostAddress": "127.0.0.1", "hostPort": 9000, "proto": "udp" },
                    { "containerPort": 81, "count": 1, "hostAddress": "0.0.0.0", "hostPort": 8082, "proto": "tcp" }
                ]
            }
        });
        assert_eq!(
            parse_container(v).ports,
            vec!["9000-9002:7000-7002", "8082:81"]
        );
        // Payloads without `count` keep the single-port form.
        assert_eq!(
            format_published_port(&json!({ "hostPort": 8080, "containerPort": 80 })),
            "8080:80"
        );
    }

    #[test]
    fn parse_image_1_0_shape() {
        let v = json!({
            "id": "5b10f432ef3da1b8d4c7eb6c487f2f5a8f096bc91145e68878dd4a5019afde11",
            "configuration": {
                "creationDate": "2026-04-15T20:00:31Z",
                "descriptor": { "digest": "sha256:5b10f432ef3da1b8d4c7eb6c487f2f5a8f096bc91145e68878dd4a5019afde11" },
                "name": "docker.io/library/alpine:latest"
            },
            "variants": [
                { "platform": { "architecture": "amd64", "os": "linux" }, "size": 3865822u64 },
                { "platform": { "architecture": "arm64", "os": "linux" }, "size": 3999999u64 }
            ]
        });
        let img = parse_image(v);
        assert_eq!(img.reference, "docker.io/library/alpine:latest");
        assert_eq!(img.created, "2026-04-15T20:00:31Z");
        assert_eq!(img.id, "sha256:5b10f432");
        // Variant sizes are summed bytes → GiB.
        assert!((img.size - ((3865822.0 + 3999999.0) / (1024.0 * 1024.0 * 1024.0))).abs() < 1e-9);
    }

    #[test]
    fn parse_volume_1_0_shape() {
        let v = json!({
            "id": "cgui-test-vol",
            "configuration": {
                "creationDate": "2026-06-12T13:00:15Z",
                "driver": "local",
                "format": "ext4",
                "name": "cgui-test-vol",
                "sizeInBytes": 549755813888u64,
                "source": "/nonexistent/volumes/cgui-test-vol/volume.img"
            }
        });
        let refs = std::collections::HashMap::from([("cgui-test-vol".to_string(), 2u32)]);
        let vol = parse_volume(v, &refs);
        assert_eq!(vol.name, "cgui-test-vol");
        assert_eq!(vol.driver, "local");
        assert_eq!(vol.refs, 2);
        assert!((vol.size - 512.0).abs() < 1e-6);
    }

    #[test]
    fn parse_network_1_0_shape() {
        let v = json!({
            "id": "default",
            "configuration": {
                "creationDate": "2026-04-11T19:00:16Z",
                "mode": "nat",
                "name": "default"
            },
            "status": {
                "ipv4Gateway": "192.168.64.1",
                "ipv4Subnet": "192.168.64.0/24",
                "ipv6Subnet": "fdea:57c1:ba3e:6cbe::/64"
            }
        });
        let n = parse_network(v, &std::collections::HashMap::new());
        assert_eq!(n.name, "default");
        assert_eq!(n.mode, "nat");
        assert_eq!(n.state, "active");
        assert_eq!(n.subnet, "192.168.64.0/24");
        assert_eq!(n.gateway, "192.168.64.1");
        assert_eq!(n.containers, 0);
    }

    #[test]
    fn parse_network_uses_ref_counts() {
        let v = json!({
            "id": "cgui-t-net",
            "configuration": { "mode": "nat", "name": "cgui-t-net" },
            "status": {}
        });
        let refs = std::collections::HashMap::from([("cgui-t-net".to_string(), 3u32)]);
        let n = parse_network(v, &refs);
        assert_eq!(n.containers, 3);
    }

    #[test]
    fn network_ref_counts_counts_only_running_attachments() {
        // Verbatim shape from `container ls --all --format json` on 1.4.1:
        // a running container's `status.networks` lists what it's actually
        // attached to; a stopped one reports `status.networks: []` even
        // though `configuration.networks` still names its intended network.
        let ls = json!([
            {
                "configuration": { "networks": [{ "network": "cgui-t-net" }] },
                "status": {
                    "state": "running",
                    "networks": [{ "network": "cgui-t-net", "hostname": "a", "ipv4Address": "10.0.0.2" }]
                }
            },
            {
                "configuration": { "networks": [{ "network": "cgui-t-net" }] },
                "status": { "state": "stopped", "networks": [] }
            },
            {
                "configuration": { "networks": [{ "network": "default" }] },
                "status": {
                    "state": "running",
                    "networks": [{ "network": "default" }]
                }
            }
        ]);
        let counts = network_ref_counts(serde_json::to_vec(&ls).unwrap().as_slice());
        // Only the one running container attached to cgui-t-net counts —
        // the stopped one's `configuration.networks` entry is not counted.
        assert_eq!(counts.get("cgui-t-net"), Some(&1));
        assert_eq!(counts.get("default"), Some(&1));
    }

    #[test]
    fn volume_ref_counts_counts_volume_mounts_only() {
        let ls = json!([
            { "configuration": { "mounts": [
                // type object with the volume name nested inside.
                { "destination": "/data", "source": "/path/volume.img",
                  "type": { "volume": { "name": "vol-a", "format": "ext4" } } },
                // non-volume mount types are skipped.
                { "destination": "/run", "source": "", "type": { "tmpfs": {} } }
            ] } },
            { "configuration": { "mounts": [
                { "destination": "/data", "source": "/other/volume.img",
                  "type": { "volume": { "name": "vol-a", "format": "ext4" } } },
                { "destination": "/host", "source": "/host", "type": { "virtiofs": {} } }
            ] } }
        ]);
        let counts = volume_ref_counts(serde_json::to_vec(&ls).unwrap().as_slice());
        assert_eq!(counts.get("vol-a"), Some(&2));
        assert_eq!(counts.len(), 1);
    }

    fn container_json(id: &str) -> Value {
        json!({
            "status": { "state": "running" },
            "configuration": {
                "id": id,
                "image": { "reference": "docker.io/library/alpine:latest" }
            }
        })
    }

    #[test]
    fn parse_container_keeps_full_id_for_uuid_and_stack_names() {
        for id in [
            "d5d88c0b-3037-4d11-bac4-4ed8d55c4461",
            "mystack_web",
            "cgui-live-ids-12345",
        ] {
            let c = parse_container(container_json(id));
            // The CLI matches ids exactly: id must be passed through intact.
            assert_eq!(c.id, id);
            assert_eq!(c.short_id, id.chars().take(8).collect::<String>());
            assert_eq!(c.short_id.len(), 8);
        }
        // Ids of 8 chars or fewer are unchanged in both fields.
        let c = parse_container(container_json("web"));
        assert_eq!((c.id.as_str(), c.short_id.as_str()), ("web", "web"));
    }

    const IMAGE_LS_ONE: &str = r#"[{"configuration":{"name":"docker.io/library/alpine:latest","descriptor":{"digest":"sha256:0123456789abcdef"}},"variants":[{"size":1073741824}]}]"#;

    #[test]
    fn image_ls_partial_output_returns_images_and_warning() {
        let stderr =
            "Warning: skipping image foo: bad manifest\nError: failed to read 2 image(s)\n";
        let l = parse_image_ls(false, IMAGE_LS_ONE.as_bytes(), stderr, &"exit status: 1").unwrap();
        assert_eq!(l.images.len(), 1);
        assert_eq!(l.images[0].reference, "docker.io/library/alpine:latest");
        let w = l.warning.unwrap();
        assert!(w.starts_with("2 images couldn't be read: "), "{w}");
        assert!(w.contains("bad manifest"));
    }

    #[test]
    fn image_ls_partial_singular_and_unknown_count() {
        let l = parse_image_ls(false, b"[]", "failed to read 1 image(s)", &"x").unwrap();
        assert!(l.images.is_empty());
        assert!(l.warning.unwrap().starts_with("1 image couldn't be read"));
        let l = parse_image_ls(false, b"[]", "", &"x").unwrap();
        assert_eq!(l.warning.as_deref(), Some("Some images couldn't be read"));
    }

    #[test]
    fn image_ls_nonzero_with_garbage_is_an_error() {
        let e = parse_image_ls(false, b"", SERVICES_DOWN, &"exit status: 1").unwrap_err();
        assert!(is_services_down_error(&e.to_string()));
        assert!(parse_image_ls(false, b"not json", "boom", &"x").is_err());
    }

    #[test]
    fn image_ls_success_has_no_warning() {
        let l = parse_image_ls(true, IMAGE_LS_ONE.as_bytes(), "", &"ok").unwrap();
        assert_eq!(l.images.len(), 1);
        assert!(l.warning.is_none());
        assert!(parse_image_ls(true, b"nope", "", &"ok").is_err());
    }
}
