// UI tests for the container 1.6 / 1.7 features: commit, aliases,
// create volume/network, machine options/logs/inspect.

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { DetailModal } from './modals';
import { getTheme } from './theme';
import { containers } from './fixtures';
import { api } from './api';
import { resetCliVersion } from './cliVersion';
import { validateImageRef } from './validate';
import { defaultCommitRef } from './containerActions';

const t = getTheme(true);

beforeEach(() => { resetCliVersion(); });
afterEach(() => { vi.restoreAllMocks(); });

describe('commit to image', () => {
  const running = containers.find(c => c.status === 'running')!;

  it('prompts for a reference, commits, and refreshes the images list', async () => {
    const user = userEvent.setup();
    vi.spyOn(api, 'cliVersion').mockResolvedValue('1.7.1');
    vi.spyOn(api, 'inspectContainer').mockResolvedValue('{}');
    const prompt = vi.spyOn(window, 'prompt').mockReturnValue('myapp:snap1');
    const commit = vi.spyOn(api, 'commitContainer').mockResolvedValue('myapp:snap1');
    const changed = vi.fn();
    render(<DetailModal item={running} t={t} onClose={() => {}} onExec={() => {}} onImagesChanged={changed} />);
    await user.click(screen.getByRole('button', { name: 'Commit to image…' }));
    await waitFor(() => expect(commit).toHaveBeenCalledWith(running.id, 'myapp:snap1'));
    expect(prompt.mock.calls[0][1]).toBe(defaultCommitRef(running.name));
    await waitFor(() => expect(changed).toHaveBeenCalled());
  });

  it('does nothing when the prompt is cancelled', async () => {
    const user = userEvent.setup();
    vi.spyOn(api, 'cliVersion').mockResolvedValue('1.7.1');
    vi.spyOn(api, 'inspectContainer').mockResolvedValue('{}');
    vi.spyOn(window, 'prompt').mockReturnValue(null);
    const commit = vi.spyOn(api, 'commitContainer').mockResolvedValue('x');
    render(<DetailModal item={running} t={t} onClose={() => {}} onExec={() => {}} />);
    await user.click(screen.getByRole('button', { name: 'Commit to image…' }));
    expect(commit).not.toHaveBeenCalled();
  });

  it('re-prompts with the reason when the reference is invalid', async () => {
    const user = userEvent.setup();
    vi.spyOn(api, 'cliVersion').mockResolvedValue('1.7.1');
    vi.spyOn(api, 'inspectContainer').mockResolvedValue('{}');
    const prompt = vi.spyOn(window, 'prompt').mockReturnValueOnce('Bad Ref').mockReturnValueOnce('good:1');
    const commit = vi.spyOn(api, 'commitContainer').mockResolvedValue('good:1');
    render(<DetailModal item={running} t={t} onClose={() => {}} onExec={() => {}} />);
    await user.click(screen.getByRole('button', { name: 'Commit to image…' }));
    await waitFor(() => expect(commit).toHaveBeenCalledWith(running.id, 'good:1'));
    expect(prompt).toHaveBeenCalledTimes(2);
    expect(String(prompt.mock.calls[1][0])).toMatch(/spaces/);
  });

  it('is disabled with a tooltip on a CLI older than 1.6', async () => {
    vi.spyOn(api, 'cliVersion').mockResolvedValue('1.5.2');
    vi.spyOn(api, 'inspectContainer').mockResolvedValue('{}');
    render(<DetailModal item={running} t={t} onClose={() => {}} onExec={() => {}} />);
    const btn = screen.getByRole('button', { name: 'Commit to image…' });
    await waitFor(() => expect(btn).toBeDisabled());
    expect(btn).toHaveAttribute('title', 'Needs Apple container 1.6 or later');
  });
});

describe('validateImageRef / defaultCommitRef', () => {
  it('accepts ordinary references and rejects mistakes', () => {
    expect(validateImageRef('myapp:latest')).toBeNull();
    expect(validateImageRef('localhost:5000/team/app:1.0')).toBeNull();
    expect(validateImageRef('')).not.toBeNull();
    expect(validateImageRef('-x')).not.toBeNull();
    expect(validateImageRef('My App')).not.toBeNull();
    expect(validateImageRef('MyApp:1')).not.toBeNull();
    expect(validateImageRef('app:')).not.toBeNull();
  });

  it('turns a container name into a valid default reference', () => {
    for (const n of ['web', 'My_Web.App', 'api server', '--weird--', '']) {
      expect(validateImageRef(defaultCommitRef(n))).toBeNull();
    }
    expect(defaultCommitRef('Web')).toBe('web:latest');
  });
});
