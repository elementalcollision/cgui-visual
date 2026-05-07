import { describe, it, expect, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { TrivyModal, DoctorModal, OnboardingModal, parseInspect, fuzzyScore, parseImageInspect } from './modals';
import { getTheme } from './theme';

const t = getTheme(true);

describe('TrivyModal', () => {
  it('lists every fixture finding when no filter is active', async () => {
    render(<TrivyModal t={t} image="alpine" onClose={() => {}} />);
    // Wait for the api fallback fixture to populate.
    await screen.findByText(/CVE-2024-21626/);
    // Twelve fixture findings — at least the two CRITICAL CVEs and several HIGHs.
    expect(screen.getByText('CVE-2024-21626')).toBeInTheDocument();
    expect(screen.getByText('CVE-2025-1138')).toBeInTheDocument();
    expect(screen.getByText('CVE-2024-3094')).toBeInTheDocument();
    expect(screen.getByText('CVE-2024-7264')).toBeInTheDocument();
  });

  it('filters by severity when a severity pill is clicked', async () => {
    const user = userEvent.setup();
    render(<TrivyModal t={t} image="alpine" onClose={() => {}} />);
    await screen.findByText(/CVE-2024-21626/);
    // Click the CRITICAL severity pill specifically. The drawer-enabled
    // rows also contain CRITICAL in their accessible name, so we anchor
    // on the count-suffixed pill ("CRITICAL 2") to disambiguate.
    const critPill = screen.getByRole('button', { name: /^CRITICAL\s+\d+$/i });
    await user.click(critPill);
    expect(screen.getByText('CVE-2024-21626')).toBeInTheDocument();
    expect(screen.getByText('CVE-2025-1138')).toBeInTheDocument();
    // A HIGH-only CVE should be hidden under the CRITICAL filter.
    expect(screen.queryByText('CVE-2024-3094')).not.toBeInTheDocument();
  });

  it('filters by free-text search', async () => {
    const user = userEvent.setup();
    render(<TrivyModal t={t} image="alpine" onClose={() => {}} />);
    await screen.findByText(/CVE-2024-21626/);
    const search = screen.getByPlaceholderText(/Search CVE/);
    await user.type(search, 'openssh');
    expect(screen.getByText('CVE-2024-6387')).toBeInTheDocument();
    expect(screen.queryByText('CVE-2024-21626')).not.toBeInTheDocument();
  });
});

describe('fuzzyScore (Cmd-K palette ranker)', () => {
  it('returns 0 for an empty query (no constraint)', () => {
    expect(fuzzyScore('anything', '')).toBe(0);
  });
  it('returns Infinity when the query is not a subsequence', () => {
    expect(fuzzyScore('alpine', 'xyz')).toBe(Infinity);
  });
  it('prefers tighter matches over scattered ones', () => {
    // "redis" tightly matches "redis-server"; "rds" still matches but with gaps.
    const tight = fuzzyScore('redis-server', 'redis');
    const loose = fuzzyScore('redis-server', 'rds');
    expect(tight).toBeLessThan(loose);
    expect(tight).toBeLessThan(Infinity);
    expect(loose).toBeLessThan(Infinity);
  });
  it('treats word-start matches as cheaper than mid-word ones', () => {
    // 'p' at the start of "postgres" should beat 'p' inside "compose-app".
    const start = fuzzyScore('postgres', 'p');
    const mid = fuzzyScore('compose-app', 'p');
    expect(start).toBeLessThanOrEqual(mid);
  });
});

describe('parseInspect (DetailModal projection)', () => {
  it('returns null for non-JSON input', () => {
    expect(parseInspect('Loading…')).toBeNull();
    expect(parseInspect('not json')).toBeNull();
  });

  it('extracts env / mounts / network / ports from a docker-style payload', () => {
    const payload = JSON.stringify({
      State: { Status: 'running', Health: { Status: 'healthy', FailingStreak: 0, Log: [{ ExitCode: 0, Output: 'ok' }] } },
      Config: { Env: ['FOO=bar', 'NOEQ', 'PATH=/usr/bin'] },
      Mounts: [
        { Source: '/host', Destination: '/in', Type: 'bind', RW: false },
        { Source: 'vol', Destination: '/data', Type: 'volume' },
      ],
      NetworkSettings: {
        Networks: { bridge: { IPAddress: '172.17.0.2', MacAddress: 'aa:bb' } },
        Ports: {
          '8080/tcp': [{ HostPort: '8080', HostIp: '0.0.0.0' }],
          '9090/tcp': null,
        },
      },
    });
    const p = parseInspect(payload)!;
    expect(p.env).toEqual([
      { key: 'FOO', value: 'bar' },
      { key: 'NOEQ', value: '' },
      { key: 'PATH', value: '/usr/bin' },
    ]);
    expect(p.mounts).toHaveLength(2);
    expect(p.mounts[0].readOnly).toBe(true);
    expect(p.mounts[1].readOnly).toBeUndefined();
    expect(p.network[0]).toEqual({ name: 'bridge', ip: '172.17.0.2', mac: 'aa:bb' });
    expect(p.ports.find(x => x.container === '8080/tcp')?.host).toBe('0.0.0.0:8080');
    expect(p.ports.find(x => x.container === '9090/tcp')?.host).toContain('exposed');
    expect(p.health?.status).toBe('healthy');
  });

  it('handles top-level inspect arrays (Apple `container inspect`)', () => {
    const payload = JSON.stringify([
      { Config: { Env: ['A=1'] }, NetworkSettings: { IPAddress: '10.0.0.5' } },
    ]);
    const p = parseInspect(payload)!;
    expect(p.env).toEqual([{ key: 'A', value: '1' }]);
    expect(p.network[0]).toEqual({ name: 'default', ip: '10.0.0.5', mac: undefined });
  });
});

describe('parseImageInspect (ImageInspectModal projection)', () => {
  it('returns null for non-JSON input', () => {
    expect(parseImageInspect('Loading…')).toBeNull();
  });

  it('walks history and aligns diff_ids to non-empty layers', () => {
    const payload = JSON.stringify({
      architecture: 'arm64',
      os: 'linux',
      rootfs: { diff_ids: ['sha256:aaa', 'sha256:bbb'] },
      history: [
        { created: '2024-01-01T00:00:00Z', created_by: '/bin/sh -c #(nop) FROM alpine', empty_layer: true },
        { created: '2024-01-02T00:00:00Z', created_by: '/bin/sh -c apk add curl' },
        { created: '2024-01-03T00:00:00Z', created_by: '/bin/sh -c #(nop) ENV KEY=v', empty_layer: true },
        { created: '2024-01-04T00:00:00Z', created_by: '/bin/sh -c make install' },
      ],
    });
    const p = parseImageInspect(payload)!;
    expect(p.layers).toHaveLength(4);
    expect(p.layers[0].emptyLayer).toBe(true);
    expect(p.layers[0].diffId).toBeUndefined();
    expect(p.layers[1].emptyLayer).toBeFalsy();
    expect(p.layers[1].diffId).toBe('sha256:aaa');
    expect(p.layers[2].emptyLayer).toBe(true);
    expect(p.layers[3].diffId).toBe('sha256:bbb');
    expect(p.architecture).toBe('arm64');
    expect(p.os).toBe('linux');
  });

  it('handles docker-style PascalCase shape', () => {
    const payload = JSON.stringify([{
      Architecture: 'amd64', Os: 'linux',
      RootFS: { DiffIDs: ['sha256:xyz'] },
      History: [
        { Created: '2024-06-01T00:00:00Z', CreatedBy: '/bin/sh -c apk add jq' },
      ],
    }]);
    const p = parseImageInspect(payload)!;
    expect(p.layers[0].createdBy).toMatch(/apk add jq/);
    expect(p.layers[0].diffId).toBe('sha256:xyz');
    expect(p.architecture).toBe('amd64');
  });
});

describe('DoctorModal', () => {
  it('renders the fixture summary line', async () => {
    render(<DoctorModal t={t} onClose={() => {}} />);
    await screen.findByText(/active profile/);
    // 6 ok + 2 warn + 0 fail in fixtures
    expect(screen.getByText(/6 passed · 2 warnings · 0 failures/)).toBeInTheDocument();
  });
});

describe('OnboardingModal', () => {
  it('renders both install paths with Apple as the primary CTA', () => {
    render(<OnboardingModal t={t} onAvailable={() => {}} onDismiss={() => {}} />);
    expect(screen.getByText(/CLI not detected/i)).toBeInTheDocument();
    // Apple's signed installer is the prominent action.
    expect(screen.getByRole('button', { name: /Download from apple\/container releases/i })).toBeInTheDocument();
    // Homebrew alternative is present but secondary.
    expect(screen.getByText(/brew install container/)).toBeInTheDocument();
    expect(screen.getByRole('button', { name: /^Copy$/ })).toBeInTheDocument();
  });

  it('Copy button writes the brew command to the clipboard', async () => {
    const user = userEvent.setup();
    const writeText = vi.fn().mockResolvedValue(undefined);
    Object.defineProperty(navigator, 'clipboard', {
      value: { writeText }, writable: true, configurable: true,
    });
    render(<OnboardingModal t={t} onAvailable={() => {}} onDismiss={() => {}} />);
    await user.click(screen.getByRole('button', { name: /^Copy$/ }));
    expect(writeText).toHaveBeenCalledWith('brew install container');
    // The button label flips to confirm the copy.
    expect(await screen.findByText(/✓ Copied/)).toBeInTheDocument();
  });

  it('Continue with sample data calls onDismiss', async () => {
    const user = userEvent.setup();
    const onDismiss = vi.fn();
    render(<OnboardingModal t={t} onAvailable={() => {}} onDismiss={onDismiss} />);
    await user.click(screen.getByRole('button', { name: /Continue with sample data/i }));
    expect(onDismiss).toHaveBeenCalled();
  });

  it('Re-check fires onAvailable when the runtime is available', async () => {
    const user = userEvent.setup();
    const onAvailable = vi.fn();
    // Browser-dev mode: api.runtimeAvailable falls back to true, so the
    // re-check path resolves to "available" without needing to mock invoke.
    render(<OnboardingModal t={t} onAvailable={onAvailable} onDismiss={() => {}} />);
    await user.click(screen.getByRole('button', { name: /Re-check/i }));
    // findBy waits for the async setState after api.runtimeAvailable resolves.
    await screen.findByRole('button', { name: /Re-check/i });
    expect(onAvailable).toHaveBeenCalled();
  });
});
