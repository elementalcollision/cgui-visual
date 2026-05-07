// Fixture data — same shape as src/fixtures.ts. Used as a fallback when the
// runtime CLI isn't available (dev machines without Apple's `container`).
// Hand-keyed from the original Workbench design prototype.

#![allow(dead_code)]

use crate::model::*;
use serde_json::json;

pub fn containers() -> Vec<Container> {
    vec![
        Container {
            id: "a3f8e2c1".into(),
            name: "mlperf-inference-llama2".into(),
            image: "mlcommons/inference:llama2-70b".into(),
            status: "running".into(),
            uptime: "2d 14h".into(),
            exit_code: None,
            cpu: 78.2,
            mem: MemUsage {
                used: 38.4,
                limit: 64.0,
                unit: "GiB".into(),
                pct: 60.0,
            },
            ports: vec!["8080:8080".into(), "9090:9090".into()],
            stack: Some("mlperf-inference".into()),
            created: "2026-04-30 09:14:22".into(),
            cpu_history: vec![
                12.0, 18.0, 24.0, 35.0, 48.0, 62.0, 71.0, 78.0, 82.0, 79.0, 76.0, 78.0, 80.0, 78.0,
                75.0, 78.0, 81.0, 78.0, 76.0, 78.0, 77.0, 78.0, 79.0, 78.0,
            ],
            cmd: vec![
                "python".into(),
                "inference_server.py".into(),
                "--model".into(),
                "llama2-70b".into(),
            ],
            net_io_bps: 0.0,
            disk_io_bps: 0.0,
            started_unix: None,
        },
        Container {
            id: "b7d4a9e2".into(),
            name: "pgvector-embeddings".into(),
            image: "pgvector/pgvector:pg16".into(),
            status: "running".into(),
            uptime: "5d 02h".into(),
            exit_code: None,
            cpu: 12.4,
            mem: MemUsage {
                used: 2.1,
                limit: 8.0,
                unit: "GiB".into(),
                pct: 26.3,
            },
            ports: vec!["15432:5432".into()],
            stack: Some("mlperf-inference".into()),
            created: "2026-04-27 21:08:11".into(),
            cpu_history: vec![
                8.0, 10.0, 12.0, 14.0, 11.0, 13.0, 15.0, 12.0, 10.0, 11.0, 13.0, 12.0, 14.0, 12.0,
                11.0, 12.0, 13.0, 12.0, 11.0, 12.0, 13.0, 12.0, 12.0, 12.0,
            ],
            cmd: vec!["postgres".into()],
            net_io_bps: 0.0,
            disk_io_bps: 0.0,
            started_unix: None,
        },
        Container {
            id: "c2e9b3f4".into(),
            name: "criteo-trainer".into(),
            image: "mlcommons/training:dlrm-criteo".into(),
            status: "running".into(),
            uptime: "14h 22m".into(),
            exit_code: None,
            cpu: 94.8,
            mem: MemUsage {
                used: 56.2,
                limit: 64.0,
                unit: "GiB".into(),
                pct: 87.8,
            },
            ports: vec![],
            stack: Some("dlrm-training".into()),
            created: "2026-05-01 23:42:09".into(),
            cpu_history: vec![
                85.0, 88.0, 92.0, 94.0, 96.0, 95.0, 94.0, 93.0, 95.0, 94.0, 96.0, 97.0, 95.0, 94.0,
                93.0, 94.0, 95.0, 94.0, 93.0, 94.0, 95.0, 94.0, 95.0, 95.0,
            ],
            cmd: vec![
                "python".into(),
                "train.py".into(),
                "--config".into(),
                "dlrm.yaml".into(),
            ],
            net_io_bps: 0.0,
            disk_io_bps: 0.0,
            started_unix: None,
        },
        Container {
            id: "d8a1c5b9".into(),
            name: "ailuminate-grader".into(),
            image: "mlcommons/safety:ailuminate-1.2".into(),
            status: "running".into(),
            uptime: "1h 03m".into(),
            exit_code: None,
            cpu: 24.1,
            mem: MemUsage {
                used: 4.8,
                limit: 16.0,
                unit: "GiB".into(),
                pct: 30.0,
            },
            ports: vec!["7860:7860".into()],
            stack: None,
            created: "2026-05-02 13:08:55".into(),
            cpu_history: vec![
                18.0, 22.0, 26.0, 28.0, 24.0, 22.0, 24.0, 26.0, 24.0, 22.0, 24.0, 25.0, 24.0, 23.0,
                24.0, 25.0, 24.0, 23.0, 24.0, 25.0, 24.0, 24.0, 24.0, 24.0,
            ],
            cmd: vec!["python".into(), "-m".into(), "ailuminate.grader".into()],
            net_io_bps: 0.0,
            disk_io_bps: 0.0,
            started_unix: None,
        },
        Container {
            id: "e4f7d2a8".into(),
            name: "redis-eval-cache".into(),
            image: "redis:7.2-alpine".into(),
            status: "running".into(),
            uptime: "5d 02h".into(),
            exit_code: None,
            cpu: 1.8,
            mem: MemUsage {
                used: 0.18,
                limit: 1.0,
                unit: "GiB".into(),
                pct: 18.0,
            },
            ports: vec!["16379:6379".into()],
            stack: Some("mlperf-inference".into()),
            created: "2026-04-27 21:08:11".into(),
            cpu_history: vec![
                1.0, 2.0, 1.0, 2.0, 3.0, 1.0, 2.0, 2.0, 1.0, 2.0, 1.0, 2.0, 2.0, 1.0, 2.0, 1.0,
                2.0, 2.0, 1.0, 2.0, 1.0, 2.0, 2.0, 2.0,
            ],
            cmd: vec!["redis-server".into()],
            net_io_bps: 0.0,
            disk_io_bps: 0.0,
            started_unix: None,
        },
        Container {
            id: "f1c8e6b3".into(),
            name: "storage-bench-fio".into(),
            image: "mlcommons/storage:fio-3.36".into(),
            status: "exited".into(),
            uptime: "—".into(),
            exit_code: Some(0),
            cpu: 0.0,
            mem: MemUsage {
                used: 0.0,
                limit: 4.0,
                unit: "GiB".into(),
                pct: 0.0,
            },
            ports: vec![],
            stack: None,
            created: "2026-05-02 08:11:33".into(),
            cpu_history: vec![0.0; 24],
            cmd: vec!["fio".into(), "/etc/jobs/seq-read.fio".into()],
            net_io_bps: 0.0,
            disk_io_bps: 0.0,
            started_unix: None,
        },
        Container {
            id: "0a4b9d7e".into(),
            name: "jupyter-research".into(),
            image: "jupyter/scipy-notebook:python-3.11".into(),
            status: "running".into(),
            uptime: "8d 17h".into(),
            exit_code: None,
            cpu: 6.2,
            mem: MemUsage {
                used: 1.4,
                limit: 8.0,
                unit: "GiB".into(),
                pct: 17.5,
            },
            ports: vec!["8888:8888".into()],
            stack: None,
            created: "2026-04-24 06:51:42".into(),
            cpu_history: vec![
                5.0, 6.0, 7.0, 8.0, 6.0, 5.0, 6.0, 7.0, 6.0, 5.0, 6.0, 7.0, 8.0, 6.0, 5.0, 6.0,
                7.0, 6.0, 5.0, 6.0, 7.0, 6.0, 6.0, 6.0,
            ],
            cmd: vec!["start-notebook.sh".into()],
            net_io_bps: 0.0,
            disk_io_bps: 0.0,
            started_unix: None,
        },
        Container {
            id: "6b3c8f2a".into(),
            name: "imagenet-loader".into(),
            image: "mlcommons/training:resnet50-1.5".into(),
            status: "paused".into(),
            uptime: "3h 18m".into(),
            exit_code: None,
            cpu: 0.0,
            mem: MemUsage {
                used: 8.2,
                limit: 16.0,
                unit: "GiB".into(),
                pct: 51.3,
            },
            ports: vec![],
            stack: Some("resnet-training".into()),
            created: "2026-05-02 11:24:18".into(),
            cpu_history: vec![
                44.0, 52.0, 61.0, 58.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0,
                0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0,
            ],
            cmd: vec!["python".into(), "train_resnet.py".into()],
            net_io_bps: 0.0,
            disk_io_bps: 0.0,
            started_unix: None,
        },
    ]
}

pub fn images() -> Vec<Image> {
    let mk =
        |id: &str, r: &str, size: f64, created: &str, tags: &[&str], digest: &str, layers: u32| {
            Image {
                id: id.into(),
                reference: r.into(),
                size,
                size_unit: "GiB".into(),
                created: created.into(),
                tags: tags.iter().map(|s| s.to_string()).collect(),
                digest: digest.into(),
                layers,
            }
        };
    vec![
        mk(
            "sha256:8f3e2a91",
            "mlcommons/inference:llama2-70b",
            142.3,
            "2026-04-29",
            &["llama2-70b"],
            "sha256:8f3e2a91...c2d4",
            24,
        ),
        mk(
            "sha256:b4c7d9f2",
            "mlcommons/training:dlrm-criteo",
            8.7,
            "2026-04-22",
            &["dlrm-criteo", "latest"],
            "sha256:b4c7d9f2...8a1e",
            18,
        ),
        mk(
            "sha256:e1a8b3c6",
            "mlcommons/training:resnet50-1.5",
            4.2,
            "2026-03-14",
            &["resnet50-1.5"],
            "sha256:e1a8b3c6...f9d2",
            16,
        ),
        mk(
            "sha256:7d2f4a8b",
            "mlcommons/safety:ailuminate-1.2",
            6.1,
            "2026-04-18",
            &["ailuminate-1.2", "latest"],
            "sha256:7d2f4a8b...3c5e",
            21,
        ),
        mk(
            "sha256:c5b1e7f3",
            "mlcommons/storage:fio-3.36",
            0.4,
            "2026-02-08",
            &["fio-3.36"],
            "sha256:c5b1e7f3...a7d8",
            8,
        ),
        mk(
            "sha256:a9f3b8c1",
            "pgvector/pgvector:pg16",
            0.6,
            "2026-03-30",
            &["pg16"],
            "sha256:a9f3b8c1...b2e4",
            14,
        ),
        mk(
            "sha256:4e7c2d9a",
            "redis:7.2-alpine",
            0.04,
            "2026-04-12",
            &["7.2-alpine"],
            "sha256:4e7c2d9a...8f1c",
            7,
        ),
        mk(
            "sha256:9b6e4f8d",
            "jupyter/scipy-notebook:python-3.11",
            4.8,
            "2026-03-02",
            &["python-3.11"],
            "sha256:9b6e4f8d...d3a7",
            22,
        ),
        mk(
            "sha256:2c8a5b7e",
            "docker.io/library/alpine:3.19",
            0.008,
            "2026-01-22",
            &["3.19", "latest"],
            "sha256:2c8a5b7e...e6c1",
            1,
        ),
    ]
}

pub fn volumes() -> Vec<Volume> {
    let mk = |name: &str, mp: &str, size: f64, used: f64, refs: u32| Volume {
        name: name.into(),
        driver: "local".into(),
        mountpoint: mp.into(),
        size,
        used,
        unit: "GiB".into(),
        refs,
    };
    vec![
        mk(
            "mlperf-imagenet-2012",
            "/var/lib/container/volumes/mlperf-imagenet-2012/_data",
            144.0,
            142.7,
            1,
        ),
        mk(
            "criteo-1tb-click-logs",
            "/var/lib/container/volumes/criteo-1tb-click-logs/_data",
            1024.0,
            988.4,
            1,
        ),
        mk(
            "pgvector-embeddings-data",
            "/var/lib/container/volumes/pgvector-embeddings-data/_data",
            64.0,
            18.2,
            1,
        ),
        mk(
            "huggingface-cache",
            "/var/lib/container/volumes/huggingface-cache/_data",
            256.0,
            84.1,
            3,
        ),
        mk(
            "mlperf-storage-bench",
            "/var/lib/container/volumes/mlperf-storage-bench/_data",
            512.0,
            0.0,
            0,
        ),
        mk(
            "jupyter-notebooks",
            "/var/lib/container/volumes/jupyter-notebooks/_data",
            32.0,
            4.8,
            1,
        ),
    ]
}

pub fn networks() -> Vec<Network> {
    vec![
        Network {
            id: "net-001".into(),
            name: "default".into(),
            mode: "bridge".into(),
            state: "active".into(),
            subnet: "192.168.64.0/24".into(),
            gateway: "192.168.64.1".into(),
            dns: vec!["1.1.1.1".into(), "8.8.8.8".into()],
            containers: 4,
        },
        Network {
            id: "net-002".into(),
            name: "mlperf-inference".into(),
            mode: "bridge".into(),
            state: "active".into(),
            subnet: "192.168.65.0/24".into(),
            gateway: "192.168.65.1".into(),
            dns: vec!["1.1.1.1".into()],
            containers: 3,
        },
        Network {
            id: "net-003".into(),
            name: "dlrm-training".into(),
            mode: "bridge".into(),
            state: "active".into(),
            subnet: "192.168.66.0/24".into(),
            gateway: "192.168.66.1".into(),
            dns: vec!["1.1.1.1".into()],
            containers: 1,
        },
        Network {
            id: "net-004".into(),
            name: "host".into(),
            mode: "host".into(),
            state: "active".into(),
            subnet: "—".into(),
            gateway: "—".into(),
            dns: vec![],
            containers: 0,
        },
    ]
}

pub fn stacks() -> Vec<Stack> {
    let svc = |n: &str, i: &str, st: &str, h: &str| Service {
        name: n.into(),
        image: i.into(),
        state: st.into(),
        health: h.into(),
    };
    vec![
        Stack {
            name: "mlperf-inference".into(),
            services: vec![
                svc("pgvector", "pgvector/pgvector:pg16", "running", "healthy"),
                svc("redis", "redis:7.2-alpine", "running", "healthy"),
                svc(
                    "inference",
                    "mlcommons/inference:llama2-70b",
                    "running",
                    "healthy",
                ),
            ],
            restart: "always:3".into(),
            health: "✓ healthy (3)".into(),
            file: "~/.config/cgui/stacks/mlperf-inference.toml".into(),
        },
        Stack {
            name: "dlrm-training".into(),
            services: vec![svc(
                "trainer",
                "mlcommons/training:dlrm-criteo",
                "running",
                "waiting",
            )],
            restart: "on-fail:1".into(),
            health: "waiting".into(),
            file: "~/.config/cgui/stacks/dlrm-training.toml".into(),
        },
        Stack {
            name: "resnet-training".into(),
            services: vec![
                svc("loader", "mlcommons/training:resnet50-1.5", "paused", "—"),
                svc("monitor", "prom/prometheus:v2.48", "stopped", "—"),
            ],
            restart: "—".into(),
            health: "partial".into(),
            file: "~/.config/cgui/stacks/resnet-training.toml".into(),
        },
        Stack {
            name: "storage-bench".into(),
            services: vec![svc("fio", "mlcommons/storage:fio-3.36", "stopped", "—")],
            restart: "—".into(),
            health: "—".into(),
            file: "~/.config/cgui/stacks/storage-bench.toml".into(),
        },
    ]
}

pub fn logs() -> Vec<String> {
    [
        "2026-05-02T18:08:14.221Z [INFO]  inference_server: Loading checkpoint from /models/llama2-70b/consolidated.00.pth",
        "2026-05-02T18:08:14.892Z [INFO]  inference_server: Tokenizer loaded — vocab=32000",
        "2026-05-02T18:08:18.104Z [INFO]  inference_server: Model loaded — 70.0B parameters, 142.3 GiB on GPU",
        "2026-05-02T18:08:18.207Z [INFO]  inference_server: Warmup pass — 8 sequences x 512 tokens",
        "2026-05-02T18:08:32.918Z [INFO]  inference_server: Warmup complete — p50=42ms p99=58ms",
        "2026-05-02T18:08:33.001Z [INFO]  inference_server: Listening on 0.0.0.0:8080",
        "2026-05-02T18:08:33.002Z [INFO]  inference_server: Metrics on 0.0.0.0:9090",
        "2026-05-02T18:09:01.443Z [INFO]  inference_server: req_id=8f2a1c batch=4 prompt_tokens=128 completion_tokens=256 latency=384ms",
        "2026-05-02T18:09:02.778Z [INFO]  inference_server: req_id=3b7e9d batch=2 prompt_tokens=64 completion_tokens=128 latency=192ms",
        "2026-05-02T18:09:04.221Z [WARN]  inference_server: req_id=c1d4f8 batch=1 — kv-cache pressure 87% — consider reducing max_concurrent",
        "2026-05-02T18:09:05.108Z [INFO]  inference_server: req_id=5e8a2b batch=4 prompt_tokens=512 completion_tokens=128 latency=412ms",
        "2026-05-02T18:09:06.991Z [INFO]  inference_server: req_id=9c3f7e batch=4 prompt_tokens=256 completion_tokens=512 latency=782ms",
        "2026-05-02T18:09:08.224Z [INFO]  mlperf_logger: SUBMISSION_BEGIN closed_division=\"datacenter\" benchmark=\"llama2-70b\"",
        "2026-05-02T18:09:08.225Z [INFO]  mlperf_logger: TARGET_LATENCY_NS=2000000000 SCENARIO=server",
        "2026-05-02T18:09:11.502Z [INFO]  inference_server: req_id=4d8b3c batch=4 prompt_tokens=128 completion_tokens=384 latency=512ms",
        "2026-05-02T18:09:12.881Z [ERROR] inference_server: req_id=e2a7f9 — CUDA OOM at batch_size=8 — falling back to batch_size=4",
        "2026-05-02T18:09:13.014Z [INFO]  inference_server: req_id=e2a7f9 retry batch=4 prompt_tokens=512 completion_tokens=512 latency=948ms",
        "2026-05-02T18:09:14.224Z [INFO]  inference_server: req_id=8c5d1a batch=4 prompt_tokens=64 completion_tokens=64 latency=124ms",
        "2026-05-02T18:09:15.811Z [INFO]  inference_server: req_id=2f6b4e batch=4 prompt_tokens=128 completion_tokens=256 latency=388ms",
        "2026-05-02T18:09:17.102Z [INFO]  inference_server: req_id=7a4d2c batch=4 prompt_tokens=256 completion_tokens=128 latency=302ms",
        "2026-05-02T18:09:18.402Z [INFO]  inference_server: req_id=1e8b9f batch=2 prompt_tokens=64 completion_tokens=512 latency=698ms",
        "2026-05-02T18:09:19.881Z [INFO]  inference_server: req_id=4c2a8d batch=4 prompt_tokens=128 completion_tokens=128 latency=212ms",
        "2026-05-02T18:09:21.108Z [INFO]  inference_server: req_id=9b5e3f batch=4 prompt_tokens=512 completion_tokens=256 latency=584ms",
        "2026-05-02T18:09:22.404Z [INFO]  inference_server: req_id=6d8c1a batch=4 prompt_tokens=256 completion_tokens=384 latency=712ms",
    ].iter().map(|s| s.to_string()).collect()
}

pub fn pull_stream() -> Vec<String> {
    [
        "Pulling from mlcommons/inference",
        "manifest: digest=sha256:8f3e2a91c2d4...",
        "Layer 1/24: a8c4f7b2 — 142 MB",
        "  ▸ pulling… 14.2 MB / 142.0 MB (10%)",
        "  ▸ pulling… 42.6 MB / 142.0 MB (30%)",
        "  ▸ pulling… 89.3 MB / 142.0 MB (62%)",
        "  ▸ pulling… 142.0 MB / 142.0 MB (100%)",
        "  ✓ extracted",
        "Layer 2/24: b7e9c1f4 — 2.1 GB",
        "  ▸ pulling… 0.4 GB / 2.1 GB (19%)",
        "  ▸ pulling… 1.2 GB / 2.1 GB (57%)",
        "  ▸ pulling… 2.1 GB / 2.1 GB (100%)",
        "  ✓ extracted",
        "Layer 3/24: c4a7d2e8 — 38.2 GB",
        "  ▸ pulling… 4.2 GB / 38.2 GB (11%)",
        "  ▸ pulling… 12.8 GB / 38.2 GB (33%)",
        "  ▸ pulling… 24.1 GB / 38.2 GB (63%)",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect()
}

pub fn trivy() -> TrivyResult {
    let mk =
        |sev: &str, cve: &str, pkg: &str, installed: &str, fixed: &str, title: &str| TrivyFinding {
            sev: sev.into(),
            cve: cve.into(),
            pkg: pkg.into(),
            installed: installed.into(),
            fixed: fixed.into(),
            title: title.into(),
            ..TrivyFinding::default()
        };
    TrivyResult {
        image: "mlcommons/inference:llama2-70b".into(),
        counts: json!({ "CRITICAL": 2, "HIGH": 11, "MEDIUM": 34, "LOW": 87 }),
        findings: vec![
            mk(
                "CRITICAL",
                "CVE-2024-21626",
                "runc",
                "1.1.7",
                "1.1.12",
                "runc: file descriptor leak allows container escape",
            ),
            mk(
                "CRITICAL",
                "CVE-2025-1138",
                "openssl",
                "3.0.11",
                "3.0.14",
                "openssl: heap buffer overflow in X.509 verification",
            ),
            mk(
                "HIGH",
                "CVE-2024-2961",
                "glibc",
                "2.36-9",
                "2.36-9+deb12u4",
                "glibc: iconv() buffer overflow in ISO-2022-CN-EXT charset",
            ),
            mk(
                "HIGH",
                "CVE-2024-3094",
                "xz-utils",
                "5.4.1",
                "5.6.1+really5.4.5-1",
                "xz-utils: malicious code injection in liblzma",
            ),
            mk(
                "HIGH",
                "CVE-2024-6387",
                "openssh-server",
                "9.2p1",
                "9.2p1-2+deb12u3",
                "openssh: regreSSHion — race condition in SIGALRM handler",
            ),
            mk(
                "HIGH",
                "CVE-2025-0911",
                "libcurl4",
                "7.88.1-10",
                "7.88.1-10+deb12u6",
                "curl: HSTS bypass via dotted hostname",
            ),
            mk(
                "HIGH",
                "CVE-2025-2244",
                "libpython3.11",
                "3.11.2",
                "3.11.7",
                "python: integer overflow in tarfile module",
            ),
            mk(
                "HIGH",
                "CVE-2024-45491",
                "libexpat1",
                "2.5.0",
                "2.5.0-1+deb12u1",
                "expat: integer overflow in dtdCopy() leading to memory corruption",
            ),
            mk(
                "HIGH",
                "CVE-2025-3110",
                "libtorch-cpu",
                "2.1.0",
                "2.2.2",
                "pytorch: untrusted deserialization in torch.load() default path",
            ),
            mk(
                "MEDIUM",
                "CVE-2024-7264",
                "libcurl4",
                "7.88.1-10",
                "7.88.1-10+deb12u5",
                "curl: ASN.1 date parser overread",
            ),
            mk(
                "MEDIUM",
                "CVE-2024-9143",
                "libssl3",
                "3.0.11",
                "3.0.13",
                "openssl: low-severity out-of-bounds memory read in BN_GF2m_*",
            ),
            mk(
                "MEDIUM",
                "CVE-2025-0167",
                "libgnutls30",
                "3.7.9",
                "3.8.4",
                "gnutls: timing side-channel in RSA-PSK key exchange",
            ),
        ],
    }
}

pub fn updates() -> Vec<Update> {
    vec![
        Update {
            component: "container".into(), installed: "0.13.0".into(), latest: "0.14.2".into(),
            published: "2026-04-28".into(),
            notes: "## What's new in container 0.14.2\n\n### Performance\n- Reduced cold-start latency by 18% on Apple Silicon hosts.\n- Lazy snapshotting now stable and on by default.\n\n### Security\n- Updated runc to 1.1.12 (addresses CVE-2024-21626).\n- New `--no-new-privileges` default for `container run`.\n\n### Bug fixes\n- Fixed race in volume cleanup that could leak sparse images on `container delete -v`.\n- `container stats` now reports correct memory limits when cgroup v2 hierarchy is partial.".into(),
        },
        Update {
            component: "cgui".into(), installed: "0.13.0".into(), latest: "0.14.2".into(),
            published: "2026-05-01".into(),
            notes: "## cgui 0.14.2\n\n- Per-tab refresh cadence (skip 2s tick on Logs + follow).\n- `y` copies pull/build/log buffer to pbcopy.\n- `--profile <name>` one-shot CLI override (no persist).\n- Stack `cap_add` / `cap_drop` passthrough.\n- Healthcheck `start_period_s` startup grace.".into(),
        },
    ]
}

pub fn doctor() -> Vec<DoctorCheck> {
    let ok = |t: &str| DoctorCheck {
        ok: true,
        text: t.into(),
        warn: None,
    };
    let warn = |t: &str| DoctorCheck {
        ok: false,
        text: t.into(),
        warn: Some(true),
    };
    vec![
        ok("active profile: container → container"),
        ok("`container` resolves to /usr/local/bin/container"),
        ok("`container --version` → container CLI version 0.13.0"),
        ok("container system status: running"),
        warn("no profiles.toml at ~/.config/cgui/profiles.toml (using built-in default)"),
        ok("state.json at ~/.config/cgui/state.json parses cleanly"),
        warn("trivy not on PATH (image scan disabled — `brew install trivy`)"),
        ok("runtime API reachable on /var/run/container.sock"),
    ]
}

pub fn inspect_json() -> String {
    serde_json::to_string_pretty(&json!({
        "Id": "a3f8e2c1b9d4736e8f1a5c2b8d9e4f7a",
        "Name": "mlperf-inference-llama2",
        "Created": "2026-04-30T09:14:22.118Z",
        "State": { "Status": "running", "Running": true, "Pid": 28114, "ExitCode": 0, "StartedAt": "2026-04-30T09:14:22.501Z" },
        "Image": "sha256:8f3e2a91c2d4a7b9d4736e8f1a5c2b8d9e4f7a3c1b9d4736e8f1a5c2b8d9e4f7a",
        "Config": {
            "Hostname": "a3f8e2c1",
            "Image": "mlcommons/inference:llama2-70b",
            "Cmd": ["python","inference_server.py","--model","llama2-70b"],
            "Env": ["PATH=/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin","HF_HOME=/cache","CUDA_VISIBLE_DEVICES=0,1,2,3"],
            "WorkingDir": "/app",
            "ExposedPorts": { "8080/tcp": {}, "9090/tcp": {} }
        },
        "NetworkSettings": { "IPAddress": "192.168.65.4", "Ports": { "8080/tcp": [{ "HostPort": "8080" }], "9090/tcp": [{ "HostPort": "9090" }] } },
        "Mounts": [
            { "Source": "huggingface-cache", "Destination": "/cache", "Type": "volume" },
            { "Source": "/Users/dave/models", "Destination": "/models", "Type": "bind", "RW": false }
        ],
        "HostConfig": { "Memory": 68719476736u64, "NanoCpus": 8000000000u64, "RestartPolicy": { "Name": "always" } }
    })).unwrap_or_default()
}
