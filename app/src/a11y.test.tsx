import { describe, it, expect, vi, afterEach } from 'vitest';
import { useState } from 'react';
import { render, screen, fireEvent, act } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { DialogBackdrop } from './a11y';
import { ContainersView, ImagesView, VolumesView, NetworksView } from './views';
import { Sidebar, TopBar } from './components';
import { SettingsModal, DoctorModal, HelpModal, RunImageModal, DetailModal, TrivyModal } from './modals';
import { ToastTray, toast } from './toast';
import { containers } from './fixtures';
import { getTheme } from './theme';
import type { Tab } from './types';

const t = getTheme(true);

// Opener button + a dialog that mounts/unmounts, to exercise focus restore.
function Harness({ onClose }: { onClose?: () => void }) {
  const [open, setOpen] = useState(false);
  return (
    <div>
      <button onClick={() => setOpen(true)}>opener</button>
      {open && (
        <DialogBackdrop title="Example dialog" onClose={() => { onClose?.(); setOpen(false); }}>
          <div>
            <button>first</button>
            <input aria-label="field" />
            <button>last</button>
          </div>
        </DialogBackdrop>
      )}
    </div>
  );
}

describe('DialogBackdrop', () => {
  it('renders a modal dialog with an accessible name', () => {
    render(<DialogBackdrop title="Example dialog" onClose={() => {}}><button>x</button></DialogBackdrop>);
    const dlg = screen.getByRole('dialog', { name: 'Example dialog' });
    expect(dlg).toHaveAttribute('aria-modal', 'true');
  });

  it('supports aria-labelledby via labelId', () => {
    render(<DialogBackdrop labelId="h" onClose={() => {}}><h2 id="h">Heading text</h2></DialogBackdrop>);
    expect(screen.getByRole('dialog', { name: 'Heading text' })).toBeInTheDocument();
  });

  it('Escape closes it', async () => {
    const user = userEvent.setup();
    const onClose = vi.fn();
    render(<DialogBackdrop title="d" onClose={onClose}><button>x</button></DialogBackdrop>);
    await user.keyboard('{Escape}');
    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it('Escape does nothing when escapeCloses is false', async () => {
    const user = userEvent.setup();
    const onClose = vi.fn();
    render(<DialogBackdrop title="d" onClose={onClose} escapeCloses={false}><button>x</button></DialogBackdrop>);
    await user.keyboard('{Escape}');
    expect(onClose).not.toHaveBeenCalled();
  });

  it('only the topmost of stacked dialogs handles Escape', async () => {
    const user = userEvent.setup();
    const outer = vi.fn();
    const inner = vi.fn();
    render(
      <>
        <DialogBackdrop title="outer" onClose={outer}><button>a</button></DialogBackdrop>
        <DialogBackdrop title="inner" onClose={inner}><button>b</button></DialogBackdrop>
      </>,
    );
    await user.keyboard('{Escape}');
    expect(inner).toHaveBeenCalledTimes(1);
    expect(outer).not.toHaveBeenCalled();
  });

  it('moves focus into the dialog on open and restores it on close', async () => {
    const user = userEvent.setup();
    render(<Harness />);
    const opener = screen.getByRole('button', { name: 'opener' });
    await user.click(opener);
    const dlg = screen.getByRole('dialog', { name: 'Example dialog' });
    expect(dlg.contains(document.activeElement)).toBe(true);
    await user.keyboard('{Escape}');
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
    expect(opener).toHaveFocus();
  });

  it('keeps focus on an autoFocus child instead of stealing it', () => {
    render(<DialogBackdrop title="d" onClose={() => {}}><input aria-label="auto" autoFocus /></DialogBackdrop>);
    expect(screen.getByLabelText('auto')).toHaveFocus();
  });

  it('traps Tab and Shift+Tab inside the dialog', async () => {
    const user = userEvent.setup();
    render(<Harness />);
    await user.click(screen.getByRole('button', { name: 'opener' }));
    const first = screen.getByRole('button', { name: 'first' });
    const last = screen.getByRole('button', { name: 'last' });
    last.focus();
    await user.tab();
    expect(first).toHaveFocus();
    await user.tab({ shift: true });
    expect(last).toHaveFocus();
    // Focus that escaped (e.g. clicked outside) is pulled back in.
    screen.getByRole('button', { name: 'opener' }).focus();
    await user.tab();
    expect(first).toHaveFocus();
  });

  it('closes on a click that starts and ends on the backdrop', () => {
    const onClose = vi.fn();
    render(<DialogBackdrop title="d" onClose={onClose}><button>x</button></DialogBackdrop>);
    const backdrop = screen.getByRole('dialog').parentElement!;
    fireEvent.mouseDown(backdrop);
    fireEvent.mouseUp(backdrop);
    fireEvent.click(backdrop);
    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it('does not close when a drag starts inside the dialog and ends on the backdrop', () => {
    const onClose = vi.fn();
    render(<DialogBackdrop title="d" onClose={onClose}><button>x</button></DialogBackdrop>);
    const backdrop = screen.getByRole('dialog').parentElement!;
    fireEvent.mouseDown(screen.getByRole('button', { name: 'x' }));
    fireEvent.mouseUp(backdrop);
    // Browsers dispatch the click on the common ancestor, i.e. the backdrop.
    fireEvent.click(backdrop);
    expect(onClose).not.toHaveBeenCalled();
  });

  it('does not close on a click inside the dialog', async () => {
    const user = userEvent.setup();
    const onClose = vi.fn();
    render(<DialogBackdrop title="d" onClose={onClose}><button>x</button></DialogBackdrop>);
    await user.click(screen.getByRole('button', { name: 'x' }));
    expect(onClose).not.toHaveBeenCalled();
  });
});

describe('real modals expose dialogs', () => {
  it.each([
    ['Doctor', <DoctorModal key="d" t={t} onClose={() => {}} />, 'cgui doctor'],
    ['Help', <HelpModal key="h" t={t} onClose={() => {}} />, 'Keyboard shortcuts'],
    ['Run image', <RunImageModal key="r" t={t} image="alpine" onLaunched={() => {}} onClose={() => {}} />, 'Run image'],
  ])('%s modal is a labelled dialog', (_n, el, name) => {
    render(el);
    expect(screen.getByRole('dialog', { name })).toBeInTheDocument();
  });

  it('modal close buttons are named', () => {
    render(<HelpModal t={t} onClose={() => {}} />);
    expect(screen.getByRole('button', { name: 'Close' })).toBeInTheDocument();
  });
});

describe('ContainersView keyboard + labels', () => {
  const noop = () => {};
  function renderView(extra: Partial<React.ComponentProps<typeof ContainersView>> = {}) {
    return render(
      <ContainersView t={t} search="" selected="" setSelected={noop} onInspect={noop} onLogs={noop} containers={containers} {...extra} />,
    );
  }

  it('row action icon buttons are findable by name', () => {
    renderView();
    expect(screen.getAllByRole('button', { name: /^inspect$/i }).length).toBeGreaterThan(0);
    expect(screen.getAllByRole('button', { name: /^logs$/i }).length).toBeGreaterThan(0);
    expect(screen.getAllByRole('button', { name: /^stop$/i }).length).toBeGreaterThan(0);
    expect(screen.getAllByRole('button', { name: /^start$/i }).length).toBeGreaterThan(0);
  });

  it('rows are focusable groups; Enter opens, Space selects', async () => {
    const user = userEvent.setup();
    const setSelected = vi.fn();
    const onInspect = vi.fn();
    renderView({ setSelected, onInspect });
    const c = containers[0];
    const row = screen.getByRole('group', { name: `Container ${c.name}` });
    expect(row).toHaveAttribute('tabindex', '0');
    row.focus();
    await user.keyboard(' ');
    expect(setSelected).toHaveBeenCalledWith(c.id);
    await user.keyboard('{Enter}');
    expect(onInspect).toHaveBeenCalledWith(c);
  });

  it('status is available as text, not colour alone', () => {
    renderView();
    expect(screen.getAllByText(/^running$/i).length).toBeGreaterThan(0);
  });
});

describe('Settings toggles', () => {
  it('Dark mode, Menu-bar mode and Notify on exit are labelled switches', () => {
    render(
      <SettingsModal
        t={t} onClose={() => {}} runtime="container" setRuntime={() => {}}
        dark setDark={() => {}}
        menubarMode={false} setMenubarMode={() => {}}
        globalHotkey="" setGlobalHotkey={() => {}}
        notifyOnExit={false} setNotifyOnExit={() => {}}
      />,
    );
    expect(screen.getByRole('switch', { name: /dark mode/i })).toBeChecked();
    expect(screen.getByRole('switch', { name: /menu-bar mode/i })).not.toBeChecked();
    expect(screen.getByRole('switch', { name: /notify on container exit/i })).not.toBeChecked();
  });
});

describe('toasts are announced', () => {
  afterEach(() => { vi.useRealTimers(); });
  it('info toasts use role=status, errors role=alert', () => {
    vi.useFakeTimers();
    render(<ToastTray t={t} />);
    act(() => { toast('all good', 'info'); toast('it broke', 'error'); });
    expect(screen.getByRole('status')).toHaveTextContent('all good');
    expect(screen.getByRole('alert')).toHaveTextContent('it broke');
    act(() => { vi.advanceTimersByTime(4001); });
  });
});

function unnamed(root: HTMLElement) {
  return Array.from(root.querySelectorAll('button')).filter(b => {
    const name = (b.getAttribute('aria-label') || b.getAttribute('title') || b.textContent || '').trim();
    return !name && !b.getAttribute('aria-labelledby');
  });
}

describe('every button has an accessible name', () => {
  it('detail, trivy, help and run-image modals', async () => {
    const { container } = render(
      <>
        <DetailModal item={containers[0]} t={t} onClose={() => {}} onExec={() => {}} />
        <TrivyModal t={t} image="alpine" onClose={() => {}} />
        <HelpModal t={t} onClose={() => {}} />
        <RunImageModal t={t} image="alpine" onLaunched={() => {}} onClose={() => {}} />
      </>,
    );
    await screen.findByText(/CVE-2024-21626/);
    expect(container.querySelectorAll('button').length).toBeGreaterThan(10);
    expect(unnamed(container)).toEqual([]);
  });

  it('chrome (sidebar collapsed + topbar)', () => {
    const headings = {} as Record<Tab, { title: string; sub: string }>;
    for (const k of ['containers', 'images', 'volumes', 'networks', 'stacks', 'logs'] as Tab[]) headings[k] = { title: k, sub: '' };
    const { container } = render(
      <>
        <Sidebar tab="containers" setTab={() => {}} collapsed t={t} onSettings={() => {}} onDoctor={() => {}} runningCount={1} stacksCount={1} />
        <TopBar tab="images" t={t} search="" setSearch={() => {}} onPull={() => {}} onCollapse={() => {}}
          onPrune={() => {}} onImport={() => {}} onBuild={() => {}} runtime="container" dark setDark={() => {}}
          onUpdate={() => {}} updateCount={2} headings={headings} />
      </>,
    );
    expect(unnamed(container)).toEqual([]);
  });

  it('containers / images / volumes / networks views', async () => {
    const noop = () => {};
    const { container } = render(
      <>
        <ContainersView t={t} search="" selected="" setSelected={noop} onInspect={noop} onLogs={noop} containers={containers} />
        <ImagesView t={t} search="" reloadKey={0} onScan={noop} onRun={noop} onInspect={noop} />
        <VolumesView t={t} search="" reloadKey={0} onInspect={noop} />
        <NetworksView t={t} search="" reloadKey={0} onInspect={noop} />
      </>,
    );
    await screen.findAllByText(/mlcommons|alpine|inference/i);
    expect(unnamed(container)).toEqual([]);
  });

  it('settings and doctor modals', async () => {
    const { container } = render(
      <>
        <SettingsModal
          t={t} onClose={() => {}} runtime="container" setRuntime={() => {}}
          dark setDark={() => {}} menubarMode setMenubarMode={() => {}}
          globalHotkey="" setGlobalHotkey={() => {}} notifyOnExit setNotifyOnExit={() => {}}
        />
        <DoctorModal t={t} onClose={() => {}} />
      </>,
    );
    await screen.findByText(/passed/);
    expect(unnamed(container)).toEqual([]);
  });
});
