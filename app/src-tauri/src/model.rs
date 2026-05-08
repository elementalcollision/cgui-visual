// Domain types — kept in lockstep with src/types.ts.
// All field names use camelCase via serde rename so they round-trip with the
// TS layer without conversion.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemUsage {
    pub used: f64,
    pub limit: f64,
    pub unit: String,
    pub pct: f64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Container {
    pub id: String,
    pub name: String,
    pub image: String,
    pub status: String,
    pub uptime: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exit_code: Option<i32>,
    pub cpu: f64,
    pub mem: MemUsage,
    pub ports: Vec<String>,
    pub stack: Option<String>,
    pub created: String,
    pub cpu_history: Vec<f64>,
    pub cmd: Vec<String>,
    // Bytes/sec, derived from cumulative-byte deltas across polls. 0.0 when
    // we don't yet have a previous sample.
    pub net_io_bps: f64,
    pub disk_io_bps: f64,
    // Unix-epoch seconds of when the container was started. Populated when
    // Apple's CLI emits `startedDate` (NSDate epoch). Used by the stack
    // healthcheck poller to compute elapsed-since-start for the
    // `start_period_s` grace window.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub started_unix: Option<i64>,
}

impl Default for MemUsage {
    fn default() -> Self {
        Self {
            used: 0.0,
            limit: 0.0,
            unit: "GiB".into(),
            pct: 0.0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Image {
    pub id: String,
    #[serde(rename = "ref")]
    pub reference: String,
    pub size: f64,
    pub size_unit: String,
    pub created: String,
    pub tags: Vec<String>,
    pub digest: String,
    pub layers: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Volume {
    pub name: String,
    pub driver: String,
    pub mountpoint: String,
    pub size: f64,
    pub used: f64,
    pub unit: String,
    pub refs: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Network {
    pub id: String,
    pub name: String,
    pub mode: String,
    pub state: String,
    pub subnet: String,
    pub gateway: String,
    pub dns: Vec<String>,
    pub containers: u32,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Service {
    pub name: String,
    pub image: String,
    pub state: String,
    pub health: String,
    /// Names of services this service depends on, mirrored from the
    /// stack TOML. Used by the StackGraph to draw dependency edges.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub depends_on: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Stack {
    pub name: String,
    pub services: Vec<Service>,
    pub restart: String,
    pub health: String,
    pub file: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct TrivyFinding {
    pub sev: String,
    pub cve: String,
    pub pkg: String,
    pub installed: String,
    pub fixed: String,
    pub title: String,
    /// CVSS v3 base score (0.0–10.0) when trivy reports one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cvss: Option<f64>,
    /// Free-form description from trivy. Surfaced only by the detail drawer.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Reference URLs (advisories, vendor pages, NVD) for the drawer.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub refs: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrivyResult {
    pub image: String,
    pub counts: serde_json::Value,
    pub findings: Vec<TrivyFinding>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Update {
    pub component: String,
    pub installed: String,
    pub latest: String,
    pub published: String,
    pub notes: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DoctorCheck {
    pub ok: bool,
    pub text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub warn: Option<bool>,
    /// Optional one-click remediation. When set, the DoctorModal renders
    /// a button next to the check that performs the action. Matches the
    /// frontend's DoctorFix shape (kind = 'url' opens a link in the
    /// default browser; kind = 'copy' puts the command on the clipboard).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fix: Option<DoctorFix>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum DoctorFix {
    /// Open this URL in the user's default browser.
    Url { label: String, url: String },
    /// Copy this shell command to the clipboard so the user can paste it
    /// into Terminal. We never run shell commands directly from doctor —
    /// `brew install …` etc. should be opt-in to keep the surface safe.
    Copy { label: String, command: String },
}
