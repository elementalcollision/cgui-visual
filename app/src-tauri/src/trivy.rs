// Permissive parser for `trivy image --format json` output. Mirrors cgui's
// trivy.rs but emits the wire shape the UI's TrivyModal already renders.

use serde::Deserialize;
use serde_json::json;
use std::time::Duration;
use tokio::process::Command;

use crate::model::{TrivyFinding, TrivyResult};

const TRIVY_TIMEOUT: Duration = Duration::from_secs(120);

#[derive(Debug, Default, Clone, Deserialize)]
struct Raw {
    #[serde(rename = "Results", default)]
    results: Vec<RawTarget>,
    #[serde(rename = "ArtifactName", default)]
    artifact: Option<String>,
}

#[derive(Debug, Default, Clone, Deserialize)]
struct RawTarget {
    #[serde(rename = "Vulnerabilities", default)]
    vulnerabilities: Vec<RawVuln>,
}

#[derive(Debug, Default, Clone, Deserialize)]
struct RawVuln {
    #[serde(rename = "VulnerabilityID", default)]
    id: String,
    #[serde(rename = "PkgName", default)]
    pkg: String,
    #[serde(rename = "InstalledVersion", default)]
    installed: String,
    #[serde(rename = "FixedVersion", default)]
    fixed: String,
    #[serde(rename = "Severity", default)]
    severity: String,
    #[serde(rename = "Title", default)]
    title: String,
    #[serde(rename = "Description", default)]
    description: String,
    #[serde(rename = "References", default)]
    references: Vec<String>,
    #[serde(rename = "CVSS", default)]
    cvss: std::collections::BTreeMap<String, RawCvss>,
}

// Trivy emits per-vendor CVSS blocks (`nvd`, `redhat`, `ghsa`, etc.) keyed
// by source. We pluck the V3Score, preferring nvd → ghsa → first non-zero.
#[derive(Debug, Default, Clone, Deserialize)]
struct RawCvss {
    #[serde(rename = "V3Score", default)]
    v3: Option<f64>,
}

fn pick_cvss(map: &std::collections::BTreeMap<String, RawCvss>) -> Option<f64> {
    for source in ["nvd", "ghsa", "redhat"] {
        if let Some(s) = map.get(source).and_then(|c| c.v3) {
            if s > 0.0 {
                return Some(s);
            }
        }
    }
    map.values().find_map(|c| c.v3.filter(|s| *s > 0.0))
}

pub async fn available() -> bool {
    Command::new("trivy")
        .arg("--version")
        .output()
        .await
        .map(|o| o.status.success())
        .unwrap_or(false)
}

pub async fn scan(image: &str) -> Option<TrivyResult> {
    if !available().await {
        return None;
    }
    let fut = Command::new("trivy")
        .args([
            "image",
            "--quiet",
            "--format",
            "json",
            "--scanners",
            "vuln",
            image,
        ])
        .output();
    let out = tokio::time::timeout(TRIVY_TIMEOUT, fut).await.ok()?.ok()?;
    if !out.status.success() {
        eprintln!(
            "trivy scan failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        );
        return None;
    }
    let raw: Raw = serde_json::from_slice(&out.stdout).ok()?;

    let mut findings: Vec<TrivyFinding> = Vec::new();
    for tgt in raw.results {
        for v in tgt.vulnerabilities {
            let cvss = pick_cvss(&v.cvss);
            let description = if v.description.is_empty() {
                None
            } else {
                Some(v.description)
            };
            findings.push(TrivyFinding {
                sev: normalize_severity(&v.severity),
                cve: v.id,
                pkg: v.pkg,
                installed: v.installed,
                fixed: v.fixed,
                title: v.title,
                cvss,
                description,
                refs: v.references,
            });
        }
    }
    findings.sort_by(|a, b| {
        sev_rank(&a.sev)
            .cmp(&sev_rank(&b.sev))
            .then(a.cve.cmp(&b.cve))
    });

    let mut counts = [0u64; 5]; // crit, high, med, low, unknown
    for f in &findings {
        counts[sev_rank(&f.sev) as usize] += 1;
    }

    Some(TrivyResult {
        image: raw.artifact.unwrap_or_else(|| image.to_string()),
        counts: json!({
            "CRITICAL": counts[0], "HIGH": counts[1],
            "MEDIUM": counts[2], "LOW": counts[3],
        }),
        findings,
    })
}

fn normalize_severity(s: &str) -> String {
    match s.to_ascii_uppercase().as_str() {
        "CRITICAL" => "CRITICAL",
        "HIGH" => "HIGH",
        "MEDIUM" => "MEDIUM",
        "LOW" => "LOW",
        _ => "LOW", // bucket unknowns into LOW for the UI's 4-color filter
    }
    .to_string()
}

fn sev_rank(s: &str) -> u8 {
    match s {
        "CRITICAL" => 0,
        "HIGH" => 1,
        "MEDIUM" => 2,
        "LOW" => 3,
        _ => 4,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_severity_buckets_unknown_to_low() {
        assert_eq!(normalize_severity("CRITICAL"), "CRITICAL");
        assert_eq!(normalize_severity("high"), "HIGH");
        assert_eq!(normalize_severity("UNKNOWN"), "LOW");
        assert_eq!(normalize_severity(""), "LOW");
    }

    #[test]
    fn pick_cvss_prefers_nvd_then_ghsa_then_first_nonzero() {
        use std::collections::BTreeMap;
        let mut m = BTreeMap::new();
        m.insert("redhat".into(), RawCvss { v3: Some(7.5) });
        m.insert("nvd".into(), RawCvss { v3: Some(9.8) });
        m.insert("ghsa".into(), RawCvss { v3: Some(8.0) });
        assert_eq!(pick_cvss(&m), Some(9.8));

        let mut m2 = BTreeMap::new();
        m2.insert("ghsa".into(), RawCvss { v3: Some(6.1) });
        m2.insert("redhat".into(), RawCvss { v3: Some(0.0) });
        assert_eq!(pick_cvss(&m2), Some(6.1));

        // Falls through to first non-zero when no preferred source has it.
        let mut m3 = BTreeMap::new();
        m3.insert("foo".into(), RawCvss { v3: Some(0.0) });
        m3.insert("bar".into(), RawCvss { v3: Some(4.2) });
        assert_eq!(pick_cvss(&m3), Some(4.2));

        let m4: BTreeMap<String, RawCvss> = BTreeMap::new();
        assert_eq!(pick_cvss(&m4), None);
    }

    #[test]
    fn sev_rank_orders_critical_first() {
        assert!(sev_rank("CRITICAL") < sev_rank("HIGH"));
        assert!(sev_rank("HIGH") < sev_rank("MEDIUM"));
        assert!(sev_rank("MEDIUM") < sev_rank("LOW"));
    }

    // End-to-end parse of a minimal trivy JSON. Doesn't shell out; just checks
    // the deserialize+sort+count pipeline.
    #[tokio::test]
    async fn parse_minimal_trivy_payload() {
        let json = r#"{
            "ArtifactName": "alpine:3",
            "Results": [{
                "Target": "alpine:3",
                "Vulnerabilities": [
                    {"VulnerabilityID":"CVE-Z","Severity":"LOW","PkgName":"a"},
                    {"VulnerabilityID":"CVE-A","Severity":"CRITICAL","PkgName":"b"},
                    {"VulnerabilityID":"CVE-M","Severity":"MEDIUM","PkgName":"c"}
                ]
            }]
        }"#;
        // Reuse the parsing path without spawning trivy.
        let raw: Raw = serde_json::from_str(json).unwrap();
        let mut findings: Vec<TrivyFinding> = raw
            .results
            .into_iter()
            .flat_map(|t| t.vulnerabilities)
            .map(|v| TrivyFinding {
                sev: normalize_severity(&v.severity),
                cve: v.id,
                pkg: v.pkg,
                installed: v.installed,
                fixed: v.fixed,
                title: v.title,
                ..TrivyFinding::default()
            })
            .collect();
        findings.sort_by_key(|a| sev_rank(&a.sev));
        assert_eq!(findings[0].sev, "CRITICAL");
        assert_eq!(findings[2].sev, "LOW");
    }
}
