// Tauri invoke wrapper. Falls back to in-process fixtures when running outside Tauri
// (e.g., `vite dev` in a regular browser tab) so the UI is still usable for design work.

import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import type {
  Container, Image, Volume, Network, Stack,
  TrivyResult, Update, DoctorCheck, HistoryPoint, VulnHistory, DiskUsage,
} from './types';
import * as fixtures from './fixtures';

const inTauri = typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;

export type Prefs = {
  dark: boolean;
  sidebarCollapsed: boolean;
  runtime: 'container' | 'docker' | 'podman';
  lastTab: string;
  menubarMode: boolean;
  globalHotkey: string;
  notifyOnExit: boolean;
};

const PREFS_DEFAULT: Prefs = {
  dark: true, sidebarCollapsed: false, runtime: 'container', lastTab: 'containers',
  menubarMode: false, globalHotkey: '', notifyOnExit: true,
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

// For commands where we want to surface errors to the user (actions like
// stop/delete), don't swallow.
async function invokeStrict<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  if (!inTauri) throw new Error('not running in Tauri');
  return invoke<T>(cmd, args);
}

export const api = {
  // Lists / inspect
  listContainers: () => call<Container[]>('list_containers', fixtures.containers),
  listImages:     () => call<Image[]>('list_images', fixtures.images),
  listVolumes:    () => call<Volume[]>('list_volumes', fixtures.volumes),
  listNetworks:   () => call<Network[]>('list_networks', fixtures.networks),
  listStacks:     () => call<Stack[]>('list_stacks', fixtures.stacks),
  inspectContainer: (id: string) => call<string>('inspect_container', fixtures.inspectJson, { id }),
  doctor:         () => call<DoctorCheck[]>('doctor', fixtures.doctor),
  scanImage:      (image: string) => call<TrivyResult>('scan_image', fixtures.trivy, { image }),
  listUpdates:    () => call<Update[]>('list_updates', fixtures.updates),

  // Container actions — strict; UI surfaces errors via console + (later) toasts.
  startContainer:   (id: string) => inTauri ? invokeStrict<void>('start_container',   { id }) : Promise.resolve(),
  stopContainer:    (id: string) => inTauri ? invokeStrict<void>('stop_container',    { id }) : Promise.resolve(),
  killContainer:    (id: string, signal?: string) => inTauri ? invokeStrict<void>('kill_container', { id, signal }) : Promise.resolve(),
  deleteContainer:  (id: string) => inTauri ? invokeStrict<void>('delete_container',  { id }) : Promise.resolve(),
  restartContainer: (id: string) => inTauri ? invokeStrict<void>('restart_container', { id }) : Promise.resolve(),

  // Streams: backend spawns a child process, frontend subscribes to events.
  startLogStream: (id: string, opts?: { boot?: boolean; tail?: number }) =>
    inTauri ? invokeStrict<void>('start_log_stream', { id, boot: opts?.boot, tail: opts?.tail }) : Promise.resolve(),
  startPull:      (reference: string) => inTauri ? invokeStrict<void>('start_pull', { reference }) : Promise.resolve(),

  // Deletes (strict — caller should confirm first and surface errors via toast).
  deleteImage:   (reference: string) => inTauri ? invokeStrict<void>('delete_image', { reference }) : Promise.resolve(),
  deleteVolume:  (name: string)      => inTauri ? invokeStrict<void>('delete_volume', { name }) : Promise.resolve(),
  deleteNetwork: (id: string)        => inTauri ? invokeStrict<void>('delete_network', { id }) : Promise.resolve(),
  execContainer: (id: string)        => inTauri ? invokeStrict<void>('exec_container', { id }) : Promise.resolve(),

  // Inspect: returns pretty JSON. In browser-dev mode hands back a small placeholder.
  inspectVolume:  (name: string)      => inTauri ? invokeStrict<string>('inspect_volume',  { name })      : Promise.resolve('{\n  "name": "' + name + '",\n  "driver": "local"\n}'),
  inspectNetwork: (id: string)        => inTauri ? invokeStrict<string>('inspect_network', { id })        : Promise.resolve('{\n  "id": "' + id + '"\n}'),
  inspectImage:   (reference: string) => inTauri ? invokeStrict<string>('inspect_image',   { reference }) : Promise.resolve('{\n  "reference": "' + reference + '",\n  "layers": []\n}'),

  // run image: returns the new container id on success.
  runImage: (args: { image: string; name?: string; ports?: string[]; env?: string[]; command?: string }) =>
    inTauri ? invokeStrict<string>('run_image', { args }) : Promise.resolve('dev-mode-no-op'),

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

  // Per-runtime availability probe (B8). Outside Tauri we report all
  // three as available so the Settings UI exercises every state.
  probeRuntime: (name: string) =>
    inTauri ? invokeStrict<boolean>('probe_runtime', { name }) :
              Promise.resolve(true),

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
  onLogLine: async (cb: (line: string) => void): Promise<UnlistenFn> => {
    if (!inTauri) {
      // Replay fixture lines on a timer so the LogsView still has movement.
      let i = 0;
      const id = window.setInterval(() => {
        if (i >= fixtures.logs.length) { window.clearInterval(id); return; }
        cb(fixtures.logs[i++]);
      }, 80);
      return () => window.clearInterval(id);
    }
    return listen<string>('logs:tick', e => cb(e.payload));
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
