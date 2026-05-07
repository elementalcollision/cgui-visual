/* cgui — main app. Workbench + frameless, locked in. */

const { useState: uS, useEffect: uE, useMemo: uM } = React;
const { FramelessChrome, Sidebar, TopBar } = window.cguiBase;
const V = window.VARIATIONS;
const { ContainersView, ImagesView, VolumesView, NetworksView, StacksView, LogsView } = window.cguiViews;
const { DetailModal, PullModal, TrivyModal, UpdateModal, DoctorModal, SettingsModal } = window.cguiModals;

const TWEAK_DEFAULTS = /*EDITMODE-BEGIN*/{
  "dark": true,
  "sidebarCollapsed": false,
  "runtime": "container",
  "showSpark": true,
  "scenario": "many"
}/*EDITMODE-END*/;

const VARIATION = 'workbench'; // locked

function App() {
  const tweaks = window.useTweaks ? window.useTweaks(TWEAK_DEFAULTS) : [TWEAK_DEFAULTS, () => {}];
  const [tw, setTw] = tweaks;
  const dark = tw.dark;
  const collapsed = tw.sidebarCollapsed;
  const runtime = tw.runtime;

  const t = uM(() => ({ ...V[VARIATION][dark ? 'dark' : 'light'], radius: V[VARIATION].radius, density: V[VARIATION].density }), [dark]);

  const [search, setSearch] = uS('');
  const [selected, setSelected] = uS('a3f8e2c1');
  const [modal, setModal] = uS(() => {
    const sp = typeof location !== 'undefined' ? new URLSearchParams(location.search) : null;
    let h = (sp?.get('modal')) || (typeof location !== 'undefined' ? location.hash.replace('#', '') : '');
    if (!h && typeof localStorage !== 'undefined') h = localStorage.getItem('cgui_modal') || '';
    if (h === 'trivy') return { type: 'trivy' };
    if (h === 'pull') return { type: 'pull' };
    if (h === 'update') return { type: 'update' };
    if (h === 'doctor') return { type: 'doctor' };
    if (h === 'settings') return { type: 'settings' };
    if (h === 'detail') return { type: 'detail', payload: window.SAMPLE.containers[0] };
    return null;
  });
  const [tab, setTab] = uS(() => {
    const sp = typeof location !== 'undefined' ? new URLSearchParams(location.search) : null;
    let h = (sp?.get('modal')) || (sp?.get('tab')) || (typeof location !== 'undefined' ? location.hash.replace('#', '') : '');
    if (!h && typeof localStorage !== 'undefined') h = localStorage.getItem('cgui_modal') || localStorage.getItem('cgui_tab') || '';
    if (h === 'images' || h === 'trivy') return 'images';
    if (h === 'volumes') return 'volumes';
    if (h === 'networks') return 'networks';
    if (h === 'stacks') return 'stacks';
    if (h === 'logs') return 'logs';
    return 'containers';
  });
  const [showUpdateBadge, setShowUpdateBadge] = uS(true);

  uE(() => {
    window.__cguiOpenModal = (m, payload) => setModal(m ? { type: m, payload } : null);
    window.__cguiSetTab = (t) => setTab(t);
    window.__cguiSetTweak = (k, v) => setTw(k, v);
    return () => { delete window.__cguiOpenModal; delete window.__cguiSetTab; delete window.__cguiSetTweak; };
  }, []);

  uE(() => {
    const onKey = (e) => {
      if (e.key === 'Escape') setModal(null);
      if (e.key === '/' && !modal && document.activeElement.tagName !== 'INPUT') {
        e.preventDefault();
        document.querySelector('input[placeholder^="Filter"]')?.focus();
      }
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [modal]);

  const desktopBg = dark
    ? 'radial-gradient(ellipse at 30% 20%, #2A2D3A 0%, #14161D 60%, #0A0B10 100%)'
    : 'radial-gradient(ellipse at 30% 20%, #DDE2EC 0%, #C5CCDB 60%, #B0B8CB 100%)';

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
        :root { --font-serif: "Newsreader", "Iowan Old Style", Georgia, serif; }
        input::placeholder { color: ${t.fg3}; }
      `}</style>

      <div style={{ width: '100%', height: '100%', maxWidth: 1480, maxHeight: 920, position: 'relative' }}>
        <FramelessChrome t={t}>
          <div style={{ flex: 1, display: 'flex', minHeight: 0 }}>
            <Sidebar
              tab={tab} setTab={setTab} collapsed={collapsed} t={t} variation={VARIATION}
              onSettings={() => setModal({ type: 'settings' })}
              onDoctor={() => setModal({ type: 'doctor' })}
            />
            <div style={{ flex: 1, display: 'flex', flexDirection: 'column', minWidth: 0 }}>
              <TopBar
                tab={tab} t={t} variation={VARIATION}
                search={search} setSearch={setSearch}
                onPull={() => setModal({ type: 'pull' })}
                onCollapse={() => setTw('sidebarCollapsed', !collapsed)}
                collapsed={collapsed}
                runtime={runtime}
                dark={dark}
                setDark={(v) => setTw('dark', v)}
                onUpdate={showUpdateBadge ? () => setModal({ type: 'update' }) : null}
              />
              {tab === 'containers' && <ContainersView t={t} variation={VARIATION} search={search} selected={selected} setSelected={setSelected} onInspect={(c) => setModal({ type: 'detail', payload: c })} onLogs={() => setTab('logs')} />}
              {tab === 'images' && <ImagesView t={t} variation={VARIATION} search={search} onScan={() => setModal({ type: 'trivy' })} />}
              {tab === 'volumes' && <VolumesView t={t} variation={VARIATION} />}
              {tab === 'networks' && <NetworksView t={t} variation={VARIATION} />}
              {tab === 'stacks' && <StacksView t={t} variation={VARIATION} onInspect={() => {}} />}
              {tab === 'logs' && <LogsView t={t} variation={VARIATION} />}
              <StatusBar t={t} runtime={runtime} tab={tab} />
            </div>
          </div>
        </FramelessChrome>

        {modal?.type === 'detail' && <DetailModal item={modal.payload} t={t} variation={VARIATION} onClose={() => setModal(null)} />}
        {modal?.type === 'pull' && <PullModal t={t} variation={VARIATION} onClose={() => setModal(null)} />}
        {modal?.type === 'trivy' && <TrivyModal t={t} variation={VARIATION} onClose={() => setModal(null)} />}
        {modal?.type === 'update' && <UpdateModal t={t} variation={VARIATION} onClose={() => { setModal(null); setShowUpdateBadge(false); }} />}
        {modal?.type === 'doctor' && <DoctorModal t={t} variation={VARIATION} onClose={() => setModal(null)} />}
        {modal?.type === 'settings' && <SettingsModal t={t} variation={VARIATION} runtime={runtime} setRuntime={(r) => setTw('runtime', r)} onClose={() => setModal(null)} />}
      </div>

      <window.TweaksPanel title="Tweaks">
        <window.TweakSection label="Appearance" />
        <window.TweakToggle label="Dark mode" value={dark} onChange={(v) => setTw('dark', v)} />
        <window.TweakToggle label="Sidebar collapsed" value={collapsed} onChange={(v) => setTw('sidebarCollapsed', v)} />
        <window.TweakSection label="Runtime profile" />
        <window.TweakRadio label="Active" value={runtime}
          options={[
            { value: 'container', label: 'container' },
            { value: 'docker', label: 'docker' },
            { value: 'podman', label: 'podman' },
          ]}
          onChange={(v) => setTw('runtime', v)} />
      </window.TweaksPanel>
    </div>
  );
}

function StatusBar({ t, runtime, tab }) {
  return (
    <div style={{
      height: 26, borderTop: `1px solid ${t.border}`, background: t.surfaceAlt,
      display: 'flex', alignItems: 'center', padding: '0 14px', gap: 14,
      fontSize: 11, color: t.fg3, fontFamily: t.mono, flexShrink: 0,
    }}>
      <span><span style={{ color: t.success }}>●</span> connected</span>
      <span style={{ color: t.fg3 }}>·</span>
      <span>runtime <span style={{ color: t.fg2 }}>{runtime}</span></span>
      <span style={{ color: t.fg3 }}>·</span>
      <span>tab <span style={{ color: t.fg2 }}>{tab}</span></span>
      <div style={{ flex: 1 }} />
      <span style={{ color: t.fg3 }}>↑/↓ navigate</span>
      <span style={{ color: t.fg3 }}>↵ inspect</span>
      <span style={{ color: t.fg3 }}>L logs</span>
      <span style={{ color: t.fg3 }}>S scan</span>
      <span style={{ color: t.fg3 }}>? help</span>
    </div>
  );
}

ReactDOM.createRoot(document.getElementById('root')).render(<App />);
