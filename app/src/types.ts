// Domain types — must stay in sync with src-tauri/src/model.rs serde shapes.

export type ContainerStatus = 'running' | 'paused' | 'exited' | 'stopped';

export interface MemUsage {
  used: number;
  limit: number;
  unit: string;
  pct: number;
}

export interface Container {
  id: string;
  name: string;
  image: string;
  status: ContainerStatus;
  uptime: string;
  exitCode?: number;
  cpu: number;
  mem: MemUsage;
  ports: string[];
  stack: string | null;
  created: string;
  cpuHistory: number[];
  cmd: string[];
  netIoBps?: number;
  diskIoBps?: number;
  startedUnix?: number;
}

export interface Image {
  id: string;
  ref: string;
  size: number;
  sizeUnit: string;
  created: string;
  tags: string[];
  digest: string;
  layers: number;
}

export interface Volume {
  name: string;
  driver: string;
  mountpoint: string;
  size: number;
  used: number;
  unit: string;
  refs: number;
}

export interface Network {
  id: string;
  name: string;
  mode: string;
  state: string;
  subnet: string;
  gateway: string;
  dns: string[];
  containers: number;
}

/** Per-category disk usage from `container system df`. Mirrors
 *  runtime::DiskUsage / DiskUsageRow (serde camelCase). */
export interface DiskUsageRow {
  total: number;
  active: number;
  sizeInBytes: number;
  reclaimable: number;
}

export interface DiskUsage {
  images: DiskUsageRow;
  containers: DiskUsageRow;
  volumes: DiskUsageRow;
}

export interface Service {
  name: string;
  image: string;
  state: ContainerStatus;
  health: string;
  /** Names of services this one depends on, mirrored from the stack TOML. */
  dependsOn?: string[];
}

export interface Stack {
  name: string;
  services: Service[];
  restart: string;
  health: string;
  file: string;
}

export type Severity = 'CRITICAL' | 'HIGH' | 'MEDIUM' | 'LOW';

export interface TrivyFinding {
  sev: Severity;
  cve: string;
  pkg: string;
  installed: string;
  fixed: string;
  title: string;
  // Optional fields populated by the real trivy parser; absent on legacy
  // fixtures and on findings where trivy didn't report the data.
  cvss?: number;
  description?: string;
  refs?: string[];
}

export interface TrivyResult {
  image: string;
  counts: Record<Severity, number>;
  findings: TrivyFinding[];
}

export interface Update {
  component: string;
  installed: string;
  latest: string;
  published: string;
  notes: string;
  /** GitHub release page for the new version. May be empty on legacy payloads. */
  url?: string;
}

export type DoctorFix =
  | { kind: 'url'; label: string; url: string }
  | { kind: 'copy'; label: string; command: string };

export interface ScanPoint {
  scannedAt: number;
  critical: number;
  high: number;
  medium: number;
  low: number;
  total: number;
}

export interface VulnHistory {
  /** Most-recent-first list of summary points. */
  points: ScanPoint[];
  /** CVE ids that appeared in the latest scan but not the previous one. */
  newSinceLast: string[];
}

export interface HistoryPoint {
  /** Unix-epoch seconds of the sample. */
  ts: number;
  cpu: number;
  /** GiB used at the sample. */
  memUsed: number;
  /** Bytes/sec at the sample. */
  netBps: number;
  diskBps: number;
  status: string;
}

export interface DoctorCheck {
  ok: boolean;
  text: string;
  warn?: boolean;
  fix?: DoctorFix;
}

export type Tab = 'containers' | 'images' | 'volumes' | 'networks' | 'stacks' | 'logs';
export type Runtime = 'container' | 'docker' | 'podman';

export type Modal =
  | { type: 'detail'; payload: Container }
  | { type: 'pull' }
  | { type: 'build' }
  | { type: 'trivy'; image?: string }
  | { type: 'update' }
  | { type: 'doctor' }
  | { type: 'settings' }
  | { type: 'volumeInspect'; name: string }
  | { type: 'networkInspect'; id: string; name: string }
  | { type: 'imageInspect'; reference: string }
  | { type: 'runImage'; image: string }
  | { type: 'onboarding' }
  | { type: 'commandPalette' }
  | { type: 'terminal'; container: Container }
  | { type: 'stackGraph'; stack: Stack }
  | { type: 'help' }
  | null;
