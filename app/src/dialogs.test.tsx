// Regression tests for the "confirmations silently auto-accepted" incident:
// in the packaged app window.confirm resolved to an always-truthy Promise, so
// destructive actions and the self-updater ran without consent. All call
// sites now go through ./dialogs; these tests pin the declined paths.

import { describe, it, expect, vi, afterEach, beforeEach } from 'vitest';
import { render, screen, waitFor, act } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { getTheme } from './theme';
import { api } from './api';
import * as fixtures from './fixtures';

vi.mock('./dialogs', async () => {
  const actual = await vi.importActual<typeof import('./dialogs')>('./dialogs');
  return { ...actual, askConfirm: vi.fn() };
});
const nativeConfirm = vi.fn();
vi.mock('@tauri-apps/plugin-dialog', () => ({ confirm: (...a: unknown[]) => nativeConfirm(...a) }));
const listeners = new Map<string, () => unknown>();
vi.mock('@tauri-apps/api/event', () => ({
  listen: async (name: string, cb: () => unknown) => { listeners.set(name, cb); return () => {}; },
}));
const updaterCheck = vi.fn();
vi.mock('@tauri-apps/plugin-updater', () => ({ check: () => updaterCheck() }));
const relaunch = vi.fn();
vi.mock('@tauri-apps/plugin-process', () => ({ relaunch: () => relaunch() }));

import * as dialogs from './dialogs';
import { ContainersView, VolumesView } from './views';
import { TextPromptHost } from './TextPromptHost';
import App from './App';

const t = getTheme(true);
const askConfirmMock = vi.mocked(dialogs.askConfirm);
const realDialogs = await vi.importActual<typeof import('./dialogs')>('./dialogs');

beforeEach(() => { askConfirmMock.mockReset(); nativeConfirm.mockReset(); });
afterEach(() => { vi.restoreAllMocks(); });

describe('askConfirm (real implementation)', () => {
  it('outside Tauri falls back to window.confirm', async () => {
    const spy = vi.spyOn(window, 'confirm').mockReturnValue(true);
    expect(await realDialogs.askConfirm('sure?')).toBe(true);
    spy.mockReturnValue(false);
    expect(await realDialogs.askConfirm('sure?')).toBe(false);
  });

  describe('inside Tauri', () => {
    beforeEach(() => { (window as any).__TAURI_INTERNALS__ = {}; });
    afterEach(() => { delete (window as any).__TAURI_INTERNALS__; });

    it('uses the native dialog with custom labels and awaits the answer', async () => {
      const winConfirm = vi.spyOn(window, 'confirm');
      nativeConfirm.mockResolvedValue(false);
      expect(await realDialogs.askConfirm('Delete?', { title: 'T', okLabel: 'Delete', cancelLabel: 'Keep' })).toBe(false);
      expect(nativeConfirm).toHaveBeenCalledWith('Delete?', expect.objectContaining({ title: 'T', okLabel: 'Delete', cancelLabel: 'Keep', kind: 'warning' }));
      nativeConfirm.mockResolvedValue(true);
      expect(await realDialogs.askConfirm('Delete?')).toBe(true);
      expect(winConfirm).not.toHaveBeenCalled();
    });

    it('fails closed when the native dialog throws', async () => {
      nativeConfirm.mockRejectedValue(new Error('not allowed'));
      expect(await realDialogs.askConfirm('Delete?')).toBe(false);
    });
  });
});

describe('askText', () => {
  it('falls back to window.prompt when no host is mounted', async () => {
    const spy = vi.spyOn(window, 'prompt').mockReturnValue('x');
    expect(await realDialogs.askText('path?', 'd')).toBe('x');
    expect(spy).toHaveBeenCalledWith('path?', 'd');
  });

  it('returns null on Cancel and on Escape, the typed value on Enter', async () => {
    const user = userEvent.setup();
    render(<TextPromptHost t={t} />);

    let p = realDialogs.askText('Where?', '/tmp/');
    const input = await screen.findByRole('textbox');
    expect(input).toHaveFocus();
    expect(input).toHaveValue('/tmp/');
    await user.click(screen.getByRole('button', { name: 'Cancel' }));
    expect(await p).toBeNull();
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument();

    p = realDialogs.askText('Where?', '/tmp/');
    await screen.findByRole('dialog');
    await user.keyboard('{Escape}');
    expect(await p).toBeNull();

    p = realDialogs.askText('Where?', '/tmp/');
    await screen.findByRole('dialog');
    await user.clear(screen.getByRole('textbox'));
    await user.type(screen.getByRole('textbox'), '/etc{Enter}');
    expect(await p).toBe('/etc');
  });

  it('queues concurrent requests', async () => {
    const user = userEvent.setup();
    render(<TextPromptHost t={t} />);
    const a = realDialogs.askText('first', '1');
    const b = realDialogs.askText('second', '2');
    await screen.findByText('first');
    await user.keyboard('{Enter}');
    expect(await a).toBe('1');
    await screen.findByText('second');
    await user.keyboard('{Enter}');
    expect(await b).toBe('2');
  });
});

describe('declined confirmations do nothing', () => {
  it('bulk delete of containers', async () => {
    const user = userEvent.setup();
    askConfirmMock.mockResolvedValue(false);
    const del = vi.spyOn(api, 'deleteContainer').mockResolvedValue(undefined as never);
    render(<ContainersView t={t} search="" selected="" setSelected={() => {}} onInspect={() => {}} onLogs={() => {}} containers={fixtures.containers} />);
    await user.click(await screen.findByTitle('Select all visible'));
    await user.click(screen.getByRole('button', { name: /Delete/ }));
    await waitFor(() => expect(askConfirmMock).toHaveBeenCalledTimes(1));
    expect(askConfirmMock.mock.calls[0][0]).toMatch(/^Delete \d+ container/);
    expect(del).not.toHaveBeenCalled();
  });

  it('delete volume declined vs accepted', async () => {
    const user = userEvent.setup();
    vi.spyOn(api, 'listVolumes').mockResolvedValue(fixtures.volumes);
    const del = vi.spyOn(api, 'deleteVolume').mockResolvedValue(undefined as never);
    render(<VolumesView t={t} search="" onInspect={() => {}} />);
    const btns = await screen.findAllByTitle('Delete volume');

    askConfirmMock.mockResolvedValue(false);
    await user.click(btns[0]);
    await waitFor(() => expect(askConfirmMock).toHaveBeenCalledTimes(1));
    expect(del).not.toHaveBeenCalled();

    askConfirmMock.mockResolvedValue(true);
    await user.click(btns[0]);
    await waitFor(() => expect(del).toHaveBeenCalledWith(fixtures.volumes[0].name));
  });
});

describe('self-updater consent', () => {
  beforeEach(() => {
    (window as any).__TAURI_INTERNALS__ = {};
    listeners.clear();
    updaterCheck.mockReset();
    relaunch.mockReset();
  });
  afterEach(() => { delete (window as any).__TAURI_INTERNALS__; });

  async function fire() {
    render(<App />);
    await waitFor(() => expect(listeners.has('menu:check-updates')).toBe(true));
    await act(async () => { await listeners.get('menu:check-updates')!(); });
  }

  it('does not download or relaunch when the user declines', async () => {
    const downloadAndInstall = vi.fn();
    updaterCheck.mockResolvedValue({ version: '9.9.9', currentVersion: '0.5.9', body: 'notes', downloadAndInstall });
    askConfirmMock.mockResolvedValue(false);
    await fire();
    expect(askConfirmMock).toHaveBeenCalledTimes(1);
    expect(askConfirmMock.mock.calls[0][0]).toContain('9.9.9');
    expect(askConfirmMock.mock.calls[0][0]).toContain('you have 0.5.9');
    expect(askConfirmMock.mock.calls[0][0]).toContain('notes');
    expect(askConfirmMock.mock.calls[0][1]).toMatchObject({ okLabel: 'Install and Relaunch', cancelLabel: 'Later' });
    expect(downloadAndInstall).not.toHaveBeenCalled();
    expect(relaunch).not.toHaveBeenCalled();
  });

  it('downloads and relaunches only after acceptance', async () => {
    const downloadAndInstall = vi.fn().mockResolvedValue(undefined);
    updaterCheck.mockResolvedValue({ version: '9.9.9', body: '', downloadAndInstall });
    askConfirmMock.mockResolvedValue(true);
    await fire();
    expect(downloadAndInstall).toHaveBeenCalledTimes(1);
    expect(relaunch).toHaveBeenCalledTimes(1);
  });
});
