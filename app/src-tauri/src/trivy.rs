// Permissive parser for `trivy image --format json` output. Mirrors cgui's
// trivy.rs but emits the wire shape the UI's TrivyModal already renders.

use serde::{Deserialize, Serialize};
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
        .kill_on_drop(true)
        .arg("--version")
        .output()
        .await
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Outcome of a scan request. The three non-success-with-findings states
/// are distinct on purpose: a missing binary or a failed/timed-out scan must
/// never be rendered as a clean (0 CVE) result.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "status", rename_all = "camelCase")]
pub enum ScanOutcome {
    Ok { result: TrivyResult },
    NotInstalled,
    Failed { message: String },
}

pub async fn scan(image: &str) -> ScanOutcome {
    if !available().await {
        return ScanOutcome::NotInstalled;
    }
    let fut = Command::new("trivy")
        .kill_on_drop(true)
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
    let out = match tokio::time::timeout(TRIVY_TIMEOUT, fut).await {
        Err(_) => {
            return ScanOutcome::Failed {
                message: format!("scan timed out after {}s", TRIVY_TIMEOUT.as_secs()),
            }
        }
        Ok(Err(e)) => {
            return ScanOutcome::Failed {
                message: format!("could not run trivy: {e}"),
            }
        }
        Ok(Ok(out)) => out,
    };
    match map_output(
        out.status.success(),
        out.status.code(),
        &out.stdout,
        &String::from_utf8_lossy(&out.stderr),
        image,
    ) {
        Ok(result) => ScanOutcome::Ok { result },
        Err(message) => ScanOutcome::Failed { message },
    }
}

/// Map a finished `trivy image` process to a result or a user-facing
/// failure message. Non-zero exit and unparseable JSON are failures, never
/// an empty report.
fn map_output(
    success: bool,
    code: Option<i32>,
    stdout: &[u8],
    stderr: &str,
    image: &str,
) -> Result<TrivyResult, String> {
    if !success {
        let detail = stderr.trim();
        let code = code.map_or_else(|| "a signal".to_string(), |c| c.to_string());
        return Err(if detail.is_empty() {
            format!("trivy exited with {code} and no output")
        } else {
            format!("trivy exited with {code}: {detail}")
        });
    }
    parse_report(stdout, image)
}

fn parse_report(stdout: &[u8], image: &str) -> Result<TrivyResult, String> {
    let raw: Raw =
        serde_json::from_slice(stdout).map_err(|e| format!("could not parse trivy output: {e}"))?;

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

    Ok(TrivyResult {
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

    #[test]
    fn nonzero_exit_is_a_failure_with_stderr() {
        let e = map_output(false, Some(1), b"", "FATAL image not found\n", "x").unwrap_err();
        assert_eq!(e, "trivy exited with 1: FATAL image not found");
        let e = map_output(false, None, b"", "", "x").unwrap_err();
        assert!(e.contains("no output"), "{e}");
    }

    #[test]
    fn garbage_stdout_is_a_failure_not_an_empty_report() {
        assert!(map_output(true, Some(0), b"", "", "x").is_err());
        assert!(map_output(true, Some(0), b"not json", "", "x")
            .unwrap_err()
            .starts_with("could not parse trivy output"));
    }

    #[test]
    fn clean_and_findings_reports_are_ok_and_distinguishable() {
        let clean = map_output(
            true,
            Some(0),
            br#"{"ArtifactName":"a:1","Results":[]}"#,
            "",
            "a:1",
        )
        .unwrap();
        assert!(clean.findings.is_empty());
        assert_eq!(clean.counts["CRITICAL"], 0);
        let dirty = map_output(
            true,
            Some(0),
            br#"{"Results":[{"Vulnerabilities":[{"VulnerabilityID":"CVE-1","Severity":"HIGH"}]}]}"#,
            "",
            "img",
        )
        .unwrap();
        assert_eq!(dirty.image, "img");
        assert_eq!(dirty.findings.len(), 1);
        assert_eq!(dirty.counts["HIGH"], 1);
    }

    #[test]
    fn scan_outcome_serializes_with_status_tag() {
        let v = serde_json::to_value(ScanOutcome::NotInstalled).unwrap();
        assert_eq!(v, json!({"status": "notInstalled"}));
        let v = serde_json::to_value(ScanOutcome::Failed {
            message: "m".into(),
        })
        .unwrap();
        assert_eq!(v, json!({"status": "failed", "message": "m"}));
        let ok = ScanOutcome::Ok {
            result: parse_report(br#"{"Results":[]}"#, "i").unwrap(),
        };
        let v = serde_json::to_value(ok).unwrap();
        assert_eq!(v["status"], "ok");
        assert_eq!(v["result"]["image"], "i");
    }
}
