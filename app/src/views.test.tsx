import { describe, it, expect, vi, afterEach } from 'vitest';
import { render, screen, waitFor, act } from '@testing-library/react';
import { stripAnsi, LogsView } from './views';
import { api } from './api';
import { getTheme } from './theme';

describe('stripAnsi', () => {
  it('removes simple SGR colour codes', () => {
    // bold + green hello + reset
    expect(stripAnsi('[1;32mhello[0m')).toBe('hello');
  });

  it('removes cursor-movement and erase-line sequences', () => {
    // CSI 2K = erase line, CSI G = move cursor to column 1
    expect(stripAnsi('foo[2K[Gbar')).toBe('foobar');
  });

  it('removes OSC sequences (terminal title etc.)', () => {
    // ESC ] 0 ; title BEL — common terminal-title set sequence
    expect(stripAnsi('keep]0;some titleme')).toBe('keepme');
  });

  it('passes through plain text untouched', () => {
    expect(stripAnsi('plain log line 42')).toBe('plain log line 42');
  });
});

describe('LogsView stream lifecycle', () => {
  const t = getTheme(true);

  function setup() {
    const handlers = new Map<string, (line: string) => void>();
    const started: string[] = [];
    const stopped: string[] = [];
    vi.spyOn(api, 'loadLogs').mockResolvedValue([]);
    vi.spyOn(api, 'onLogLine').mockImplementation(async (streamId, cb) => {
      handlers.set(streamId, cb);
      return () => { handlers.delete(streamId); };
    });
    vi.spyOn(api, 'startLogStream').mockImplementation(async (streamId) => { started.push(streamId); });
    vi.spyOn(api, 'stopLogStream').mockImplementation(async (streamId) => { stopped.push(streamId); });
    return { handlers, started, stopped };
  }

  afterEach(() => { vi.restoreAllMocks(); });

  it('stops the previous stream when the target container changes', async () => {
    const { started, stopped } = setup();
    const { rerender } = render(<LogsView t={t} target="a" />);
    await waitFor(() => expect(started).toHaveLength(1));
    rerender(<LogsView t={t} target="b" />);
    await waitFor(() => expect(started).toHaveLength(2));
    await waitFor(() => expect(stopped).toEqual([started[0]]));
    expect(started[0]).not.toBe(started[1]);
  });

  it('stops its stream on unmount', async () => {
    const { started, stopped } = setup();
    const { unmount } = render(<LogsView t={t} target="a" />);
    await waitFor(() => expect(started).toHaveLength(1));
    unmount();
    await waitFor(() => expect(stopped).toEqual([started[0]]));
  });

  it('only renders lines delivered on its own stream id', async () => {
    const { handlers, started } = setup();
    const { rerender } = render(<LogsView t={t} target="a" />);
    await waitFor(() => expect(started).toHaveLength(1));
    const oldCb = handlers.get(started[0])!;
    rerender(<LogsView t={t} target="b" />);
    await waitFor(() => expect(started).toHaveLength(2));
    act(() => { oldCb('stale line from A'); });
    act(() => { handlers.get(started[1])?.('fresh line from B'); });
    expect(await screen.findByText('fresh line from B')).toBeInTheDocument();
    expect(screen.queryByText('stale line from A')).not.toBeInTheDocument();
  });

  it('does not seed persisted history when the live stream starts (no duplicate lines)', async () => {
    const { handlers, started } = setup();
    vi.mocked(api.loadLogs).mockResolvedValue([{ ts: 1, line: 'persisted line' }]);
    render(<LogsView t={t} target="a" />);
    await waitFor(() => expect(started).toHaveLength(1));
    act(() => { handlers.get(started[0])?.('live line'); });
    expect(await screen.findByText('live line')).toBeInTheDocument();
    expect(api.loadLogs).not.toHaveBeenCalled();
    expect(screen.queryByText('persisted line')).not.toBeInTheDocument();
  });

  it('falls back to persisted history when the live stream cannot start', async () => {
    setup();
    vi.mocked(api.loadLogs).mockResolvedValue([{ ts: 1, line: 'persisted line' }]);
    vi.mocked(api.startLogStream).mockRejectedValue('container with ID a not found');
    vi.spyOn(console, 'warn').mockImplementation(() => {});
    render(<LogsView t={t} target="a" />);
    expect(await screen.findByText('persisted line')).toBeInTheDocument();
  });
});
