import { describe, it, expect } from 'vitest';
import { api } from './api';
import * as fixtures from './fixtures';

// In jsdom there's no __TAURI_INTERNALS__, so every call() should fall back
// to fixtures. These tests prove the dev-mode safety net stays intact.

describe('api fixture fallback (outside Tauri)', () => {
  it('listContainers returns the fixture set', async () => {
    const cs = await api.listContainers();
    expect(cs).toEqual(fixtures.containers);
  });

  it('listImages, listVolumes, listNetworks all fall back', async () => {
    expect(await api.listImages()).toEqual(fixtures.images);
    expect(await api.listVolumes()).toEqual(fixtures.volumes);
    expect(await api.listNetworks()).toEqual(fixtures.networks);
  });

  it('inspectContainer returns the fixture json blob', async () => {
    const s = await api.inspectContainer('any');
    expect(s).toBe(fixtures.inspectJson);
  });

  it('strict commands resolve as no-ops in browser-dev mode', async () => {
    await expect(api.startContainer('x')).resolves.toBeUndefined();
    await expect(api.stopContainer('x')).resolves.toBeUndefined();
    await expect(api.restartContainer('x')).resolves.toBeUndefined();
    await expect(api.deleteImage('x')).resolves.toBeUndefined();
    await expect(api.execContainer('x')).resolves.toBeUndefined();
  });

  it('event subscriptions return a no-op unsubscribe fn outside Tauri', async () => {
    const unlisten = await api.onContainersTick(() => {});
    expect(typeof unlisten).toBe('function');
    expect(() => unlisten()).not.toThrow();
  });

  it('savePrefs is a no-op outside Tauri', async () => {
    await expect(api.savePrefs({
      dark: true, sidebarCollapsed: false, runtime: 'container', lastTab: 'containers',
      menubarMode: false, globalHotkey: '', notifyOnExit: true,
    })).resolves.toBeUndefined();
  });
});
