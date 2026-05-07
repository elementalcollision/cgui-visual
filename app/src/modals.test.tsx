import { describe, it, expect, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { TrivyModal, DoctorModal, OnboardingModal, parseInspect } from './modals';
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
