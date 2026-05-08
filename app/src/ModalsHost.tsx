// Modal dispatch host. Lazy-loaded via React.lazy in App.tsx so the
// ~30 KB of modal code doesn't ship in the initial bundle. The user
// pays the import cost on the first modal open, which is async-fine
// (modals already do an `api.invoke()` before rendering content).

import { lazy, Suspense } from 'react';
import { api } from './api';

// Lazy-load TerminalModal so xterm.js (~250 KB) isn't in the initial bundle.
const TerminalModal = lazy(() => import('./Terminal'));

import {
  DetailModal,
  PullModal,
  TrivyModal,
  UpdateModal,
  DoctorModal,
  SettingsModal,
  JsonInspectModal,
  RunImageModal,
  OnboardingModal,
  CommandPaletteModal,
  ImageInspectModal,
} from './modals';
import { toast } from './toast';
import type { ThemeTokens } from './theme';
import type { Container, Image, Modal, Runtime, Stack, Tab } from './types';

export default function ModalsHost(props: {
  modal: Modal;
  t: ThemeTokens;
  runtime: Runtime;
  setRuntime: (r: Runtime) => void;
  pullReference: string;
  onClose: () => void;
  onUpdateClosed: () => void;
  onOnboardingResolved: () => void;
  // Settings-modal state, threaded so toggles round-trip via App's prefs.
  menubarMode: boolean;
  setMenubarMode: (b: boolean) => void;
  globalHotkey: string;
  setGlobalHotkey: (s: string) => void;
  notifyOnExit: boolean;
  setNotifyOnExit: (b: boolean) => void;
  dark: boolean;
  setDark: (b: boolean) => void;
  // Command palette inputs + dispatch
  containers: Container[];
  images: Image[];
  stacks: Stack[];
  setTab: (t: Tab) => void;
  setModal: (m: Modal) => void;
  setLogTarget: (id: string | undefined) => void;
}) {
  const {
    modal, t, runtime, setRuntime, pullReference, onClose, onUpdateClosed, onOnboardingResolved,
    menubarMode, setMenubarMode, globalHotkey, setGlobalHotkey, notifyOnExit, setNotifyOnExit,
    dark, setDark, containers, images, stacks, setTab, setModal, setLogTarget,
  } = props;
  if (!modal) return null;

  switch (modal.type) {
    case 'onboarding':
      return <OnboardingModal t={t} onAvailable={onOnboardingResolved} onDismiss={onClose} />;
    case 'detail':
      return <DetailModal
        item={modal.payload} t={t} onClose={onClose}
        onExec={c => setModal({ type: 'terminal', container: c })}
      />;
    case 'pull':
      return <PullModal t={t} reference={pullReference} onClose={onClose} />;
    case 'trivy':
      return <TrivyModal t={t} image={modal.image} onClose={onClose} />;
    case 'update':
      return <UpdateModal t={t} onClose={onUpdateClosed} />;
    case 'doctor':
      return <DoctorModal t={t} onClose={onClose} />;
    case 'settings':
      return <SettingsModal
        t={t} runtime={runtime} setRuntime={setRuntime} onClose={onClose}
        dark={dark} setDark={setDark}
        menubarMode={menubarMode} setMenubarMode={setMenubarMode}
        globalHotkey={globalHotkey} setGlobalHotkey={setGlobalHotkey}
        notifyOnExit={notifyOnExit} setNotifyOnExit={setNotifyOnExit}
      />;
    case 'volumeInspect':
      return <JsonInspectModal t={t} title={modal.name} subtitle="container volume inspect"
                               fetcher={() => api.inspectVolume(modal.name)} onClose={onClose} />;
    case 'networkInspect':
      return <JsonInspectModal t={t} title={modal.name} subtitle="container network inspect"
                               fetcher={() => api.inspectNetwork(modal.id)} onClose={onClose} />;
    case 'imageInspect':
      return <ImageInspectModal t={t} reference={modal.reference} onClose={onClose} />;
    case 'runImage':
      return <RunImageModal t={t} image={modal.image}
                            onLaunched={(id) => toast(`launched ${id}`, 'info')}
                            onClose={onClose} />;
    case 'terminal':
      return (
        <Suspense fallback={null}>
          <TerminalModal container={modal.container} t={t} onClose={onClose} />
        </Suspense>
      );
    case 'commandPalette':
      return <CommandPaletteModal
        t={t}
        containers={containers}
        images={images}
        stacks={stacks}
        onClose={onClose}
        onTab={tab => { setTab(tab); }}
        onAction={id => {
          if (id === 'settings') setModal({ type: 'settings' });
          else if (id === 'doctor') setModal({ type: 'doctor' });
          else if (id === 'pull') setModal({ type: 'pull' });
        }}
        onContainer={c => setModal({ type: 'detail', payload: c })}
        onImage={img => setModal({ type: 'imageInspect', reference: img.ref })}
        onStack={s => {
          // No dedicated stack inspect modal — switch to Stacks tab and
          // route the search bar to surface the picked stack name. The
          // search filter already plumbs through StacksView.
          setTab('stacks');
          setLogTarget(undefined);
          void s;
        }}
      />;
  }
}
