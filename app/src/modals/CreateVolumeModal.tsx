import { useState } from 'react';
import { api } from '../api';
import type { ThemeTokens } from '../theme';
import { toast, withToast } from '../toast';
import { validateLabel, validateSize, validateVolumeName } from '../validate';
import { useCreateFormStyles, FieldNote, CreateModalShell } from './CreateModalShell';

export function CreateVolumeModal({ t, onCreated, onClose }: {
  t: ThemeTokens; onCreated: () => void; onClose: () => void;
}) {
  const s = useCreateFormStyles(t);
  const [name, setName] = useState('');
  const [size, setSize] = useState('');
  const [labels, setLabels] = useState('');
  const [busy, setBusy] = useState(false);
  const labelList = labels.split('\n').map(l => l.trim()).filter(Boolean);
  const nameError = name.trim() ? validateVolumeName(name) : null;
  const sizeError = validateSize(size);
  const labelError = labelList.map(validateLabel).find(Boolean) ?? null;
  const canSubmit = !!name.trim() && !nameError && !sizeError && !labelError;
  const submit = () => {
    const n = name.trim();
    setBusy(true);
    withToast(`create volume ${n}`, api.createVolume({
      name: n,
      size: size.trim() || undefined,
      labels: labelList.length ? labelList : undefined,
    }))
      .then(() => { toast(`Created volume ${n}`, 'info'); onCreated(); onClose(); })
      .catch(() => setBusy(false));
  };
  return (
    <CreateModalShell t={t} title="New volume" subtitle="container volume create" busy={busy}
                      canSubmit={canSubmit} submitLabel="Create" onSubmit={submit} onClose={onClose}>
      <div>
        <label style={s.label} htmlFor="vol-name">Name</label>
        <input id="vol-name" style={{ ...s.input, borderColor: nameError ? t.danger : t.border }} autoFocus
               value={name} onChange={e => setName(e.target.value)} placeholder="mydata" spellCheck={false}
               aria-invalid={nameError ? true : undefined} />
        <FieldNote t={t} error={nameError} />
      </div>
      <div>
        <label style={s.label} htmlFor="vol-size">Size <span style={s.opt}>(optional)</span></label>
        <input id="vol-size" style={{ ...s.input, borderColor: sizeError ? t.danger : t.border }}
               value={size} onChange={e => setSize(e.target.value)} placeholder="10G" spellCheck={false}
               aria-invalid={sizeError ? true : undefined} />
        <FieldNote t={t} error={sizeError}>Maximum size, with an optional K, M, G, T or P suffix. Leave empty for the CLI default (a large sparse disk that only uses space as it fills).</FieldNote>
      </div>
      <div>
        <label style={s.label} htmlFor="vol-labels">Labels <span style={s.opt}>(key=value, one per line)</span></label>
        <textarea id="vol-labels" style={{ ...s.input, minHeight: 48, resize: 'vertical', borderColor: labelError ? t.danger : t.border }}
                  value={labels} onChange={e => setLabels(e.target.value)} placeholder="team=infra" spellCheck={false}
                  aria-invalid={labelError ? true : undefined} />
        <FieldNote t={t} error={labelError} />
      </div>
    </CreateModalShell>
  );
}
