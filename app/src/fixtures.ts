// Fixture data ported from the original Workbench design prototype.
// Used by the Rust backend stubs and as the dev fallback when running outside Tauri.

import type {
  Container, Image, Volume, Network, Stack,
  TrivyResult, Update, DoctorCheck,
} from './types';

export const containers: Container[] = [
  {
    id: 'a3f8e2c1', name: 'mlperf-inference-llama2', image: 'mlcommons/inference:llama2-70b',
    status: 'running', uptime: '2d 14h', cpu: 78.2,
    mem: { used: 38.4, limit: 64.0, unit: 'GiB', pct: 60.0 },
    ports: ['8080:8080', '9090:9090'], stack: 'mlperf-inference', created: '2026-04-30 09:14:22',
    cpuHistory: [12, 18, 24, 35, 48, 62, 71, 78, 82, 79, 76, 78, 80, 78, 75, 78, 81, 78, 76, 78, 77, 78, 79, 78],
    cmd: ['python', 'inference_server.py', '--model', 'llama2-70b'],
  },
  {
    id: 'b7d4a9e2', name: 'pgvector-embeddings', image: 'pgvector/pgvector:pg16',
    status: 'running', uptime: '5d 02h', cpu: 12.4,
    mem: { used: 2.1, limit: 8.0, unit: 'GiB', pct: 26.3 },
    ports: ['15432:5432'], stack: 'mlperf-inference', created: '2026-04-27 21:08:11',
    cpuHistory: [8, 10, 12, 14, 11, 13, 15, 12, 10, 11, 13, 12, 14, 12, 11, 12, 13, 12, 11, 12, 13, 12, 12, 12],
    cmd: ['postgres'],
  },
  {
    id: 'c2e9b3f4', name: 'criteo-trainer', image: 'mlcommons/training:dlrm-criteo',
    status: 'running', uptime: '14h 22m', cpu: 94.8,
    mem: { used: 56.2, limit: 64.0, unit: 'GiB', pct: 87.8 },
    ports: [], stack: 'dlrm-training', created: '2026-05-01 23:42:09',
    cpuHistory: [85, 88, 92, 94, 96, 95, 94, 93, 95, 94, 96, 97, 95, 94, 93, 94, 95, 94, 93, 94, 95, 94, 95, 95],
    cmd: ['python', 'train.py', '--config', 'dlrm.yaml'],
  },
  {
    id: 'd8a1c5b9', name: 'ailuminate-grader', image: 'mlcommons/safety:ailuminate-1.2',
    status: 'running', uptime: '1h 03m', cpu: 24.1,
    mem: { used: 4.8, limit: 16.0, unit: 'GiB', pct: 30.0 },
    ports: ['7860:7860'], stack: null, created: '2026-05-02 13:08:55',
    cpuHistory: [18, 22, 26, 28, 24, 22, 24, 26, 24, 22, 24, 25, 24, 23, 24, 25, 24, 23, 24, 25, 24, 24, 24, 24],
    cmd: ['python', '-m', 'ailuminate.grader'],
  },
  {
    id: 'e4f7d2a8', name: 'redis-eval-cache', image: 'redis:7.2-alpine',
    status: 'running', uptime: '5d 02h', cpu: 1.8,
    mem: { used: 0.18, limit: 1.0, unit: 'GiB', pct: 18.0 },
    ports: ['16379:6379'], stack: 'mlperf-inference', created: '2026-04-27 21:08:11',
    cpuHistory: [1, 2, 1, 2, 3, 1, 2, 2, 1, 2, 1, 2, 2, 1, 2, 1, 2, 2, 1, 2, 1, 2, 2, 2],
    cmd: ['redis-server'],
  },
  {
    id: 'f1c8e6b3', name: 'storage-bench-fio', image: 'mlcommons/storage:fio-3.36',
    status: 'exited', uptime: '—', exitCode: 0, cpu: 0,
    mem: { used: 0, limit: 4.0, unit: 'GiB', pct: 0 },
    ports: [], stack: null, created: '2026-05-02 08:11:33',
    cpuHistory: new Array(24).fill(0),
    cmd: ['fio', '/etc/jobs/seq-read.fio'],
  },
  {
    id: '0a4b9d7e', name: 'jupyter-research', image: 'jupyter/scipy-notebook:python-3.11',
    status: 'running', uptime: '8d 17h', cpu: 6.2,
    mem: { used: 1.4, limit: 8.0, unit: 'GiB', pct: 17.5 },
    ports: ['8888:8888'], stack: null, created: '2026-04-24 06:51:42',
    cpuHistory: [5, 6, 7, 8, 6, 5, 6, 7, 6, 5, 6, 7, 8, 6, 5, 6, 7, 6, 5, 6, 7, 6, 6, 6],
    cmd: ['start-notebook.sh'],
  },
  {
    id: '6b3c8f2a', name: 'imagenet-loader', image: 'mlcommons/training:resnet50-1.5',
    status: 'paused', uptime: '3h 18m', cpu: 0,
    mem: { used: 8.2, limit: 16.0, unit: 'GiB', pct: 51.3 },
    ports: [], stack: 'resnet-training', created: '2026-05-02 11:24:18',
    cpuHistory: [44, 52, 61, 58, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    cmd: ['python', 'train_resnet.py'],
  },
];

export const images: Image[] = [
  { id: 'sha256:8f3e2a91', ref: 'mlcommons/inference:llama2-70b', size: 142.3, sizeUnit: 'GiB', created: '2026-04-29', tags: ['llama2-70b'], digest: 'sha256:8f3e2a91...c2d4', layers: 24 },
  { id: 'sha256:b4c7d9f2', ref: 'mlcommons/training:dlrm-criteo', size: 8.7, sizeUnit: 'GiB', created: '2026-04-22', tags: ['dlrm-criteo', 'latest'], digest: 'sha256:b4c7d9f2...8a1e', layers: 18 },
  { id: 'sha256:e1a8b3c6', ref: 'mlcommons/training:resnet50-1.5', size: 4.2, sizeUnit: 'GiB', created: '2026-03-14', tags: ['resnet50-1.5'], digest: 'sha256:e1a8b3c6...f9d2', layers: 16 },
  { id: 'sha256:7d2f4a8b', ref: 'mlcommons/safety:ailuminate-1.2', size: 6.1, sizeUnit: 'GiB', created: '2026-04-18', tags: ['ailuminate-1.2', 'latest'], digest: 'sha256:7d2f4a8b...3c5e', layers: 21 },
  { id: 'sha256:c5b1e7f3', ref: 'mlcommons/storage:fio-3.36', size: 0.4, sizeUnit: 'GiB', created: '2026-02-08', tags: ['fio-3.36'], digest: 'sha256:c5b1e7f3...a7d8', layers: 8 },
  { id: 'sha256:a9f3b8c1', ref: 'pgvector/pgvector:pg16', size: 0.6, sizeUnit: 'GiB', created: '2026-03-30', tags: ['pg16'], digest: 'sha256:a9f3b8c1...b2e4', layers: 14 },
  { id: 'sha256:4e7c2d9a', ref: 'redis:7.2-alpine', size: 0.04, sizeUnit: 'GiB', created: '2026-04-12', tags: ['7.2-alpine'], digest: 'sha256:4e7c2d9a...8f1c', layers: 7 },
  { id: 'sha256:9b6e4f8d', ref: 'jupyter/scipy-notebook:python-3.11', size: 4.8, sizeUnit: 'GiB', created: '2026-03-02', tags: ['python-3.11'], digest: 'sha256:9b6e4f8d...d3a7', layers: 22 },
  { id: 'sha256:2c8a5b7e', ref: 'docker.io/library/alpine:3.19', size: 0.008, sizeUnit: 'GiB', created: '2026-01-22', tags: ['3.19', 'latest'], digest: 'sha256:2c8a5b7e...e6c1', layers: 1 },
];

export const volumes: Volume[] = [
  { name: 'mlperf-imagenet-2012', driver: 'local', mountpoint: '/var/lib/container/volumes/mlperf-imagenet-2012/_data', size: 144.0, used: 142.7, unit: 'GiB', refs: 1 },
  { name: 'criteo-1tb-click-logs', driver: 'local', mountpoint: '/var/lib/container/volumes/criteo-1tb-click-logs/_data', size: 1024.0, used: 988.4, unit: 'GiB', refs: 1 },
  { name: 'pgvector-embeddings-data', driver: 'local', mountpoint: '/var/lib/container/volumes/pgvector-embeddings-data/_data', size: 64.0, used: 18.2, unit: 'GiB', refs: 1 },
  { name: 'huggingface-cache', driver: 'local', mountpoint: '/var/lib/container/volumes/huggingface-cache/_data', size: 256.0, used: 84.1, unit: 'GiB', refs: 3 },
  { name: 'mlperf-storage-bench', driver: 'local', mountpoint: '/var/lib/container/volumes/mlperf-storage-bench/_data', size: 512.0, used: 0, unit: 'GiB', refs: 0 },
  { name: 'jupyter-notebooks', driver: 'local', mountpoint: '/var/lib/container/volumes/jupyter-notebooks/_data', size: 32.0, used: 4.8, unit: 'GiB', refs: 1 },
];

export const networks: Network[] = [
  { id: 'net-001', name: 'default', mode: 'bridge', state: 'active', subnet: '192.168.64.0/24', gateway: '192.168.64.1', dns: ['1.1.1.1', '8.8.8.8'], containers: 4 },
  { id: 'net-002', name: 'mlperf-inference', mode: 'bridge', state: 'active', subnet: '192.168.65.0/24', gateway: '192.168.65.1', dns: ['1.1.1.1'], containers: 3 },
  { id: 'net-003', name: 'dlrm-training', mode: 'bridge', state: 'active', subnet: '192.168.66.0/24', gateway: '192.168.66.1', dns: ['1.1.1.1'], containers: 1 },
  { id: 'net-004', name: 'host', mode: 'host', state: 'active', subnet: '—', gateway: '—', dns: [], containers: 0 },
];

export const stacks: Stack[] = [
  {
    name: 'mlperf-inference',
    services: [
      { name: 'pgvector', image: 'pgvector/pgvector:pg16', state: 'running', health: 'healthy' },
      { name: 'redis', image: 'redis:7.2-alpine', state: 'running', health: 'healthy' },
      { name: 'inference', image: 'mlcommons/inference:llama2-70b', state: 'running', health: 'healthy', dependsOn: ['pgvector', 'redis'] },
    ],
    restart: 'always:3', health: '✓ healthy (3)', file: '~/.config/cgui/stacks/mlperf-inference.toml',
  },
  {
    name: 'dlrm-training',
    services: [{ name: 'trainer', image: 'mlcommons/training:dlrm-criteo', state: 'running', health: 'waiting' }],
    restart: 'on-fail:1', health: 'waiting', file: '~/.config/cgui/stacks/dlrm-training.toml',
  },
  {
    name: 'resnet-training',
    services: [
      { name: 'loader', image: 'mlcommons/training:resnet50-1.5', state: 'paused', health: '—' },
      { name: 'monitor', image: 'prom/prometheus:v2.48', state: 'stopped', health: '—' },
    ],
    restart: '—', health: 'partial', file: '~/.config/cgui/stacks/resnet-training.toml',
  },
  {
    name: 'storage-bench',
    services: [{ name: 'fio', image: 'mlcommons/storage:fio-3.36', state: 'stopped', health: '—' }],
    restart: '—', health: '—', file: '~/.config/cgui/stacks/storage-bench.toml',
  },
];

export const logs: string[] = [
  '2026-05-02T18:08:14.221Z [INFO]  inference_server: Loading checkpoint from /models/llama2-70b/consolidated.00.pth',
  '2026-05-02T18:08:14.892Z [INFO]  inference_server: Tokenizer loaded — vocab=32000',
  '2026-05-02T18:08:18.104Z [INFO]  inference_server: Model loaded — 70.0B parameters, 142.3 GiB on GPU',
  '2026-05-02T18:08:18.207Z [INFO]  inference_server: Warmup pass — 8 sequences x 512 tokens',
  '2026-05-02T18:08:32.918Z [INFO]  inference_server: Warmup complete — p50=42ms p99=58ms',
  '2026-05-02T18:08:33.001Z [INFO]  inference_server: Listening on 0.0.0.0:8080',
  '2026-05-02T18:08:33.002Z [INFO]  inference_server: Metrics on 0.0.0.0:9090',
  '2026-05-02T18:09:01.443Z [INFO]  inference_server: req_id=8f2a1c batch=4 prompt_tokens=128 completion_tokens=256 latency=384ms',
  '2026-05-02T18:09:02.778Z [INFO]  inference_server: req_id=3b7e9d batch=2 prompt_tokens=64 completion_tokens=128 latency=192ms',
  '2026-05-02T18:09:04.221Z [WARN]  inference_server: req_id=c1d4f8 batch=1 — kv-cache pressure 87% — consider reducing max_concurrent',
  '2026-05-02T18:09:05.108Z [INFO]  inference_server: req_id=5e8a2b batch=4 prompt_tokens=512 completion_tokens=128 latency=412ms',
  '2026-05-02T18:09:06.991Z [INFO]  inference_server: req_id=9c3f7e batch=4 prompt_tokens=256 completion_tokens=512 latency=782ms',
  '2026-05-02T18:09:08.224Z [INFO]  mlperf_logger: SUBMISSION_BEGIN closed_division="datacenter" benchmark="llama2-70b"',
  '2026-05-02T18:09:08.225Z [INFO]  mlperf_logger: TARGET_LATENCY_NS=2000000000 SCENARIO=server',
  '2026-05-02T18:09:11.502Z [INFO]  inference_server: req_id=4d8b3c batch=4 prompt_tokens=128 completion_tokens=384 latency=512ms',
  '2026-05-02T18:09:12.881Z [ERROR] inference_server: req_id=e2a7f9 — CUDA OOM at batch_size=8 — falling back to batch_size=4',
  '2026-05-02T18:09:13.014Z [INFO]  inference_server: req_id=e2a7f9 retry batch=4 prompt_tokens=512 completion_tokens=512 latency=948ms',
  '2026-05-02T18:09:14.224Z [INFO]  inference_server: req_id=8c5d1a batch=4 prompt_tokens=64 completion_tokens=64 latency=124ms',
  '2026-05-02T18:09:15.811Z [INFO]  inference_server: req_id=2f6b4e batch=4 prompt_tokens=128 completion_tokens=256 latency=388ms',
  '2026-05-02T18:09:17.102Z [INFO]  inference_server: req_id=7a4d2c batch=4 prompt_tokens=256 completion_tokens=128 latency=302ms',
  '2026-05-02T18:09:18.402Z [INFO]  inference_server: req_id=1e8b9f batch=2 prompt_tokens=64 completion_tokens=512 latency=698ms',
  '2026-05-02T18:09:19.881Z [INFO]  inference_server: req_id=4c2a8d batch=4 prompt_tokens=128 completion_tokens=128 latency=212ms',
  '2026-05-02T18:09:21.108Z [INFO]  inference_server: req_id=9b5e3f batch=4 prompt_tokens=512 completion_tokens=256 latency=584ms',
  '2026-05-02T18:09:22.404Z [INFO]  inference_server: req_id=6d8c1a batch=4 prompt_tokens=256 completion_tokens=384 latency=712ms',
];

export const pullStream: string[] = [
  'Pulling from mlcommons/inference',
  'manifest: digest=sha256:8f3e2a91c2d4...',
  'Layer 1/24: a8c4f7b2 — 142 MB',
  '  ▸ pulling… 14.2 MB / 142.0 MB (10%)',
  '  ▸ pulling… 42.6 MB / 142.0 MB (30%)',
  '  ▸ pulling… 89.3 MB / 142.0 MB (62%)',
  '  ▸ pulling… 142.0 MB / 142.0 MB (100%)',
  '  ✓ extracted',
  'Layer 2/24: b7e9c1f4 — 2.1 GB',
  '  ▸ pulling… 0.4 GB / 2.1 GB (19%)',
  '  ▸ pulling… 1.2 GB / 2.1 GB (57%)',
  '  ▸ pulling… 2.1 GB / 2.1 GB (100%)',
  '  ✓ extracted',
  'Layer 3/24: c4a7d2e8 — 38.2 GB',
  '  ▸ pulling… 4.2 GB / 38.2 GB (11%)',
  '  ▸ pulling… 12.8 GB / 38.2 GB (33%)',
  '  ▸ pulling… 24.1 GB / 38.2 GB (63%)',
];

export const trivy: TrivyResult = {
  image: 'mlcommons/inference:llama2-70b',
  counts: { CRITICAL: 2, HIGH: 11, MEDIUM: 34, LOW: 87 },
  findings: [
    { sev: 'CRITICAL', cve: 'CVE-2024-21626', pkg: 'runc', installed: '1.1.7', fixed: '1.1.12', title: 'runc: file descriptor leak allows container escape' },
    { sev: 'CRITICAL', cve: 'CVE-2025-1138', pkg: 'openssl', installed: '3.0.11', fixed: '3.0.14', title: 'openssl: heap buffer overflow in X.509 verification' },
    { sev: 'HIGH', cve: 'CVE-2024-2961', pkg: 'glibc', installed: '2.36-9', fixed: '2.36-9+deb12u4', title: 'glibc: iconv() buffer overflow in ISO-2022-CN-EXT charset' },
    { sev: 'HIGH', cve: 'CVE-2024-3094', pkg: 'xz-utils', installed: '5.4.1', fixed: '5.6.1+really5.4.5-1', title: 'xz-utils: malicious code injection in liblzma' },
    { sev: 'HIGH', cve: 'CVE-2024-6387', pkg: 'openssh-server', installed: '9.2p1', fixed: '9.2p1-2+deb12u3', title: 'openssh: regreSSHion — race condition in SIGALRM handler' },
    { sev: 'HIGH', cve: 'CVE-2025-0911', pkg: 'libcurl4', installed: '7.88.1-10', fixed: '7.88.1-10+deb12u6', title: 'curl: HSTS bypass via dotted hostname' },
    { sev: 'HIGH', cve: 'CVE-2025-2244', pkg: 'libpython3.11', installed: '3.11.2', fixed: '3.11.7', title: 'python: integer overflow in tarfile module' },
    { sev: 'HIGH', cve: 'CVE-2024-45491', pkg: 'libexpat1', installed: '2.5.0', fixed: '2.5.0-1+deb12u1', title: 'expat: integer overflow in dtdCopy() leading to memory corruption' },
    { sev: 'HIGH', cve: 'CVE-2025-3110', pkg: 'libtorch-cpu', installed: '2.1.0', fixed: '2.2.2', title: 'pytorch: untrusted deserialization in torch.load() default path' },
    { sev: 'MEDIUM', cve: 'CVE-2024-7264', pkg: 'libcurl4', installed: '7.88.1-10', fixed: '7.88.1-10+deb12u5', title: 'curl: ASN.1 date parser overread' },
    { sev: 'MEDIUM', cve: 'CVE-2024-9143', pkg: 'libssl3', installed: '3.0.11', fixed: '3.0.13', title: 'openssl: low-severity out-of-bounds memory read in BN_GF2m_*' },
    { sev: 'MEDIUM', cve: 'CVE-2025-0167', pkg: 'libgnutls30', installed: '3.7.9', fixed: '3.8.4', title: 'gnutls: timing side-channel in RSA-PSK key exchange' },
  ],
};

export const updates: Update[] = [
  {
    component: 'container', installed: '0.13.0', latest: '0.14.2', published: '2026-04-28',
    notes: [
      "## What's new in container 0.14.2",
      '',
      '### Performance',
      '- Reduced cold-start latency by 18% on Apple Silicon hosts.',
      '- Lazy snapshotting now stable and on by default.',
      '',
      '### Security',
      '- Updated runc to 1.1.12 (addresses CVE-2024-21626).',
      '- New `--no-new-privileges` default for `container run`.',
      '',
      '### Bug fixes',
      '- Fixed race in volume cleanup that could leak sparse images on `container delete -v`.',
      '- `container stats` now reports correct memory limits when cgroup v2 hierarchy is partial.',
    ].join('\n'),
  },
  {
    component: 'cgui', installed: '0.13.0', latest: '0.14.2', published: '2026-05-01',
    notes: [
      '## cgui 0.14.2',
      '',
      '- Per-tab refresh cadence (skip 2s tick on Logs + follow).',
      '- `y` copies pull/build/log buffer to pbcopy.',
      '- `--profile <name>` one-shot CLI override (no persist).',
      '- Stack `cap_add` / `cap_drop` passthrough.',
      '- Healthcheck `start_period_s` startup grace.',
    ].join('\n'),
  },
];

export const doctor: DoctorCheck[] = [
  { ok: true,  text: 'active profile: container → container' },
  { ok: true,  text: '`container` resolves to /usr/local/bin/container' },
  { ok: true,  text: '`container --version` → container CLI version 0.13.0' },
  { ok: true,  text: 'container system status: running' },
  { ok: false, text: 'no profiles.toml at ~/.config/cgui/profiles.toml (using built-in default)', warn: true },
  { ok: true,  text: 'state.json at ~/.config/cgui/state.json parses cleanly' },
  { ok: false, text: 'trivy not on PATH (image scan disabled — `brew install trivy`)', warn: true },
  { ok: true,  text: 'runtime API reachable on /var/run/container.sock' },
];

export const inspectJson: string = JSON.stringify({
  Id: 'a3f8e2c1b9d4736e8f1a5c2b8d9e4f7a',
  Name: 'mlperf-inference-llama2',
  Created: '2026-04-30T09:14:22.118Z',
  State: { Status: 'running', Running: true, Pid: 28114, ExitCode: 0, StartedAt: '2026-04-30T09:14:22.501Z' },
  Image: 'sha256:8f3e2a91c2d4a7b9d4736e8f1a5c2b8d9e4f7a3c1b9d4736e8f1a5c2b8d9e4f7a',
  Config: {
    Hostname: 'a3f8e2c1',
    Image: 'mlcommons/inference:llama2-70b',
    Cmd: ['python', 'inference_server.py', '--model', 'llama2-70b'],
    Env: ['PATH=/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin', 'HF_HOME=/cache', 'CUDA_VISIBLE_DEVICES=0,1,2,3'],
    WorkingDir: '/app',
    ExposedPorts: { '8080/tcp': {}, '9090/tcp': {} },
  },
  NetworkSettings: { IPAddress: '192.168.65.4', Ports: { '8080/tcp': [{ HostPort: '8080' }], '9090/tcp': [{ HostPort: '9090' }] } },
  Mounts: [
    { Source: 'huggingface-cache', Destination: '/cache', Type: 'volume' },
    { Source: '/Users/dave/models', Destination: '/models', Type: 'bind', RW: false },
  ],
  HostConfig: { Memory: 68719476736, NanoCpus: 8000000000, RestartPolicy: { Name: 'always' } },
}, null, 2);
