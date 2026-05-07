import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, screen, act } from '@testing-library/react';
import { toast, withToast, ToastTray } from './toast';
import { getTheme } from './theme';

describe('toast', () => {
  beforeEach(() => { vi.useFakeTimers(); });
  afterEach(() => { vi.useRealTimers(); });

  it('renders nothing when the queue is empty', () => {
    const { container } = render(<ToastTray t={getTheme(true)} />);
    expect(container.firstChild).toBeNull();
  });

  it('renders a toast and auto-dismisses after 4s', () => {
    render(<ToastTray t={getTheme(true)} />);
    act(() => { toast('start mycontainer failed'); });
    expect(screen.getByText('start mycontainer failed')).toBeInTheDocument();
    act(() => { vi.advanceTimersByTime(4001); });
    expect(screen.queryByText('start mycontainer failed')).not.toBeInTheDocument();
  });

  it('withToast surfaces a rejected promise via toast and re-throws', async () => {
    // Real timers here — fake-timer scheduling masks React's effect flushing
    // for the listener registered by ToastTray.
    vi.useRealTimers();
    render(<ToastTray t={getTheme(true)} />);
    const p = withToast('stop foo', Promise.reject(new Error('not running')));
    await expect(p).rejects.toThrow('not running');
    expect(await screen.findByText(/stop foo failed: not running/)).toBeInTheDocument();
  });
});
