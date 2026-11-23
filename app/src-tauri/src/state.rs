// Per-container CPU history + previous-sample tracking. Apple's `container
// stats` reports cumulative cpu time in microseconds, so CPU% is computed
// as a delta across two polls. History sustains the sparkline.

use anyhow::Result;
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Mutex;
use std::time::Instant;

use crate::model::{Container, MemUsage};
use crate::runtime;

const HISTORY_LEN: usize = 24;

#[derive(Default)]
struct Inner {
    cpu_history: HashMap<String, VecDeque<f64>>,
    last_sample: HashMap<String, LastSample>,
}

#[derive(Clone, Copy)]
struct LastSample {
    when: Instant,
    cpu_usec: u64,
    net_bytes: u64,  // rx + tx
    disk_bytes: u64, // r + w
}

pub struct History {
    inner: Mutex<Inner>,
}

impl Default for History {
    fn default() -> Self {
        Self::new()
    }
}

impl History {
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(Inner::default()),
        }
    }

    fn record(&self, g: &mut Inner, id: &str, sample: f64) {
        let q = g
            .cpu_history
            .entry(id.to_string())
            .or_insert_with(|| VecDeque::with_capacity(HISTORY_LEN));
        if q.len() == HISTORY_LEN {
            q.pop_front();
        }
        q.push_back(sample);
    }

    fn snapshot(g: &Inner, id: &str) -> Vec<f64> {
        g.cpu_history
            .get(id)
            .map(|q| q.iter().copied().collect())
            .unwrap_or_else(|| vec![0.0; HISTORY_LEN])
    }

    fn gc(g: &mut Inner, alive: &HashSet<String>) {
        g.cpu_history.retain(|k, _| alive.contains(k));
        g.last_sample.retain(|k, _| alive.contains(k));
    }
}

// Single poll: snapshot containers + stats, derive CPU% from cumulative-usec
// deltas vs the previous poll, accumulate history, and overlay onto each
// returned container.
pub async fn poll_once(history: &History) -> Result<Vec<Container>> {
    let (mut cs, stats) = runtime::list_containers_raw().await?;
    let now = Instant::now();
    let alive: HashSet<String> = cs.iter().map(|c| c.id.clone()).collect();

    let mut g = history.inner.lock().unwrap();
    for c in &mut cs {
        let Some(s) = runtime::match_stat(&stats, &c.id) else {
            // No stats row (likely not running). Still record a 0 sample so
            // the sparkline has a slot for this id.
            history.record(&mut g, &c.id, 0.0);
            c.cpu_history = History::snapshot(&g, &c.id);
            continue;
        };
        let net_bytes = s.network_rx_bytes + s.network_tx_bytes;
        let disk_bytes = s.block_read_bytes + s.block_write_bytes;
        let (cpu_pct, net_bps, disk_bps) = derive_rates(
            &mut g.last_sample,
            &c.id,
            now,
            s.cpu_usage_usec,
            net_bytes,
            disk_bytes,
        );
        let cpu_pct = pick_cpu_pct(s.cpu_utilization, cpu_pct);
        let used_gib = bytes_to_gib(s.memory_usage_bytes);
        let limit_gib = if s.memory_limit_bytes > 0 {
            bytes_to_gib(s.memory_limit_bytes)
        } else {
            c.mem.limit
        };
        let pct = if limit_gib > 0.0 {
            (used_gib / limit_gib) * 100.0
        } else {
            0.0
        };
        c.cpu = cpu_pct;
        c.mem = MemUsage {
            used: used_gib,
            limit: limit_gib,
            unit: "GiB".into(),
            pct,
        };
        c.net_io_bps = net_bps;
        c.disk_io_bps = disk_bps;
        history.record(&mut g, &c.id, cpu_pct);
        c.cpu_history = History::snapshot(&g, &c.id);
    }
    History::gc(&mut g, &alive);
    Ok(cs)
}

// Compute per-core CPU% + net/disk bytes/sec from the previous sample.
// Returns (0, 0, 0) on the first call for a given id (no baseline yet).
fn derive_rates(
    last: &mut HashMap<String, LastSample>,
    id: &str,
    now: Instant,
    cur_cpu_usec: u64,
    cur_net_bytes: u64,
    cur_disk_bytes: u64,
) -> (f64, f64, f64) {
    let new = LastSample {
        when: now,
        cpu_usec: cur_cpu_usec,
        net_bytes: cur_net_bytes,
        disk_bytes: cur_disk_bytes,
    };
    let prev = last.insert(id.to_string(), new);
    let Some(prev) = prev else {
        return (0.0, 0.0, 0.0);
    };
    let wall_secs = now.duration_since(prev.when).as_secs_f64();
    if wall_secs <= 0.0 {
        return (0.0, 0.0, 0.0);
    }
    let wall_usec = wall_secs * 1_000_000.0;
    let cpu_delta = cur_cpu_usec.saturating_sub(prev.cpu_usec) as f64;
    let net_delta = cur_net_bytes.saturating_sub(prev.net_bytes) as f64;
    let disk_delta = cur_disk_bytes.saturating_sub(prev.disk_bytes) as f64;
    (
        cpu_delta / wall_usec * 100.0,
        net_delta / wall_secs,
        disk_delta / wall_secs,
    )
}

fn bytes_to_gib(b: u64) -> f64 {
    (b as f64) / (1024.0 * 1024.0 * 1024.0)
}

/// CPU% for the UI. The UI shows percent of ONE core (a container using two
/// full cores reads 200%), which is exactly `cpuUtilization * 100`, so a
/// native value from the CLI (1.7.1+) is used when present and sane; it is
/// already averaged over the CLI's own ~2 s sample and needs no baseline poll.
/// Otherwise fall back to the delta of cumulative `cpuUsageUsec` between polls.
fn pick_cpu_pct(native_fraction: Option<f64>, delta_pct: f64) -> f64 {
    match native_fraction {
        Some(f) if f.is_finite() && f >= 0.0 => f * 100.0,
        _ => delta_pct,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_cpu_utilization_is_a_fraction_of_one_core() {
        assert_eq!(pick_cpu_pct(Some(1.0), 3.0), 100.0);
        assert!((pick_cpu_pct(Some(0.0123), 0.0) - 1.23).abs() < 1e-9);
        // More than one core busy reads above 100, like the delta method.
        assert!((pick_cpu_pct(Some(2.5), 0.0) - 250.0).abs() < 1e-9);
    }

    #[test]
    fn falls_back_to_the_delta_when_native_is_absent_or_bogus() {
        assert_eq!(pick_cpu_pct(None, 42.5), 42.5);
        assert_eq!(pick_cpu_pct(Some(f64::NAN), 7.0), 7.0);
        assert_eq!(pick_cpu_pct(Some(-1.0), 7.0), 7.0);
        assert_eq!(pick_cpu_pct(Some(f64::INFINITY), 7.0), 7.0);
    }

    #[test]
    fn delta_method_matches_cumulative_usec_over_wall_time() {
        let mut last = HashMap::new();
        let t0 = Instant::now();
        // First sample has no baseline.
        assert_eq!(derive_rates(&mut last, "a", t0, 1_000_000, 0, 0).0, 0.0);
        // 500 ms of CPU over 1 s of wall time = 50% of one core.
        let t1 = t0 + std::time::Duration::from_secs(1);
        let (cpu, _, _) = derive_rates(&mut last, "a", t1, 1_500_000, 0, 0);
        assert!((cpu - 50.0).abs() < 1e-6, "{cpu}");
    }

    #[test]
    fn stat_row_parses_with_and_without_cpu_utilization() {
        let new: Vec<crate::runtime::StatRow> = serde_json::from_str(
            r#"[{"id":"a","cpuUsageUsec":5087657,"cpuUtilization":1.0002196937116234,"memoryUsageBytes":1,"memoryLimitBytes":2,"numProcesses":1}]"#,
        )
        .unwrap();
        assert_eq!(new[0].cpu_utilization, Some(1.0002196937116234));
        let old: Vec<crate::runtime::StatRow> = serde_json::from_str(
            r#"[{"id":"a","cpuUsageUsec":5087657,"memoryUsageBytes":1,"memoryLimitBytes":2}]"#,
        )
        .unwrap();
        assert_eq!(old[0].cpu_utilization, None);
    }
}
