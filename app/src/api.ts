// Tauri invoke wrapper. Falls back to in-process fixtures when running outside Tauri
// (e.g., `vite dev` in a regular browser tab) so the UI is still usable for design work.

import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import type {
  Container, Image, Volume, Network, Stack,
  TrivyResult, Update, DoctorCheck,
} from './types';
import * as fixtures from './fixtures';

const inTauri = typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;

export type Prefs = {
  dark: boolean;
  sidebarCollapsed: boolean;
  runtime: 'container' | 'docker' | 'podman';
  lastTab: string;
};

const PREFS_DEFAULT: Prefs = {
  dark: true, sidebarCollapsed: false, runtime: 'container', lastTab: 'containers',
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
  killContainer:    (id: string) => inTauri ? invokeStrict<void>('kill_container',    { id }) : Promise.resolve(),
  deleteContainer:  (id: string) => inTauri ? invokeStrict<void>('delete_container',  { id }) : Promise.resolve(),
  restartContainer: (id: string) => inTauri ? invokeStrict<void>('restart_container', { id }) : Promise.resolve(),

  // Streams: backend spawns a child process, frontend subscribes to events.
  startLogStream: (id: string) => inTauri ? invokeStrict<void>('start_log_stream', { id }) : Promise.resolve(),
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

  // First-run onboarding probe. Outside Tauri (browser dev mode) the
  // fallback returns true so the OnboardingModal doesn't trigger when the
  // entire data layer is fixtures by design.
  runtimeAvailable: () => call<boolean>('runtime_available', true),

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

  // Prefs persistence. Outside Tauri, hand back defaults / no-op save.
  loadPrefs: () => call<Prefs>('load_prefs', PREFS_DEFAULT),
  savePrefs: (p: Prefs) => inTauri ? invokeStrict<void>('save_prefs', { prefs: p }).catch(e => console.warn('save_prefs failed', e)) : Promise.resolve(),
};
