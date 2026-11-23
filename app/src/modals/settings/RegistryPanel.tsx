import { useEffect, useState } from 'react';
import { api } from '../../api';
import { pillBtn } from '../../components';
import type { ThemeTokens } from '../../theme';
import { withToast } from '../../toast';

// Registry logins (`container registry list/login/logout`). The
// password is handed straight to the CLI over stdin and lands in the
// user's keychain — it never touches app state beyond this controlled
// input, which is cleared after a successful login.
export function RegistryPanel({ t }: { t: ThemeTokens }) {
  const [logins, setLogins] = useState<{ hostname: string; username: string }[] | null>(null);
  const reload = () => api.registryList().then(setLogins).catch(() => setLogins([]));
  useEffect(() => { reload(); }, []);
  const [server, setServer] = useState('');
  const [user, setUser] = useState('');
  const [pass, setPass] = useState('');
  const canLogin = server.trim() !== '' && user.trim() !== '' && pass !== '';
  const doLogin = () => {
    if (!canLogin) return;
    withToast(`login ${server.trim()}`, api.registryLogin(server.trim(), user.trim(), pass))
      .then(() => { setPass(''); reload(); })
      .catch(() => {});
  };
  const field = (value: string, set: (s: string) => void, placeholder: string, type = 'text') => (
    <input
      value={value} onChange={e => set(e.target.value)} placeholder={placeholder} type={type}
      spellCheck={false} autoCapitalize="off" autoCorrect="off"
      style={{ flex: 1, minWidth: 0, padding: '6px 10px', background: t.surfaceAlt, color: t.fg1, border: `1px solid ${t.border}`, borderRadius: 4, fontFamily: t.mono, fontSize: 12, outline: 'none' }}
    />
  );
  return (
    <div>
      {logins === null && <div style={{ fontFamily: t.mono, fontSize: 11, color: t.fg3 }}>Loading…</div>}
      {logins?.length === 0 && <div style={{ fontFamily: t.mono, fontSize: 11, color: t.fg3, marginBottom: 8 }}>No registries logged in.</div>}
      {logins?.map(l => (
        <div key={l.hostname} style={{ display: 'flex', alignItems: 'center', gap: 10, padding: '8px 0', borderBottom: `1px solid ${t.border}` }}>
          <div style={{ flex: 1, minWidth: 0 }}>
            <div style={{ fontSize: 13, color: t.fg1, fontFamily: t.mono }}>{l.hostname}</div>
            {l.username && <div style={{ fontSize: 11, color: t.fg3, fontFamily: t.mono }}>{l.username}</div>}
          </div>
          <button style={pillBtn(t, t.danger)}
                  onClick={() => withToast(`logout ${l.hostname}`, api.registryLogout(l.hostname)).then(reload).catch(() => {})}>
            Log out
          </button>
        </div>
      ))}
      <div style={{ display: 'flex', gap: 8, marginTop: 10 }}>
        {field(server, setServer, 'registry (e.g. ghcr.io)')}
        {field(user, setUser, 'username')}
        {field(pass, setPass, 'password / token', 'password')}
        <button style={{ ...pillBtn(t), opacity: canLogin ? 1 : 0.5 }} disabled={!canLogin} onClick={doLogin}>Log in</button>
      </div>
      <div style={{ fontSize: 10, color: t.fg3, marginTop: 6 }}>
        Credentials are stored by the container CLI in your macOS keychain.
      </div>
    </div>
  );
}
