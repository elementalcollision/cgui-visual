// UI tests for the container 1.6 / 1.7 features: commit, aliases,
// create volume/network, machine options/logs/inspect.

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { DetailModal, RunImageModal, parseInspect } from './modals';
import { getTheme } from './theme';
import { containers } from './fixtures';
import { api } from './api';
import { resetCliVersion } from './cliVersion';
import { splitList, validateAlias, validateImageRef } from './validate';
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

describe('DNS aliases', () => {
  const openAdvanced = async (user: ReturnType<typeof userEvent.setup>) => {
    await user.click(screen.getByRole('button', { name: /Advanced options/ }));
  };

  it('sends the alias list (and network) to runImage', async () => {
    const user = userEvent.setup();
    vi.spyOn(api, 'cliVersion').mockResolvedValue('1.7.1');
    const run = vi.spyOn(api, 'runImage').mockResolvedValue('id1');
    const onLaunched = vi.fn();
    render(<RunImageModal t={t} image="alpine" onLaunched={onLaunched} onClose={() => {}} />);
    await openAdvanced(user);
    await user.type(screen.getByPlaceholderText('default'), 'backend');
    await user.type(screen.getByLabelText('DNS aliases'), 'db.example.test, pg.example.test');
    await user.click(screen.getByRole('button', { name: 'Run' }));
    await waitFor(() => expect(run).toHaveBeenCalled());
    const args = run.mock.calls[0][0];
    expect(args.network).toBe('backend');
    expect(args.aliases).toEqual(['db.example.test', 'pg.example.test']);
  });

  it('omits aliases when the field is empty', async () => {
    const user = userEvent.setup();
    vi.spyOn(api, 'cliVersion').mockResolvedValue('1.7.1');
    const run = vi.spyOn(api, 'runImage').mockResolvedValue('id1');
    render(<RunImageModal t={t} image="alpine" onLaunched={() => {}} onClose={() => {}} />);
    await user.click(screen.getByRole('button', { name: 'Run' }));
    await waitFor(() => expect(run).toHaveBeenCalled());
    expect(run.mock.calls[0][0].aliases).toBeUndefined();
  });

  it('flags an invalid alias inline and blocks Run', async () => {
    const user = userEvent.setup();
    vi.spyOn(api, 'cliVersion').mockResolvedValue('1.7.1');
    const run = vi.spyOn(api, 'runImage').mockResolvedValue('id1');
    render(<RunImageModal t={t} image="alpine" onLaunched={() => {}} onClose={() => {}} />);
    await openAdvanced(user);
    await user.type(screen.getByLabelText('DNS aliases'), 'bad name.test');
    expect(screen.getByLabelText('DNS aliases')).toBeInvalid();
    expect(screen.getByText(/not a valid DNS name/)).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Run' })).toBeDisabled();
    expect(run).not.toHaveBeenCalled();
  });

  it('disables the field with a tooltip on a CLI older than 1.6', async () => {
    const user = userEvent.setup();
    vi.spyOn(api, 'cliVersion').mockResolvedValue('1.4.1');
    render(<RunImageModal t={t} image="alpine" onLaunched={() => {}} onClose={() => {}} />);
    await openAdvanced(user);
    const input = screen.getByLabelText('DNS aliases');
    await waitFor(() => expect(input).toBeDisabled());
    expect(input).toHaveAttribute('title', 'Needs Apple container 1.6 or later');
  });

  it('validateAlias / splitList', () => {
    expect(validateAlias('api.example.test')).toBeNull();
    expect(validateAlias('db')).toBeNull();
    expect(validateAlias('a,alias=b')).not.toBeNull();
    expect(validateAlias('-x.test')).not.toBeNull();
    expect(validateAlias('under_score.test')).not.toBeNull();
    expect(validateAlias('')).not.toBeNull();
    expect(splitList(' a.test,b.test\nc.test ,, ')).toEqual(['a.test', 'b.test', 'c.test']);
  });

  it('parseInspect reads aliases from Apple inspect output', () => {
    const payload = JSON.stringify([{
      configuration: {
        id: 'web',
        networks: [{ network: 'backend', options: { aliases: ['db.example.test', 'pg.example.test'], hostname: 'web', mtu: 1280 } }],
      },
      status: {
        state: 'running',
        networks: [{ network: 'backend', ipv4Address: '192.168.65.2/24', macAddress: 'f6:1c:44:d4:e9:27' }],
      },
    }]);
    const p = parseInspect(payload)!;
    expect(p.network).toEqual([{
      name: 'backend', ip: '192.168.65.2/24', mac: 'f6:1c:44:d4:e9:27',
      aliases: ['db.example.test', 'pg.example.test'],
    }]);
  });

  it('parseInspect shows a stopped container’s configured aliases without an address', () => {
    const payload = JSON.stringify([{
      configuration: { id: 'web', networks: [{ network: 'backend', options: { aliases: ['a.test'] } }] },
      status: { state: 'stopped', networks: [] },
    }]);
    const p = parseInspect(payload)!;
    expect(p.network[0]).toMatchObject({ name: 'backend', ip: '', aliases: ['a.test'] });
  });

  it('shows aliases in the detail modal Network tab', async () => {
    const user = userEvent.setup();
    vi.spyOn(api, 'cliVersion').mockResolvedValue('1.7.1');
    vi.spyOn(api, 'inspectContainer').mockResolvedValue(JSON.stringify([{
      configuration: { id: 'web', networks: [{ network: 'backend', options: { aliases: ['db.example.test'] } }] },
      status: { state: 'running', networks: [{ network: 'backend', ipv4Address: '192.168.65.2/24' }] },
    }]));
    render(<DetailModal item={containers.find(c => c.status === 'running')!} t={t} onClose={() => {}} onExec={() => {}} />);
    await waitFor(() => expect(screen.getByRole('button', { name: /^network/i })).toBeEnabled());
    await user.click(screen.getByRole('button', { name: /^network/i }));
    expect(await screen.findByText('db.example.test')).toBeInTheDocument();
    expect(screen.getByText('backend (aliases)')).toBeInTheDocument();
  });
});
