import { lazy, Suspense, useEffect, useMemo, useRef, useState } from 'react';
// macOS-only app — we ship native traffic lights via Tauri's default
// `decorations: true`. Frameless chrome and platform detection were removed
// when the project's scope was locked to Apple's container runtime.
import { getTheme } from './theme';
import type { Tab, Modal, Runtime, Container, Image, Stack } from './types';
import { api, type Prefs } from './api';
import { FramelessChrome, Sidebar, TopBar, StatusBar } from './components';
import { ContainersView, ImagesView, VolumesView, NetworksView, StacksView, LogsView } from './views';
import { ToastTray } from './toast';

// All modal code is split into a separate chunk and only fetched on first
// modal open. Saves ~30 KB from the initial bundle without affecting
// perceived modal-open latency (modals already do an `api.invoke()` before
// rendering content).
const ModalsHost = lazy(() => import('./ModalsHost'));

export default function App() {
  const [dark, setDark] = useState(true);
  const [collapsed, setCollapsed] = useState(false);
  const [runtime, setRuntime] = useState<Runtime>('container');
  const [tab, setTab] = useState<Tab>('containers');
  const [search, setSearch] = useState('');
  const [selected, setSelected] = useState<string>('');
  const [modal, setModal] = useState<Modal>(null);
  // Companion-update badge state. `count` is the actual number returned
  // from the backend's listUpdates probe (not a hardcoded literal); the
  // badge stays clickable for the lifetime of the session so the user
  // can re-open the modal. `seen` flips after first dismiss so the
  // colour mutes from warning-orange to a subtle muted frame — keeps
  // the affordance visible without the constant glare. State resets on
  // app restart by design — fresh poll, fresh attention.
  const [updateCount, setUpdateCount] = useState(0);
  const [updatesSeen, setUpdatesSeen] = useState(false);
  const [logTarget, setLogTarget] = useState<string | undefined>(undefined);
  const [menubarMode, setMenubarMode] = useState(false);
  const [globalHotkey, setGlobalHotkey] = useState('');
  const [notifyOnExit, setNotifyOnExit] = useState(true);
  // ms-since-epoch of the last successful container poll (A12). 0 = never.
  const [lastTickAt, setLastTickAt] = useState(0);
  // Re-render driver for the "Ns ago" label so it counts up between ticks.
  const [, setNowTick] = useState(0);
  const prefsLoaded = useRef(false);
  // First-run onboarding: when the `container` CLI isn't on PATH we
  // surface a modal explaining what's happening and how to install it.
  // `onboardingDismissed` makes the dismissal sticky for this session
  // so dismiss → close doesn't immediately re-open the modal next tick.
  const [onboardingDismissed, setOnboardingDismissed] = useState(false);

  const t = useMemo(() => getTheme(dark), [dark]);

  // Load persisted prefs once on mount.
  useEffect(() => {
    api.loadPrefs().then(p => {
      setDark(p.dark);
      setCollapsed(p.sidebarCollapsed);
      setRuntime(p.runtime);
      if (['containers','images','volumes','networks','stacks','logs'].includes(p.lastTab)) {
        setTab(p.lastTab as Tab);
      }
      setMenubarMode(p.menubarMode);
      setGlobalHotkey(p.globalHotkey);
      setNotifyOnExit(p.notifyOnExit);
      prefsLoaded.current = true;
    });
  }, []);

  // Subscribe to the backend's per-tick wall-clock so the StatusBar can
  // render "updated Ns ago". Re-render every second to advance the label.
  useEffect(() => {
    let unlisten: (() => void) | null = null;
    api.onTickAt(ms => setLastTickAt(ms)).then(fn => { unlisten = fn; });
    const id = window.setInterval(() => setNowTick(n => n + 1), 1000);
    return () => { unlisten?.(); window.clearInterval(id); };
  }, []);

  // First-run runtime probe. Runs once on mount; if the `container` CLI
  // isn't installed and the user hasn't dismissed the modal this session,
  // open it. The 5s re-poll auto-closes the modal once they install +
  // we detect the CLI on a subsequent check, sparing them a relaunch.
  // Dev-only override: append `?onboarding=1` to force the modal even
  // when the CLI is detected, so the visual can be QA'd without
  // uninstalling `container`.
  useEffect(() => {
    let cancelled = false;
    const forceOnboarding =
      typeof location !== 'undefined' &&
      new URLSearchParams(location.search).get('onboarding') === '1';
    const check = async () => {
      const ok = forceOnboarding ? false : await api.runtimeAvailable();
      if (cancelled) return;
      if (ok) {
        setModal(m => (m?.type === 'onboarding' ? null : m));
      } else if (!onboardingDismissed) {
        setModal(m => (m ? m : { type: 'onboarding' }));
      }
    };
    check();
    const id = window.setInterval(check, 5000);
    return () => { cancelled = true; window.clearInterval(id); };
  }, [onboardingDismissed]);

  // Persist on change (skip the first render before load completes).
  useEffect(() => {
    if (!prefsLoaded.current) return;
    const p: Prefs = {
      dark, sidebarCollapsed: collapsed, runtime, lastTab: tab,
      menubarMode, globalHotkey, notifyOnExit,
    };
    api.savePrefs(p);
  }, [dark, collapsed, runtime, tab, menubarMode, globalHotkey, notifyOnExit]);

  // Push hotkey changes to the backend so they take effect immediately
  // without a restart. Surfaces parser/conflict errors via toast.
  useEffect(() => {
    if (!prefsLoaded.current) return;
    api.setGlobalHotkey(globalHotkey).catch(err => {
      // Lazy-import so the toast helper doesn't add to the boot path.
      import('./toast').then(({ toast }) => {
        toast(`Global hotkey: ${typeof err === 'string' ? err : (err?.message ?? String(err))}`);
      });
    });
  }, [globalHotkey]);

  // Slow background fetches so the Cmd-K palette has something to match
  // against without the user opening the corresponding tab first. Refreshes
  // on a 30 s timer + whenever the palette is opened. Same loop also
  // refreshes the topbar update-badge count so newly-published companion
  // releases surface without a relaunch.
  const [paletteImages, setPaletteImages] = useState<Image[]>([]);
  const [paletteStacks, setPaletteStacks] = useState<Stack[]>([]);
  useEffect(() => {
    let cancelled = false;
    const refresh = () => {
      api.listImages().then(v => { if (!cancelled) setPaletteImages(v); });
      api.listStacks().then(v => { if (!cancelled) setPaletteStacks(v); });
      api.listUpdates().then(v => { if (!cancelled) setUpdateCount(v.length); });
    };
    refresh();
    if (modal?.type === 'commandPalette') refresh();
    const id = window.setInterval(refresh, 30_000);
    return () => { cancelled = true; window.clearInterval(id); };
  }, [modal?.type]);

  const [containers, setContainers] = useState<Container[]>([]);
  useEffect(() => {
    let cancelled = false;
    let unlisten: (() => void) | null = null;
    api.listContainers().then(cs => {
      if (cancelled) return;
      setContainers(cs);
      if (cs.length) setSelected(prev => prev || cs[0].id);
    });
    api.onContainersTick(cs => { if (!cancelled) setContainers(cs); }).then(fn => {
      if (cancelled) fn(); else unlisten = fn;
    });
    return () => { cancelled = true; unlisten?.(); };
  }, []);

  useEffect(() => {
    // Tabs in display order — kept here so ⌘1..⌘6 mirrors the Sidebar.
    const tabsOrder: Tab[] = ['containers', 'images', 'volumes', 'networks', 'stacks', 'logs'];
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') setModal(null);
      const inInput = (document.activeElement as HTMLElement | null)?.tagName === 'INPUT';
      // Cmd/Ctrl-K → command palette. Suppress when typing in inputs so
      // search fields can still use ⌘K for word delete on macOS — wait,
      // ⌘K is a noop in inputs by default, safe to intercept globally.
      if ((e.metaKey || e.ctrlKey) && (e.key === 'k' || e.key === 'K')) {
        e.preventDefault();
        setModal(m => m?.type === 'commandPalette' ? null : { type: 'commandPalette' });
        return;
      }
      // Cmd/Ctrl-1..6 → switch tabs in Sidebar order.
      if ((e.metaKey || e.ctrlKey) && /^[1-6]$/.test(e.key)) {
        e.preventDefault();
        const idx = Number(e.key) - 1;
        if (tabsOrder[idx]) setTab(tabsOrder[idx]);
        return;
      }
      if (e.key === '/' && !modal && !inInput) {
        e.preventDefault();
        document.querySelector<HTMLInputElement>('input[placeholder^="Filter"]')?.focus();
      }
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [modal]);

  const desktopBg = dark
    ? 'radial-gradient(ellipse at 30% 20%, #2A2D3A 0%, #14161D 60%, #0A0B10 100%)'
    : 'radial-gradient(ellipse at 30% 20%, #DDE2EC 0%, #C5CCDB 60%, #B0B8CB 100%)';

  const runningCount = containers.filter(c => c.status === 'running').length;

  const headings: Record<Tab, { title: string; sub: string }> = {
    containers: { title: 'Containers', sub: `${runningCount} running · ${containers.length} total` },
    images:     { title: 'Images',     sub: 'image catalogue' },
    volumes:    { title: 'Volumes',    sub: 'persistent storage' },
    networks:   { title: 'Networks',   sub: 'bridge / host networks' },
    stacks:     { title: 'Stacks',     sub: 'compose-style groupings' },
    logs:       { title: 'Logs',       sub: logTarget ? `${logTarget} · live` : 'all sources · live' },
  };

  return (
    <div style={{
      width: '100vw', height: '100vh', overflow: 'hidden', background: desktopBg,
      display: 'flex', alignItems: 'center', justifyContent: 'center', padding: 24,
      fontFamily: '"Inter", system-ui, sans-serif',
      color: t.fg1,
    }}>
      <style>{`
        @keyframes pulse { 0%,100%{opacity:1} 50%{opacity:.4} }
        ::-webkit-scrollbar { width: 10px; height: 10px; }
        ::-webkit-scrollbar-track { background: transparent; }
        ::-webkit-scrollbar-thumb { background: ${t.border}; border-radius: 999px; }
        ::-webkit-scrollbar-thumb:hover { background: ${t.borderStrong}; }
        input::placeholder { color: ${t.fg3}; }
      `}</style>

      <div style={{ width: '100%', height: '100%', maxWidth: 1480, maxHeight: 920, position: 'relative' }}>
        <FramelessChrome t={t}>
          <div style={{ flex: 1, display: 'flex', minHeight: 0 }}>
            <Sidebar
              tab={tab} setTab={setTab} collapsed={collapsed} t={t}
              onSettings={() => setModal({ type: 'settings' })}
              onDoctor={() => setModal({ type: 'doctor' })}
              runningCount={runningCount}
              stacksCount={4}
            />
            <div style={{ flex: 1, display: 'flex', flexDirection: 'column', minWidth: 0 }}>
              <TopBar
                tab={tab} t={t}
                search={search} setSearch={setSearch}
                onPull={() => setModal({ type: 'pull' })}
                onCollapse={() => setCollapsed(!collapsed)}
                runtime={runtime}
                dark={dark}
                setDark={setDark}
                onUpdate={updateCount > 0 ? () => setModal({ type: 'update' }) : null}
                updateCount={updateCount}
                updatesSeen={updatesSeen}
                headings={headings}
              />
              {tab === 'containers' && (
                <ContainersView
                  t={t} search={search}
                  selected={selected} setSelected={setSelected}
                  onInspect={(c: Container) => setModal({ type: 'detail', payload: c })}
                  onLogs={(c: Container) => { setLogTarget(c.id); setTab('logs'); }}
                  containers={containers}
                />
              )}
              {tab === 'images'   && <ImagesView   t={t} search={search}
                                       onScan={(img: Image) => setModal({ type: 'trivy', image: img.ref })}
                                       onRun={(img: Image) => setModal({ type: 'runImage', image: img.ref })}
                                       onInspect={(img: Image) => setModal({ type: 'imageInspect', reference: img.ref })} />}
              {tab === 'volumes'  && <VolumesView  t={t} search={search} onInspect={(v) => setModal({ type: 'volumeInspect', name: v.name })} />}
              {tab === 'networks' && <NetworksView t={t} search={search} onInspect={(n) => setModal({ type: 'networkInspect', id: n.id, name: n.name })} />}
              {tab === 'stacks'   && <StacksView   t={t} search={search} onGraph={s => setModal({ type: 'stackGraph', stack: s })} />}
              {tab === 'logs'     && <LogsView     t={t} target={logTarget ?? containers.find(c => c.status === 'running')?.id} />}
              <StatusBar t={t} runtime={runtime} tab={tab} lastTickAt={lastTickAt} />
            </div>
          </div>
        </FramelessChrome>

        {modal && (
          <Suspense fallback={null}>
            <ModalsHost
              modal={modal}
              t={t}
              runtime={runtime}
              setRuntime={setRuntime}
              onClose={() => {
                if (modal.type === 'onboarding') setOnboardingDismissed(true);
                setModal(null);
              }}
              onUpdateClosed={() => { setModal(null); setUpdatesSeen(true); }}
              onOnboardingResolved={() => setModal(null)}
              dark={dark} setDark={setDark}
              menubarMode={menubarMode} setMenubarMode={setMenubarMode}
              globalHotkey={globalHotkey} setGlobalHotkey={setGlobalHotkey}
              notifyOnExit={notifyOnExit} setNotifyOnExit={setNotifyOnExit}
              containers={containers} images={paletteImages} stacks={paletteStacks}
              setTab={setTab} setModal={setModal} setLogTarget={setLogTarget}
            />
          </Suspense>
        )}
        <ToastTray t={t} />
      </div>
    </div>
  );
}
