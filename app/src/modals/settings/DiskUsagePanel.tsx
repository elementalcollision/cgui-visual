import { useEffect, useState } from 'react';
import { api } from '../../api';
import { Bar } from '../../components';
import type { ThemeTokens } from '../../theme';
import type { DiskUsage, DiskUsageRow } from '../../types';

// `container system df` rendered as one row per category. Loaded when
// the Settings modal opens; bytes are formatted GiB/MB-style by scale.
export function DiskUsagePanel({ t }: { t: ThemeTokens }) {
  const [du, setDu] = useState<DiskUsage | null>(null);
  useEffect(() => { api.systemDf().then(setDu).catch(() => {}); }, []);
  if (!du) return <div style={{ fontFamily: t.mono, fontSize: 11, color: t.fg3 }}>Loading…</div>;
  const fmt = (b: number) => {
    if (b >= 1024 ** 3) return `${(b / 1024 ** 3).toFixed(2)} GB`;
    if (b >= 1024 ** 2) return `${(b / 1024 ** 2).toFixed(1)} MB`;
    return `${(b / 1024).toFixed(0)} KB`;
  };
  const rows: { label: string; row: DiskUsageRow }[] = [
    { label: 'Images', row: du.images },
    { label: 'Containers', row: du.containers },
    { label: 'Volumes', row: du.volumes },
  ];
  return (
    <div>
      {rows.map(({ label, row }) => {
        const pct = row.sizeInBytes > 0 ? (row.reclaimable / row.sizeInBytes) * 100 : 0;
        return (
          <div key={label} style={{ display: 'flex', alignItems: 'center', gap: 12, padding: '7px 0', borderBottom: `1px solid ${t.border}` }}>
            <div style={{ width: 90, fontSize: 12, color: t.fg1 }}>{label}</div>
            <div style={{ width: 70, fontSize: 11, color: t.fg3, fontFamily: t.mono, fontVariantNumeric: 'tabular-nums' }}>
              {row.active}/{row.total} active
            </div>
            <div style={{ flex: 1 }}>
              <Bar pct={pct} color={pct > 75 ? t.warning : t.accent} bg={t.surfaceAlt} h={5} />
            </div>
            <div style={{ width: 175, textAlign: 'right', fontSize: 11, color: t.fg3, fontFamily: t.mono, fontVariantNumeric: 'tabular-nums' }}>
              {fmt(row.sizeInBytes)} · {fmt(row.reclaimable)} reclaimable
            </div>
          </div>
        );
      })}
    </div>
  );
}
