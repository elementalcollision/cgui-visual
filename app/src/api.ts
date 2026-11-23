// Tauri invoke wrapper. Falls back to in-process fixtures when running outside Tauri
// (e.g., `vite dev` in a regular browser tab) so the UI is still usable for design work.

import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import type {
  Container, Image, ImageListing, Volume, Network, StackListing,
  ScanResponse, Update, DoctorCheck, HistoryPoint, VulnHistory, DiskUsage,
} from './types';
import * as fixtures from './fixtures';

const inTauri = typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;

export type Prefs = {
  dark: boolean;
  sidebarCollapsed: boolean;
  lastTab: string;
  menubarMode: boolean;
  globalHotkey: string;
  notifyOnExit: boolean;
  backgroundLogs: boolean;
};

const PREFS_DEFAULT: Prefs = {
  dark: true, sidebarCollapsed: false, lastTab: 'containers',
  menubarMode: false, globalHotkey: '', notifyOnExit: true, backgroundLogs: true,
};

async function call<T>(cmd: string, fallback: T, args?: Record<string, unknown>): Promise<T> {
  if (!inTauri) return fallback;
  try {
    return await invoke<T>(cmd, args);
  } catch (e) {
    console.warn(`invoke ${cmd} failed, using fallback`, e);
    return fallback;
  }
}

// For list/inspect commands whose failures the UI must show (never as
// empty data): outside Tauri (`vite dev`) return the fixture; inside
// Tauri propagate the rejection so views can render an error state.
async function callOrDevFallback<T>(cmd: string, fallback: T, args?: Record<string, unknown>): Promise<T> {
  if (!inTauri) return fallback;
  return invoke<T>(cmd, args);
}

// For commands where we want to surface errors to the user (actions like
// stop/delete), don't swallow.
async function invokeStrict<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  if (!inTauri) throw new Error('not running in Tauri');
  return invoke<T>(cmd, args);
}

export const api = {
  // Lists / inspect
  // These reject on CLI failure (services stopped, etc.) so views can show
  // an explicit error state rather than "nothing exists".
  listContainers: () => callOrDevFallback<Container[]>('list_containers', fixtures.containers),
  listImages:     () => callOrDevFallback<Image[]>('list_images', fixtures.images),
  listImagesChecked: () => inTauri
    ? invoke<ImageListing>('list_images_checked')
    : Promise.resolve<ImageListing>({ images: fixtures.images, warning: null }),
  listVolumes:    () => callOrDevFallback<Volume[]>('list_volumes', fixtures.volumes),
  listNetworks:   () => callOrDevFallback<Network[]>('list_networks', fixtures.networks),
  listStacks:     () => callOrDevFallback<StackListing>('list_stacks', { stacks: fixtures.stacks, invalid: [] }),
  inspectContainer: (id: string) => callOrDevFallback<string>('inspect_container', fixtures.inspectJson, { id }),
  doctor:         () => call<DoctorCheck[]>('doctor', fixtures.doctor),
  scanImage:      (image: string) => callOrDevFallback<ScanResponse>('scan_image', { status: 'ok', result: fixtures.trivy }, { image }),
  listUpdates:    () => callOrDevFallback<Update[]>('list_updates', fixtures.updates),

  // Container actions — strict; UI surfaces errors via console + (later) toasts.
  startContainer:   (id: string) => inTauri ? invokeStrict<void>('start_container',   { id }) : Promise.resolve(),
  stopContainer:    (id: string) => inTauri ? invokeStrict<void>('stop_container',    { id }) : Promise.resolve(),
  killContainer:    (id: string, signal?: string) => inTauri ? invokeStrict<void>('kill_container', { id, signal }) : Promise.resolve(),
  deleteContainer:  (id: string) => inTauri ? invokeStrict<void>('delete_container',  { id }) : Promise.resolve(),
  restartContainer: (id: string) => inTauri ? invokeStrict<void>('restart_container', { id }) : Promise.resolve(),
  // Reclaim space (`container clean`) — takes a list of ids since the CLI
  // supports cleaning several containers in one invocation; callers pass
  // a single-element array for the per-row/detail-pane action.
  cleanContainers: (ids: string[]) => inTauri ? invokeStrict<void>('clean_containers', { ids }) : Promise.resolve(),

  // Streams: backend spawns a child process, frontend subscribes to events.
  // `streamId` is chosen by the caller (see newStreamId) so it can subscribe
  // to `logs:tick:<streamId>` before the child starts; the backend keeps a
  // registry keyed by it and `stopLogStream` kills the child.
  startLogStream: (streamId: string, id: string, opts?: { boot?: boolean; tail?: number }) =>
    inTauri ? invokeStrict<void>('start_log_stream', { streamId, id, boot: opts?.boot, tail: opts?.tail }) : Promise.resolve(),
  stopLogStream: (streamId: string) =>
    inTauri ? invokeStrict<void>('stop_log_stream', { streamId }) : Promise.resolve(),
  startPull:      (reference: string) => inTauri ? invokeStrict<void>('start_pull', { reference }) : Promise.resolve(),

  // Deletes (strict — caller should confirm first and surface errors via toast).
  deleteImage:   (reference: string) => inTauri ? invokeStrict<void>('delete_image', { reference }) : Promise.resolve(),
  deleteVolume:  (name: string)      => inTauri ? invokeStrict<void>('delete_volume', { name }) : Promise.resolve(),
  deleteNetwork: (id: string)        => inTauri ? invokeStrict<void>('delete_network', { id }) : Promise.resolve(),
  // Create (`container volume create` / `container network create`).
  createVolume:  (args: { name: string; size?: string; labels?: string[] }) =>
    inTauri ? invokeStrict<void>('create_volume', { args }) : Promise.resolve(),
  createNetwork: (args: { name: string; subnet?: string; subnetV6?: string; internal?: boolean; labels?: string[] }) =>
    inTauri ? invokeStrict<void>('create_network', { args }) : Promise.resolve(),
  execContainer: (id: string)        => inTauri ? invokeStrict<void>('exec_container', { id }) : Promise.resolve(),

  // Inspect: returns pretty JSON. In browser-dev mode hands back a small placeholder.
  inspectVolume:  (name: string)      => inTauri ? invokeStrict<string>('inspect_volume',  { name })      : Promise.resolve('{\n  "name": "' + name + '",\n  "driver": "local"\n}'),
  inspectNetwork: (id: string)        => inTauri ? invokeStrict<string>('inspect_network', { id })        : Promise.resolve('{\n  "id": "' + id + '"\n}'),
  inspectImage:   (reference: string) => inTauri ? invokeStrict<string>('inspect_image',   { reference }) : Promise.resolve('{\n  "reference": "' + reference + '",\n  "layers": []\n}'),

  // run image: returns the new container id on success. The advanced
  // block (resources/mounts/identity/platform) is phase-4 parity.
  runImage: (args: {
    image: string; name?: string; ports?: string[]; env?: string[]; command?: string;
    cpus?: number; memory?: string; volumes?: string[]; labels?: string[];
    network?: string; aliases?: string[]; workdir?: string; user?: string; platform?: string;
    rm?: boolean; rosetta?: boolean;
  }) =>
    inTauri ? invokeStrict<string>('run_image', { args }) : Promise.resolve('dev-mode-no-op'),

  // Container machines (phase 4): named VMs that containers run in.
  machineList: () => call<{ id: string; status: string; cpus: number; memory: number; diskSize: number; isDefault: boolean; createdDate: string }[]>('machine_list', []),
  // `userOpts` (container 1.6+): the machine account's name/uid/gid/home.
  machineCreate: (name: string, image: string, cpus?: number, memory?: string, noBoot = true,
                  userOpts?: { user?: string; uid?: number; gid?: number; home?: string }) =>
    inTauri ? invokeStrict<void>('machine_create', { name, image, cpus, memory, noBoot, userOpts }) : Promise.resolve(),
  // One-shot `machine logs` (stdio, or the VM boot log with `boot`).
  machineLogs: (name: string, opts?: { boot?: boolean; tail?: number }) =>
    inTauri ? invokeStrict<string>('machine_logs', { name, boot: opts?.boot, tail: opts?.tail })
            : Promise.resolve(`[dev mode] ${opts?.boot ? 'boot' : 'stdio'} log for ${name}\n`),
  machineInspect: (name: string) =>
    inTauri ? invokeStrict<string>('machine_inspect', { name })
            : Promise.resolve(`[\n  {\n    "id": "${name}"\n  }\n]`),
  machineStop: (name: string) => inTauri ? invokeStrict<void>('machine_stop', { name }) : Promise.resolve(),
  machineDelete: (name: string) => inTauri ? invokeStrict<void>('machine_delete', { name }) : Promise.resolve(),
  machineSetDefault: (name: string) => inTauri ? invokeStrict<void>('machine_set_default', { name }) : Promise.resolve(),

  // Commit a container's filesystem to a new image (`container commit`,
  // 1.6+). Resolves to the committed reference as the CLI reports it.
  commitContainer: (id: string, reference: string) =>
    inTauri ? invokeStrict<string>('commit_container', { id, reference }) : Promise.resolve(reference),

  // Tag an existing image with a new reference.
  tagImage: (source: string, target: string) =>
    inTauri ? invokeStrict<void>('tag_image', { source, target }) : Promise.resolve(),

  // Prune (strict — caller confirms first). Resolves to the CLI's stdout
  // summary of what was removed/reclaimed, for toasting.
  pruneContainers: () => inTauri ? invokeStrict<string>('prune_containers') : Promise.resolve('(dev-mode no-op)'),
  pruneImages:     () => inTauri ? invokeStrict<string>('prune_images')     : Promise.resolve('(dev-mode no-op)'),
  pruneVolumes:    () => inTauri ? invokeStrict<string>('prune_volumes')    : Promise.resolve('(dev-mode no-op)'),
  pruneNetworks:   () => inTauri ? invokeStrict<string>('prune_networks')   : Promise.resolve('(dev-mode no-op)'),

  // Disk usage per category from `container system df`.
  systemDf: () => call<DiskUsage>('system_df', {
    images:     { total: 9, active: 1, sizeInBytes: 3284578304, reclaimable: 2858975232 },
    containers: { total: 1, active: 0, sizeInBytes: 760791040, reclaimable: 760791040 },
    volumes:    { total: 0, active: 0, sizeInBytes: 0, reclaimable: 0 },
  }),

  // Detected Apple container CLI version (`1.7.1`), or null when unknown.
  // The backend caches it; restarting the services clears that cache.
  // Browser dev mode reports a current version so gated features are usable.
  cliVersion: () => call<string | null>('cli_version', '1.7.1'),

  // First-run onboarding probe. Outside Tauri (browser dev mode) the
  // fallback returns true so the OnboardingModal doesn't trigger when the
  // entire data layer is fixtures by design.
  runtimeAvailable: () => call<boolean>('runtime_available', true),

  // Compose import: read a docker-compose.yml at `path`, write a converted
  // stack TOML to ~/.config/cgui/stacks/<name>.toml. Returns the
  // destination path; throws if the parser fails or the stack already
  // exists (unless overwrite=true). Browser-dev no-op.
  importCompose: (path: string, overwrite = false) =>
    inTauri ? invokeStrict<string>('import_compose', { path, overwrite }) :
              Promise.resolve('(dev-mode no-op)'),

  // Render a stack as docker-compose YAML. Browser-dev mode hands back
  // a deterministic stub so the download flow can still be exercised.
  exportCompose: (name: string) =>
    inTauri ? invokeStrict<string>('export_compose', { name }) :
              Promise.resolve(`name: ${name}\nservices: {}\n`),

  // Stack snapshot / restore (B11). Snapshot returns the JSON envelope
  // string the frontend then saves via blob download. Restore takes
  // the JSON content (read from a user-picked file) and writes the
  // contained TOML to ~/.config/cgui/stacks/<name>.toml.
  snapshotStack: (name: string, note?: string) =>
    inTauri ? invokeStrict<string>('snapshot_stack', { name, note }) :
              Promise.resolve(JSON.stringify({
                kind: 'cgui-snapshot', version: 1,
                createdAt: new Date().toISOString(),
                cguiVersion: '0.0.0-dev',
                stack: { name, toml: `name = "${name}"\n` },
              }, null, 2)),
  restoreStack: (json: string, overwrite = false) =>
    inTauri ? invokeStrict<string>('restore_stack', { json, overwrite }) :
              Promise.resolve('(dev-mode no-op)'),
  restoreStackFromPath: (path: string, overwrite = false) =>
    inTauri ? invokeStrict<string>('restore_stack_from_path', { path, overwrite }) :
              Promise.resolve('(dev-mode no-op)'),

  // Long-form per-container metrics (B6). Returns ascending-by-ts
  // points within the last `sinceSecs` seconds. Empty when the
  // sidecar DB hasn't recorded any rows for this id yet.
  containerHistory: (id: string, sinceSecs: number) =>
    call<HistoryPoint[]>('container_history', [], { id, sinceSecs }),

  // Per-image trivy scan history (B7). `limit` caps the number of
  // points returned; defaults to 60 backend-side (~last 60 scans).
  vulnHistory: (image: string, limit?: number) =>
    call<VulnHistory>('vuln_history', { points: [], newSinceLast: [] }, { image, limit }),

  // Persisted log lines for a container (B12). Returned ascending-by-ts
  // so the LogsView can append them to its render buffer as if they
  // arrived live. `query` is an optional substring filter.
  loadLogs: (containerId: string, limit?: number, query?: string) =>
    call<{ ts: number; line: string }[]>('load_logs', [], { containerId, limit, query }),

  // Embedded terminal (B5). Open returns a session id used to address
  // subsequent write/resize/close calls. The frontend subscribes to
  // pty:tick:<id> events and feeds them straight into xterm.js.
  ptyOpen: (id: string, cols: number, rows: number, shell?: string) =>
    inTauri ? invokeStrict<string>('pty_open', { id, shell, cols, rows }) :
              Promise.resolve('dev-mode-session'),
  ptyWrite: (sessionId: string, data: string) =>
    inTauri ? invokeStrict<void>('pty_write', { sessionId, data }) :
              Promise.resolve(),
  ptyResize: (sessionId: string, cols: number, rows: number) =>
    inTauri ? invokeStrict<void>('pty_resize', { sessionId, cols, rows }) :
              Promise.resolve(),
  ptyClose: (sessionId: string) =>
    inTauri ? invokeStrict<void>('pty_close', { sessionId }) :
              Promise.resolve(),
  // Subscribe to one specific pty session's tick events. Payloads are
  // base64-encoded raw bytes (binary-safe).
  onPtyTick: async (sessionId: string, cb: (b64: string) => void): Promise<UnlistenFn> => {
    if (!inTauri) {
      // Print a friendly stub line in dev mode so the modal isn't blank.
      const enc = btoa(`[dev mode: pty session ${sessionId}]\r\n$ `);
      window.setTimeout(() => cb(enc), 50);
      return () => {};
    }
    return listen<string>(`pty:tick:${sessionId}`, e => cb(e.payload));
  },
  onPtyDone: async (sessionId: string, cb: () => void): Promise<UnlistenFn> => {
    if (!inTauri) return () => {};
    return listen<boolean>(`pty:done:${sessionId}`, () => cb());
  },

  // Stacks: up/down return per-line log output; health returns
  // [serviceName, "healthy"|"unhealthy"|"—"|"unsupported:<kind>"].
  stackUp:     (name: string) => inTauri ? invokeStrict<string[]>('stack_up',     { name }) : Promise.resolve(['(dev-mode no-op)']),
  stackDown:   (name: string) => inTauri ? invokeStrict<string[]>('stack_down',   { name }) : Promise.resolve(['(dev-mode no-op)']),
  stackHealth: (name: string) => inTauri ? invokeStrict<[string, string][]>('stack_health', { name }) : Promise.resolve([] as [string, string][]),

  // Event subscriptions. All return an unlisten fn; no-ops outside Tauri.
  onContainersTick: async (cb: (cs: Container[]) => void): Promise<UnlistenFn> => {
    if (!inTauri) return () => {};
    return listen<Container[]>('containers:tick', e => cb(e.payload));
  },
  // Emitted on each poll tick while the container poll is failing (payload:
  // the CLI's error text). No-op outside Tauri.
  onContainersError: async (cb: (message: string) => void): Promise<UnlistenFn> => {
    if (!inTauri) return () => {};
    return listen<string>('containers:tickError', e => cb(e.payload));
  },
  onLogLine: async (streamId: string, cb: (line: string) => void): Promise<UnlistenFn> => {
    if (!inTauri) {
      // Replay fixture lines on a timer so the LogsView still has movement.
      let i = 0;
      const id = window.setInterval(() => {
        if (i >= fixtures.logs.length) { window.clearInterval(id); return; }
        cb(fixtures.logs[i++]);
      }, 80);
      return () => window.clearInterval(id);
    }
    return listen<string>(`logs:tick:${streamId}`, e => cb(e.payload));
  },
  onPullLine: async (cb: (line: string) => void): Promise<UnlistenFn> => {
    if (!inTauri) {
      let i = 0;
      const id = window.setInterval(() => {
        if (i >= fixtures.pullStream.length) { window.clearInterval(id); return; }
        cb(fixtures.pullStream[i++]);
      }, 380);
      return () => window.clearInterval(id);
    }
    return listen<string>('pull:tick', e => cb(e.payload));
  },
  onPullDone: async (cb: (ok: boolean) => void): Promise<UnlistenFn> => {
    if (!inTauri) return () => {};
    return listen<boolean>('pull:done', e => cb(e.payload));
  },
  onPushLine: async (cb: (line: string) => void): Promise<UnlistenFn> => {
    if (!inTauri) {
      // Reuse the pull fixture stream so the push modal animates in dev.
      let i = 0;
      const id = window.setInterval(() => {
        if (i >= fixtures.pullStream.length) { window.clearInterval(id); return; }
        cb(fixtures.pullStream[i++]);
      }, 380);
      return () => window.clearInterval(id);
    }
    return listen<string>('push:tick', e => cb(e.payload));
  },
  onPushDone: async (cb: (ok: boolean) => void): Promise<UnlistenFn> => {
    if (!inTauri) return () => {};
    return listen<boolean>('push:done', e => cb(e.payload));
  },

  // Image lifecycle (phase 2). save/load move whole archives — strict,
  // long-running, surfaced via toast by callers.
  startPush: (reference: string) => inTauri ? invokeStrict<void>('start_push', { reference }) : Promise.resolve(),
  saveImage: (reference: string, output: string) =>
    inTauri ? invokeStrict<void>('save_image', { reference, output }) : Promise.resolve(),
  loadImage: (input: string) =>
    inTauri ? invokeStrict<string>('load_image', { input }) : Promise.resolve('(dev-mode no-op)'),

  // Container filesystem (phase 2). Either side of copy may be `<id>:<path>`.
  copyPath: (src: string, dst: string) =>
    inTauri ? invokeStrict<void>('copy_path', { src, dst }) : Promise.resolve(),
  exportContainer: (id: string, output: string) =>
    inTauri ? invokeStrict<void>('export_container', { id, output }) : Promise.resolve(),

  // Build experience (phase 3). The builder is a dedicated VM; status
  // returns null when no builder instance exists yet.
  // Rejects on CLI failure so the Builder panel can offer Start services.
  builderStatus: () => callOrDevFallback<Container | null>('builder_status', null),
  builderStart: (cpus?: number, memory?: string) =>
    inTauri ? invokeStrict<void>('builder_start', { cpus, memory }) : Promise.resolve(),
  builderStop: () => inTauri ? invokeStrict<void>('builder_stop') : Promise.resolve(),
  builderDelete: () => inTauri ? invokeStrict<void>('builder_delete') : Promise.resolve(),

  // Runtime service controls (sudo-free). Surfaced in Doctor (start) and
  // Settings (start/stop).
  systemRestart: () => inTauri ? invokeStrict<void>('system_restart') : Promise.resolve(),
  systemStart: () => inTauri ? invokeStrict<void>('system_start') : Promise.resolve(),
  systemStop:  () => inTauri ? invokeStrict<void>('system_stop')  : Promise.resolve(),
  // Whether services are up; drives the Settings Services status dot.
  // Dev/browser mode reports true so the panel renders a sane default.
  systemStatus: () => call<boolean>('system_status', true),
  startBuild: (args: { context: string; tag?: string; dockerfile?: string; buildArgs?: string[]; target?: string; noCache?: boolean }) =>
    inTauri ? invokeStrict<void>('start_build', { args }) : Promise.resolve(),
  onBuildLine: async (cb: (line: string) => void): Promise<UnlistenFn> => {
    if (!inTauri) {
      let i = 0;
      const id = window.setInterval(() => {
        if (i >= fixtures.pullStream.length) { window.clearInterval(id); return; }
        cb(fixtures.pullStream[i++]);
      }, 200);
      return () => window.clearInterval(id);
    }
    return listen<string>('build:tick', e => cb(e.payload));
  },
  onBuildDone: async (cb: (ok: boolean) => void): Promise<UnlistenFn> => {
    if (!inTauri) return () => {};
    return listen<boolean>('build:done', e => cb(e.payload));
  },

  // Registry logins (phase 2). The password goes straight to the CLI's
  // stdin and into the user's keychain — never persisted app-side.
  registryList: () => call<{ hostname: string; username: string }[]>('registry_list', []),
  registryLogin: (server: string, username: string, password: string) =>
    inTauri ? invokeStrict<void>('registry_login', { server, username, password }) : Promise.resolve(),
  registryLogout: (server: string) =>
    inTauri ? invokeStrict<void>('registry_logout', { server }) : Promise.resolve(),

  // Prefs persistence. Outside Tauri, hand back defaults / no-op save.
  loadPrefs: () => call<Prefs>('load_prefs', PREFS_DEFAULT),
  savePrefs: (p: Prefs) => inTauri ? invokeStrict<void>('save_prefs', { prefs: p }).catch(e => console.warn('save_prefs failed', e)) : Promise.resolve(),

  // Apply / clear the global summon hotkey at runtime. Empty string clears.
  // Surfaces parser / OS-conflict errors so Settings can show them.
  setGlobalHotkey: (accelerator: string) =>
    inTauri ? invokeStrict<void>('set_global_hotkey', { accelerator }) : Promise.resolve(),

  // Last-poll wall-clock subscription (A12). Backend emits a single u64
  // millis-since-epoch on each successful container poll. Used by the
  // status bar to render "updated Ns ago".
  onTickAt: async (cb: (ms: number) => void): Promise<UnlistenFn> => {
    if (!inTauri) {
      // In dev mode, fake a tick every 2 s so the indicator still moves.
      const id = window.setInterval(() => cb(Date.now()), 2000);
      cb(Date.now());
      return () => window.clearInterval(id);
    }
    return listen<number>('containers:tickAt', e => cb(e.payload));
  },
};

/** Fresh id for one log stream (alphanumerics/dashes only: it ends up in an event name). */
export function newStreamId(): string {
  return typeof crypto !== 'undefined' && 'randomUUID' in crypto
    ? crypto.randomUUID()
    : `s${Date.now().toString(36)}${Math.random().toString(36).slice(2)}`;
}
