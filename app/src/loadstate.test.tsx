// Error / empty states: a CLI failure must never look like "nothing exists",
// and a failed or missing Trivy scan must never look clean.

import { describe, it, expect, vi, afterEach } from 'vitest';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { ContainersView, ImagesView, NetworksView, StacksView, VolumesView } from './views';
import { TrivyModal } from './modals';
import { api } from './api';
import { getTheme } from './theme';
import { looksLikeServicesDown } from './loadState';
import * as fixtures from './fixtures';

const t = getTheme(true);

const SERVICES_DOWN =
  'Error: Plugins are unavailable. Start the container system services and retry:\n\n    container system start';

afterEach(() => { vi.restoreAllMocks(); });

describe('looksLikeServicesDown', () => {
  it('recognises the CLI text and ignores other failures', () => {
    expect(looksLikeServicesDown(SERVICES_DOWN)).toBe(true);
    expect(looksLikeServicesDown('`container image ls` timed out after 30s')).toBe(false);
  });
});

describe('list views when the runtime is down', () => {
  it('ImagesView shows "services aren\'t running" with a working Start services button', async () => {
    const user = userEvent.setup();
    const list = vi.spyOn(api, 'listImagesChecked').mockRejectedValue(SERVICES_DOWN);
    const start = vi.spyOn(api, 'systemStart').mockResolvedValue(undefined);
    render(<ImagesView t={t} search="" onScan={() => {}} onRun={() => {}} onInspect={() => {}} />);

    expect(await screen.findByText("Container services aren't running")).toBeInTheDocument();
    // Failure, not emptiness.
    expect(screen.queryByText(/No images/)).not.toBeInTheDocument();

    list.mockResolvedValue({ images: fixtures.images, warning: null });
    await user.click(screen.getByRole('button', { name: /Start services/ }));
    expect(start).toHaveBeenCalledTimes(1);
    // After starting, the view reloads and renders the list.
    expect(await screen.findByText('mlcommons/inference:llama2-70b')).toBeInTheDocument();
    expect(screen.queryByText("Container services aren't running")).not.toBeInTheDocument();
  });

  it('detects stopped services via system_status when the message is generic', async () => {
    vi.spyOn(api, 'listVolumes').mockRejectedValue(new Error('exited 1: connection interrupted'));
    vi.spyOn(api, 'systemStatus').mockResolvedValue(false);
    render(<VolumesView t={t} search="" onInspect={() => {}} />);
    expect(await screen.findByText("Container services aren't running")).toBeInTheDocument();
    expect(screen.getByRole('button', { name: /Start services/ })).toBeInTheDocument();
  });

  it('shows the CLI message (no Start button) for other failures', async () => {
    vi.spyOn(api, 'listNetworks').mockRejectedValue(new Error('`container network ls` timed out after 30s'));
    vi.spyOn(api, 'systemStatus').mockResolvedValue(true);
    render(<NetworksView t={t} search="" onInspect={() => {}} />);
    expect(await screen.findByText(/timed out after 30s/)).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: /Start services/ })).not.toBeInTheDocument();
    expect(screen.getByRole('button', { name: /Retry/ })).toBeInTheDocument();
    expect(screen.queryByText(/No networks/)).not.toBeInTheDocument();
  });

  it('ContainersView renders the services-down state from the poll error', async () => {
    render(
      <ContainersView
        t={t} search="" selected="" setSelected={() => {}} onInspect={() => {}} onLogs={() => {}}
        containers={[]} loadError={SERVICES_DOWN} onRetry={() => {}}
      />,
    );
    expect(await screen.findByText("Container services aren't running")).toBeInTheDocument();
    expect(screen.getByRole('button', { name: /Start services/ })).toBeInTheDocument();
    expect(screen.queryByText(/No containers/)).not.toBeInTheDocument();
  });
});

describe('empty states', () => {
  it('ImagesView says "No images" when the list is genuinely empty', async () => {
    vi.spyOn(api, 'listImagesChecked').mockResolvedValue({ images: [], warning: null });
    render(<ImagesView t={t} search="" onScan={() => {}} onRun={() => {}} onInspect={() => {}} />);
    expect(await screen.findByText(/No images/)).toBeInTheDocument();
    expect(screen.queryByText("Container services aren't running")).not.toBeInTheDocument();
    expect(screen.queryByRole('alert')).not.toBeInTheDocument();
  });

  it('ContainersView says "No containers" once loaded with an empty list', () => {
    render(
      <ContainersView
        t={t} search="" selected="" setSelected={() => {}} onInspect={() => {}} onLogs={() => {}}
        containers={[]} loaded
      />,
    );
    expect(screen.getByText(/No containers/)).toBeInTheDocument();
    expect(screen.queryByRole('alert')).not.toBeInTheDocument();
  });

  it('ContainersView does not claim "No containers" before the first poll', () => {
    render(
      <ContainersView
        t={t} search="" selected="" setSelected={() => {}} onInspect={() => {}} onLogs={() => {}}
        containers={[]} loaded={false}
      />,
    );
    expect(screen.queryByText(/No containers/)).not.toBeInTheDocument();
  });
});

describe('StacksView', () => {
  it('lists invalid stack files as non-fatal error rows next to valid stacks', async () => {
    vi.spyOn(api, 'listStacks').mockResolvedValue({
      stacks: fixtures.stacks.slice(0, 1),
      invalid: [{ file: '/cfg/stacks/typo.toml', name: 'typo', error: 'line 2: invalid basic string' }],
    });
    render(<StacksView t={t} search="" onGraph={() => {}} />);
    expect(await screen.findByText('invalid stack file typo: line 2: invalid basic string')).toBeInTheDocument();
    expect(screen.getByText(fixtures.stacks[0].name)).toBeInTheDocument();
  });

  it('shows the failing services when stack down partially fails', async () => {
    const user = userEvent.setup();
    vi.spyOn(api, 'listStacks').mockResolvedValue({ stacks: fixtures.stacks.slice(0, 1), invalid: [] });
    vi.spyOn(window, 'confirm').mockReturnValue(true);
    vi.spyOn(api, 'stackDown').mockRejectedValue(
      "stack 'x' down incomplete: 1 of 2 service(s) failed: web (stop: boom)\n$ container stop x_web\n✗ web: stop failed: boom",
    );
    render(<StacksView t={t} search="" onGraph={() => {}} />);
    await screen.findByText(fixtures.stacks[0].name);
    await user.click(screen.getByRole('button', { name: /Down/ }));
    // Step log from the error body lands in the stack's log panel.
    expect(await screen.findByText('✗ web: stop failed: boom')).toBeInTheDocument();
    // The summary line is a toast (not rendered here), not part of the log.
    expect(screen.queryByText(/down incomplete/)).not.toBeInTheDocument();
  });
});

describe('TrivyModal scan states', () => {
  it('shows "Scan failed" with the message, never a clean result', async () => {
    vi.spyOn(api, 'scanImage').mockResolvedValue({ status: 'failed', message: 'trivy exited with 1: FATAL db error' });
    render(<TrivyModal t={t} image="alpine" onClose={() => {}} />);
    expect(await screen.findByText('Scan failed')).toBeInTheDocument();
    expect(screen.getByText(/FATAL db error/)).toBeInTheDocument();
    expect(screen.queryByText(/no vulnerabilities found/i)).not.toBeInTheDocument();
    expect(screen.queryByRole('button', { name: /^CRITICAL/ })).not.toBeInTheDocument();
  });

  it('shows "not installed" with the install hint', async () => {
    vi.spyOn(api, 'scanImage').mockResolvedValue({ status: 'notInstalled' });
    render(<TrivyModal t={t} image="alpine" onClose={() => {}} />);
    expect(await screen.findByText('Trivy is not installed')).toBeInTheDocument();
    expect(screen.getByText('brew install trivy')).toBeInTheDocument();
    expect(screen.queryByText(/no vulnerabilities found/i)).not.toBeInTheDocument();
  });

  it('treats a rejected invoke as a failed scan', async () => {
    vi.spyOn(api, 'scanImage').mockRejectedValue(new Error('ipc broke'));
    render(<TrivyModal t={t} image="alpine" onClose={() => {}} />);
    expect(await screen.findByText('Scan failed')).toBeInTheDocument();
    expect(screen.getByText(/ipc broke/)).toBeInTheDocument();
  });

  it('shows an explicit clean state only for a completed scan with zero findings', async () => {
    vi.spyOn(api, 'scanImage').mockResolvedValue({
      status: 'ok',
      result: { image: 'alpine', counts: { CRITICAL: 0, HIGH: 0, MEDIUM: 0, LOW: 0 }, findings: [] },
    });
    render(<TrivyModal t={t} image="alpine" onClose={() => {}} />);
    expect(await screen.findByText(/no vulnerabilities found/i)).toBeInTheDocument();
    expect(screen.queryByText('Scan failed')).not.toBeInTheDocument();
  });

  it('retries a failed scan', async () => {
    const user = userEvent.setup();
    const scan = vi.spyOn(api, 'scanImage')
      .mockResolvedValueOnce({ status: 'failed', message: 'timed out' })
      .mockResolvedValue({ status: 'ok', result: fixtures.trivy });
    render(<TrivyModal t={t} image="alpine" onClose={() => {}} />);
    await user.click(await screen.findByRole('button', { name: 'Retry' }));
    expect(await screen.findByText(/CVE-2024-21626/)).toBeInTheDocument();
    expect(scan).toHaveBeenCalledTimes(2);
  });
});

describe('partial image listing (container >= 1.6)', () => {
  it('ImagesView lists readable images and shows a non-blocking warning banner', async () => {
    vi.spyOn(api, 'listImagesChecked').mockResolvedValue({
      images: fixtures.images,
      warning: "2 images couldn't be read: Error: failed to read 2 image(s)",
    });
    render(<ImagesView t={t} search="" onScan={() => {}} onRun={() => {}} onInspect={() => {}} />);
    expect(await screen.findByText('mlcommons/inference:llama2-70b')).toBeInTheDocument();
    const banner = screen.getByTestId('image-warning');
    expect(banner).toHaveTextContent(/2 images couldn't be read/);
    // Not the blocking error state.
    expect(screen.queryByRole('alert')).not.toBeInTheDocument();
    expect(screen.queryByRole('button', { name: /Start services/ })).not.toBeInTheDocument();
  });

  it('ImagesView shows no banner when nothing was skipped', async () => {
    vi.spyOn(api, 'listImagesChecked').mockResolvedValue({ images: fixtures.images, warning: null });
    render(<ImagesView t={t} search="" onScan={() => {}} onRun={() => {}} onInspect={() => {}} />);
    await screen.findByText('mlcommons/inference:llama2-70b');
    expect(screen.queryByTestId('image-warning')).not.toBeInTheDocument();
  });
});
