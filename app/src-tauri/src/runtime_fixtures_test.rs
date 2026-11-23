//! Offline tests that run the app's parsers over output captured from real
//! `container` CLIs (see `scripts/capture-fixtures.sh`). One directory per
//! CLI version under `tests/fixtures/`; every set must parse to sensible,
//! non-default values. Child module of `runtime` so it can reach the
//! private `parse_*` helpers.

use super::*;
use serde_json::Value;

struct Set {
    version: &'static str,
    ls: &'static str,
    stats: &'static str,
    images: &'static str,
    volumes: &'static str,
    networks: &'static str,
    df: &'static str,
    builder: &'static str,
    machines: &'static str,
    registries: &'static str,
    status: &'static str,
    cli_version: &'static str,
}

macro_rules! fx {
    ($v:literal, $f:literal) => {
        include_str!(concat!("../tests/fixtures/", $v, "/", $f))
    };
}

macro_rules! set {
    ($v:literal) => {
        Set {
            version: $v,
            ls: fx!($v, "ls.json"),
            stats: fx!($v, "stats.json"),
            images: fx!($v, "image-ls.json"),
            volumes: fx!($v, "volume-ls.json"),
            networks: fx!($v, "network-ls.json"),
            df: fx!($v, "system-df.json"),
            builder: fx!($v, "builder-status.json"),
            machines: fx!($v, "machine-list.json"),
            registries: fx!($v, "registry-list.json"),
            status: fx!($v, "system-status.json"),
            cli_version: fx!($v, "version.txt"),
        }
    };
}

fn sets() -> Vec<Set> {
    vec![set!("1.4.1"), set!("1.7.1")]
}

fn arr(s: &str) -> Vec<Value> {
    serde_json::from_str(s).expect("fixture is a JSON array")
}

/// The container our capture script started (`cgui-live-fx-*-ctr`).
fn fixture_container(set: &Set) -> Container {
    let raw = arr(set.ls);
    raw.into_iter()
        .map(parse_container)
        .find(|c| c.id.starts_with("cgui-live-fx-") && c.id.ends_with("-ctr"))
        .unwrap_or_else(|| panic!("{}: fixture container missing", set.version))
}

#[test]
fn fixtures_report_their_cli_version() {
    for s in sets() {
        assert!(
            s.cli_version.contains(s.version),
            "{}: version.txt says {}",
            s.version,
            s.cli_version
        );
        let v: Value = serde_json::from_str(s.status).unwrap();
        assert_eq!(v["client"]["version"], s.version);
        assert_eq!(v["server"]["version"], s.version);
    }
}

#[test]
fn fixtures_system_status_is_running() {
    for s in sets() {
        assert!(parse_system_running(s.status), "{}", s.version);
    }
}

#[test]
fn fixtures_container_list_parses() {
    for s in sets() {
        let c = fixture_container(&s);
        assert_eq!(c.status, "running", "{}", s.version);
        assert_eq!(c.image, "docker.io/library/alpine:latest", "{}", s.version);
        assert_eq!(c.name, c.id, "{}: hostname from network options", s.version);
        assert_eq!(c.short_id, "cgui-liv", "{}", s.version);
        assert_eq!(c.ports, vec!["18080:80"], "{}", s.version);
        assert_eq!(c.cmd, vec!["sleep", "300"], "{}", s.version);
        assert!(c.created.starts_with("20"), "{}: {}", s.version, c.created);
        assert_ne!(c.uptime, "—", "{}: running uptime", s.version);
        assert!(
            c.started_unix.is_some_and(|t| t > 1_700_000_000),
            "{}",
            s.version
        );
        assert!((c.mem.limit - 1.0).abs() < 1e-6, "{}", s.version);
    }
}

#[test]
fn fixtures_stopped_containers_have_no_uptime() {
    // 1.4.1 was captured on a machine with the builder + another stopped
    // container; 1.7.1 has only the running one.
    let s = set!("1.4.1");
    let stopped: Vec<Container> = arr(s.ls)
        .into_iter()
        .map(parse_container)
        .filter(|c| c.id == "buildkit")
        .collect();
    assert_eq!(stopped.len(), 1);
    assert_eq!(stopped[0].status, "exited");
    assert_eq!(stopped[0].uptime, "—");
    assert_eq!(stopped[0].started_unix, None);
    assert_eq!(stopped[0].name, "buildkit");
}

#[test]
fn fixtures_stats_parse() {
    for s in sets() {
        let rows: Vec<StatRow> = arr(s.stats)
            .into_iter()
            .filter_map(|v| serde_json::from_value(v).ok())
            .collect();
        assert_eq!(rows.len(), 1, "{}", s.version);
        let c = fixture_container(&s);
        let r = match_stat(&rows, &c.id).unwrap_or_else(|| panic!("{}: no stat row", s.version));
        assert!(r.cpu_usage_usec > 0, "{}", s.version);
        assert!(r.memory_usage_bytes > 0, "{}", s.version);
        assert_eq!(r.memory_limit_bytes, 1 << 30, "{}", s.version);
        assert!(r.network_rx_bytes > 0, "{}", s.version);
        assert!(r.block_read_bytes > 0, "{}", s.version);
    }
}

#[test]
fn fixtures_images_parse() {
    for s in sets() {
        let imgs: Vec<Image> = arr(s.images).into_iter().map(parse_image).collect();
        let alpine = imgs
            .iter()
            .find(|i| i.reference == "docker.io/library/alpine:latest")
            .unwrap_or_else(|| panic!("{}: alpine missing", s.version));
        assert!(alpine.id.starts_with("sha256:"), "{}", s.version);
        assert_eq!(alpine.id.len(), "sha256:".len() + 8, "{}", s.version);
        assert!(alpine.digest.starts_with("sha256:"), "{}", s.version);
        assert!(alpine.created.starts_with("20"), "{}", s.version);
        assert!(alpine.size > 0.0, "{}", s.version);
        assert!(imgs.iter().all(|i| i.reference != "?"), "{}", s.version);
    }
}

#[test]
fn fixtures_volumes_parse_with_ref_counts() {
    for s in sets() {
        let refs = volume_ref_counts(s.ls.as_bytes());
        let vols: Vec<Volume> = arr(s.volumes)
            .into_iter()
            .map(|v| parse_volume(v, &refs))
            .collect();
        assert_eq!(vols.len(), 1, "{}", s.version);
        let v = &vols[0];
        assert!(v.name.starts_with("cgui-live-fx-"), "{}", s.version);
        assert_eq!(v.driver, "local", "{}", s.version);
        assert!(v.mountpoint.ends_with("volume.img"), "{}", s.version);
        assert!((v.size - 512.0).abs() < 1e-6, "{}", s.version);
        assert_eq!(v.refs, 1, "{}: mounted by the running container", s.version);
    }
}

#[test]
fn fixtures_networks_parse_with_attachment_counts() {
    for s in sets() {
        let refs = network_ref_counts(s.ls.as_bytes());
        let nets: Vec<Network> = arr(s.networks)
            .into_iter()
            .map(|n| parse_network(n, &refs))
            .collect();
        let custom = nets
            .iter()
            .find(|n| n.name.starts_with("cgui-live-fx-"))
            .unwrap_or_else(|| panic!("{}: custom network missing", s.version));
        assert_eq!(custom.mode, "nat", "{}", s.version);
        assert_eq!(custom.state, "active", "{}", s.version);
        assert_eq!(custom.subnet, "192.168.65.0/24", "{}", s.version);
        assert_eq!(custom.gateway, "192.168.65.1", "{}", s.version);
        assert_eq!(custom.containers, 1, "{}", s.version);
        let def = nets.iter().find(|n| n.name == "default").unwrap();
        assert_eq!(def.subnet, "192.168.64.0/24", "{}", s.version);
        assert_eq!(def.containers, 0, "{}", s.version);
    }
}

#[test]
fn fixtures_system_df_parses() {
    for s in sets() {
        let df: DiskUsage = serde_json::from_str(s.df).unwrap();
        assert!(
            df.images.total > 0 && df.images.size_in_bytes > 0,
            "{}",
            s.version
        );
        assert_eq!(df.containers.active, 1, "{}", s.version);
        assert!(df.containers.size_in_bytes > 0, "{}", s.version);
        assert_eq!(df.volumes.total, 1, "{}", s.version);
        assert!(df.volumes.size_in_bytes > 0, "{}", s.version);
    }
}

#[test]
fn fixtures_builder_status_parses() {
    // 1.4.1: a stopped builder container; 1.7.1: no builder instance (`[]`).
    let b141 = arr(fx!("1.4.1", "builder-status.json"));
    let c = parse_container(b141.into_iter().next().unwrap());
    assert_eq!(c.id, "buildkit");
    assert_eq!(c.status, "exited");
    assert!(c.image.contains("container-builder-shim"));
    assert!((c.mem.limit - 2.0).abs() < 1e-6);
    assert!(arr(fx!("1.7.1", "builder-status.json")).is_empty());
}

#[test]
fn fixtures_machines_and_registries_parse() {
    for s in sets() {
        let ms: Vec<Machine> = serde_json::from_str(s.machines).unwrap();
        assert!(ms.is_empty(), "{}", s.version);
        assert!(arr(s.registries).is_empty(), "{}", s.version);
    }
}

#[test]
fn fixtures_contain_no_identifying_paths() {
    for s in sets() {
        for body in [s.ls, s.volumes, s.status, s.builder, s.networks] {
            for (i, _) in body.match_indices("/Users/") {
                assert!(
                    body[i..].starts_with("/Users/user/"),
                    "{}: unredacted home path near {}",
                    s.version,
                    &body[i..(i + 30).min(body.len())]
                );
            }
        }
    }
}
