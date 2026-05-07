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
