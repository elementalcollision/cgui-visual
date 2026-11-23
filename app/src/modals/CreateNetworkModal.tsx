import { useState } from 'react';
import { api } from '../api';
import type { ThemeTokens } from '../theme';
import { toast, withToast } from '../toast';
import { validateCidr, validateCidr6, validateLabel, validateNetworkName } from '../validate';
import { useCreateFormStyles, FieldNote, CreateModalShell } from './CreateModalShell';

export function CreateNetworkModal({ t, onCreated, onClose }: {
  t: ThemeTokens; onCreated: () => void; onClose: () => void;
}) {
  const s = useCreateFormStyles(t);
  const [name, setName] = useState('');
  const [subnet, setSubnet] = useState('');
  const [subnetV6, setSubnetV6] = useState('');
  const [internal, setInternal] = useState(false);
  const [labels, setLabels] = useState('');
  const [busy, setBusy] = useState(false);
  const labelList = labels.split('\n').map(l => l.trim()).filter(Boolean);
  const nameError = name.trim() ? validateNetworkName(name) : null;
  const subnetError = validateCidr(subnet);
  const subnetV6Error = validateCidr6(subnetV6);
  const labelError = labelList.map(validateLabel).find(Boolean) ?? null;
  const canSubmit = !!name.trim() && !nameError && !subnetError && !subnetV6Error && !labelError;
  const submit = () => {
    const n = name.trim();
    setBusy(true);
    withToast(`create network ${n}`, api.createNetwork({
      name: n,
      subnet: subnet.trim() || undefined,
      subnetV6: subnetV6.trim() || undefined,
      internal: internal || undefined,
      labels: labelList.length ? labelList : undefined,
    }))
      .then(() => { toast(`Created network ${n}`, 'info'); onCreated(); onClose(); })
      .catch(() => setBusy(false));
  };
  return (
    <CreateModalShell t={t} title="New network" subtitle="container network create" busy={busy}
                      canSubmit={canSubmit} submitLabel="Create" onSubmit={submit} onClose={onClose}>
      <div>
        <label style={s.label} htmlFor="net-name">Name</label>
        <input id="net-name" style={{ ...s.input, borderColor: nameError ? t.danger : t.border }} autoFocus
               value={name} onChange={e => setName(e.target.value)} placeholder="backend" spellCheck={false}
               aria-invalid={nameError ? true : undefined} />
        <FieldNote t={t} error={nameError}>Lowercase letters, digits, ".", "_" and "-".</FieldNote>
      </div>
      <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 12 }}>
        <div>
          <label style={s.label} htmlFor="net-subnet">IPv4 subnet <span style={s.opt}>(optional)</span></label>
          <input id="net-subnet" style={{ ...s.input, borderColor: subnetError ? t.danger : t.border }}
                 value={subnet} onChange={e => setSubnet(e.target.value)} placeholder="192.168.100.0/24" spellCheck={false}
                 aria-invalid={subnetError ? true : undefined} />
          <FieldNote t={t} error={subnetError} />
        </div>
        <div>
          <label style={s.label} htmlFor="net-subnet6">IPv6 prefix <span style={s.opt}>(optional)</span></label>
          <input id="net-subnet6" style={{ ...s.input, borderColor: subnetV6Error ? t.danger : t.border }}
                 value={subnetV6} onChange={e => setSubnetV6(e.target.value)} placeholder="fd00:1234::/64" spellCheck={false}
                 aria-invalid={subnetV6Error ? true : undefined} />
          <FieldNote t={t} error={subnetV6Error} />
        </div>
      </div>
      <label style={{ display: 'flex', alignItems: 'center', gap: 8, fontSize: 13, color: t.fg1, cursor: 'pointer' }}>
        <input type="checkbox" checked={internal} onChange={e => setInternal(e.target.checked)} style={{ accentColor: t.accent }} />
        Host-only (--internal): no access to the outside network
      </label>
      <div>
        <label style={s.label} htmlFor="net-labels">Labels <span style={s.opt}>(key=value, one per line)</span></label>
        <textarea id="net-labels" style={{ ...s.input, minHeight: 48, resize: 'vertical', borderColor: labelError ? t.danger : t.border }}
                  value={labels} onChange={e => setLabels(e.target.value)} placeholder="team=infra" spellCheck={false}
                  aria-invalid={labelError ? true : undefined} />
        <FieldNote t={t} error={labelError} />
      </div>
    </CreateModalShell>
  );
}
