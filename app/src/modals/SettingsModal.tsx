import { Icon, iconBtn } from '../components';
import type { ThemeTokens } from '../theme';
import { Backdrop } from './shared';
import { ServicesPanel } from './settings/ServicesPanel';
import { MachinesPanel } from './settings/MachinesPanel';
import { BuilderPanel } from './settings/BuilderPanel';
import { RegistryPanel } from './settings/RegistryPanel';
import { DiskUsagePanel } from './settings/DiskUsagePanel';
import { SettingsToggle } from './settings/SettingsToggle';

export function SettingsModal({
  t, onClose,
  dark, setDark,
  menubarMode, setMenubarMode,
  globalHotkey, setGlobalHotkey,
  notifyOnExit, setNotifyOnExit,
  backgroundLogs, setBackgroundLogs,
}: {
  t: ThemeTokens; onClose: () => void;
  dark: boolean; setDark: (b: boolean) => void;
  menubarMode: boolean; setMenubarMode: (b: boolean) => void;
  globalHotkey: string; setGlobalHotkey: (s: string) => void;
  notifyOnExit: boolean; setNotifyOnExit: (b: boolean) => void;
  backgroundLogs: boolean; setBackgroundLogs: (b: boolean) => void;
}) {
  return (
    <Backdrop title="Settings" onClose={onClose}>
      <div style={{ width: 640, background: t.surface, border: `1px solid ${t.border}`, borderRadius: 12, overflow: 'hidden', maxHeight: '85vh', display: 'flex', flexDirection: 'column' }}>
        <div style={{ padding: '16px 22px', borderBottom: `1px solid ${t.border}`, display: 'flex', alignItems: 'center', gap: 12 }}>
          <Icon name="cog" size={18} color={t.fg2} />
          <div style={{ flex: 1, fontSize: 15, fontWeight: 600, color: t.fg1 }}>Settings</div>
          <button onClick={onClose} style={iconBtn()} aria-label="Close"><Icon name="x" size={16} color={t.fg2} /></button>
        </div>
        <div style={{ flex: 1, overflow: 'auto', padding: '18px 22px' }}>
          <div style={{ fontSize: 11, fontWeight: 600, letterSpacing: '0.08em', textTransform: 'uppercase', color: t.fg3, marginBottom: 10 }}>Appearance</div>
          <SettingsToggle t={t} label="Dark mode" hint="Switch the UI to a light theme when off." value={dark} onChange={setDark} />

          <div style={{ fontSize: 11, fontWeight: 600, letterSpacing: '0.08em', textTransform: 'uppercase', color: t.fg3, margin: '20px 0 10px' }}>Menu bar</div>
          <SettingsToggle
            t={t}
            label="Menu-bar mode"
            hint="Closing the main window hides it instead of quitting: cgui keeps running in the menu bar (the Dock icon is hidden while the window is hidden) and the tray icon shows the running container count. Use the tray's Show cgui to bring the window back, or Quit cgui to exit. If your menu bar hides the icon, open cgui again from Spotlight or Finder to bring the window back. When off, closing the window quits cgui. Takes effect immediately."
            value={menubarMode}
            onChange={setMenubarMode}
          />
          <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', padding: '10px 0', borderBottom: `1px solid ${t.border}`, gap: 12 }}>
            <div style={{ minWidth: 0 }}>
              <div style={{ fontSize: 13, color: t.fg1 }}>Global summon hotkey</div>
              <div style={{ fontSize: 11, color: t.fg3, marginTop: 2 }}>e.g. <span style={{ fontFamily: t.mono }}>CmdOrCtrl+Alt+Space</span>. Empty disables.</div>
            </div>
            <input
              value={globalHotkey}
              onChange={e => setGlobalHotkey(e.target.value)}
              placeholder="(disabled)"
              spellCheck={false}
              style={{
                width: 220,
                padding: '6px 10px',
                background: t.surfaceAlt,
                color: t.fg1,
                border: `1px solid ${t.border}`,
                borderRadius: 4,
                fontFamily: t.mono,
                fontSize: 12,
                outline: 'none',
              }}
            />
          </div>

          <div style={{ fontSize: 11, fontWeight: 600, letterSpacing: '0.08em', textTransform: 'uppercase', color: t.fg3, margin: '20px 0 10px' }}>Notifications</div>
          <SettingsToggle
            t={t}
            label="Notify on container exit"
            hint="Show a macOS notification when a running container exits. Non-zero exit codes are highlighted."
            value={notifyOnExit}
            onChange={setNotifyOnExit}
          />

          <div style={{ fontSize: 11, fontWeight: 600, letterSpacing: '0.08em', textTransform: 'uppercase', color: t.fg3, margin: '20px 0 10px' }}>Logs</div>
          <SettingsToggle
            t={t}
            label="Save container logs in the background"
            hint="Keeps one container logs process per running container (up to 32) so output is saved even while no Logs view is open, and stores it locally for 7 days. Containers that ran and exited while cgui was closed are caught up too. Boot logs are never saved. Takes effect immediately."
            value={backgroundLogs}
            onChange={setBackgroundLogs}
          />

          <div style={{ fontSize: 11, fontWeight: 600, letterSpacing: '0.08em', textTransform: 'uppercase', color: t.fg3, margin: '20px 0 10px' }}>Services</div>
          <ServicesPanel t={t} />

          <div style={{ fontSize: 11, fontWeight: 600, letterSpacing: '0.08em', textTransform: 'uppercase', color: t.fg3, margin: '20px 0 10px' }}>Machines</div>
          <MachinesPanel t={t} />

          <div style={{ fontSize: 11, fontWeight: 600, letterSpacing: '0.08em', textTransform: 'uppercase', color: t.fg3, margin: '20px 0 10px' }}>Builder</div>
          <BuilderPanel t={t} />

          <div style={{ fontSize: 11, fontWeight: 600, letterSpacing: '0.08em', textTransform: 'uppercase', color: t.fg3, margin: '20px 0 10px' }}>Registry logins</div>
          <RegistryPanel t={t} />

          <div style={{ fontSize: 11, fontWeight: 600, letterSpacing: '0.08em', textTransform: 'uppercase', color: t.fg3, margin: '20px 0 10px' }}>Disk usage</div>
          <DiskUsagePanel t={t} />

          <div style={{ fontSize: 11, fontWeight: 600, letterSpacing: '0.08em', textTransform: 'uppercase', color: t.fg3, margin: '20px 0 10px' }}>State</div>
          <div style={{ fontFamily: t.mono, fontSize: 11, color: t.fg3 }}>~/.config/cgui-gui/state.json — auto-saved</div>
        </div>
      </div>
    </Backdrop>
  );
}
