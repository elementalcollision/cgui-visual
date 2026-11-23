import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen } from '@testing-library/react';
import { api } from './api';
import { needsCliTitle, resetCliVersion, useCliAtLeast, versionAtLeast } from './cliVersion';

describe('versionAtLeast', () => {
  it('compares numerically, not lexically', () => {
    expect(versionAtLeast('1.10.0', '1.6.0')).toBe(true);
    expect(versionAtLeast('1.7.1', '1.7.1')).toBe(true);
    expect(versionAtLeast('1.5.9', '1.6.0')).toBe(false);
    expect(versionAtLeast('1.4.1', '1.6.0')).toBe(false);
    expect(versionAtLeast('1.7', '1.7.0')).toBe(true);
    expect(versionAtLeast('v1.7.1', '1.7.1')).toBe(true);
    expect(versionAtLeast('1.7.1+build5', '1.7.1')).toBe(true);
  });

  it('fails open when the version is unknown or unparseable', () => {
    expect(versionAtLeast(null, '1.6.0')).toBe(true);
    expect(versionAtLeast(undefined, '1.6.0')).toBe(true);
    expect(versionAtLeast('nightly', '1.6.0')).toBe(true);
  });
});

describe('needsCliTitle', () => {
  it('drops trailing .0 components', () => {
    expect(needsCliTitle('1.6.0')).toBe('Needs Apple container 1.6 or later');
    expect(needsCliTitle('1.7.1')).toBe('Needs Apple container 1.7.1 or later');
  });
});

function Probe({ min }: { min: string }) {
  const gate = useCliAtLeast(min);
  return <button disabled={!gate.ok} title={gate.title}>probe</button>;
}

describe('useCliAtLeast', () => {
  beforeEach(() => { resetCliVersion(); vi.restoreAllMocks(); });

  it('disables with the tooltip on an older CLI', async () => {
    vi.spyOn(api, 'cliVersion').mockResolvedValue('1.4.1');
    render(<Probe min="1.6.0" />);
    const btn = await screen.findByRole('button', { name: 'probe' });
    await vi.waitFor(() => expect(btn).toBeDisabled());
    expect(btn).toHaveAttribute('title', 'Needs Apple container 1.6 or later');
  });

  it('stays enabled on a new enough CLI', async () => {
    vi.spyOn(api, 'cliVersion').mockResolvedValue('1.7.1');
    render(<Probe min="1.6.0" />);
    const btn = await screen.findByRole('button', { name: 'probe' });
    await vi.waitFor(() => expect(api.cliVersion).toHaveBeenCalled());
    expect(btn).toBeEnabled();
    expect(btn).not.toHaveAttribute('title');
  });

  it('stays enabled when the version is unknown', async () => {
    vi.spyOn(api, 'cliVersion').mockResolvedValue(null);
    render(<Probe min="1.6.0" />);
    const btn = await screen.findByRole('button', { name: 'probe' });
    await vi.waitFor(() => expect(api.cliVersion).toHaveBeenCalled());
    expect(btn).toBeEnabled();
  });
});
