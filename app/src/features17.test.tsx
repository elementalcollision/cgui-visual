// UI tests for the container 1.6 / 1.7 features: commit, aliases,
// create volume/network, machine options/logs/inspect.

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { CreateNetworkModal, CreateVolumeModal, DetailModal, MachineLogsModal, MachinesPanel, RunImageModal, parseInspect } from './modals';
import { TopBar } from './components';
import type { Tab } from './types';
import { getTheme } from './theme';
import { containers } from './fixtures';
import { api } from './api';
import { resetCliVersion } from './cliVersion';
import { splitList, validateAlias, validateCidr, validateCidr6, validateImageRef, validateLabel, validateNetworkName, validateSize, validateVolumeName } from './validate';
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

describe('create volume', () => {
  it('validates, calls the API with size and labels, then refreshes and closes', async () => {
    const user = userEvent.setup();
    const create = vi.spyOn(api, 'createVolume').mockResolvedValue(undefined);
    const onCreated = vi.fn();
    const onClose = vi.fn();
    render(<CreateVolumeModal t={t} onCreated={onCreated} onClose={onClose} />);
    const submit = screen.getByRole('button', { name: 'Create' });
    expect(submit).toBeDisabled(); // empty name

    await user.type(screen.getByLabelText('Name'), '-bad');
    expect(screen.getByLabelText('Name')).toBeInvalid();
    expect(submit).toBeDisabled();

    await user.clear(screen.getByLabelText('Name'));
    await user.type(screen.getByLabelText('Name'), 'mydata');
    await user.type(screen.getByLabelText(/Size/), 'lots');
    expect(screen.getByLabelText(/Size/)).toBeInvalid();
    expect(submit).toBeDisabled();
    await user.clear(screen.getByLabelText(/Size/));
    await user.type(screen.getByLabelText(/Size/), '10G');
    await user.type(screen.getByLabelText(/Labels/), 'team=infra');
    expect(submit).toBeEnabled();

    await user.click(submit);
    await waitFor(() => expect(create).toHaveBeenCalledWith({ name: 'mydata', size: '10G', labels: ['team=infra'] }));
    await waitFor(() => expect(onCreated).toHaveBeenCalled());
    expect(onClose).toHaveBeenCalled();
  });

  it('sends only the name when the optional fields are empty', async () => {
    const user = userEvent.setup();
    const create = vi.spyOn(api, 'createVolume').mockResolvedValue(undefined);
    render(<CreateVolumeModal t={t} onCreated={() => {}} onClose={() => {}} />);
    await user.type(screen.getByLabelText('Name'), 'plain');
    await user.keyboard('{Enter}'); // submitting the form works too
    await waitFor(() => expect(create).toHaveBeenCalledWith({ name: 'plain', size: undefined, labels: undefined }));
  });

  it('stays open and re-enables Create when the CLI fails', async () => {
    const user = userEvent.setup();
    vi.spyOn(api, 'createVolume').mockRejectedValue(new Error('volume already exists'));
    const onCreated = vi.fn();
    const onClose = vi.fn();
    render(<CreateVolumeModal t={t} onCreated={onCreated} onClose={onClose} />);
    await user.type(screen.getByLabelText('Name'), 'dup');
    await user.click(screen.getByRole('button', { name: 'Create' }));
    await waitFor(() => expect(screen.getByRole('button', { name: 'Create' })).toBeEnabled());
    expect(onCreated).not.toHaveBeenCalled();
    expect(onClose).not.toHaveBeenCalled();
  });
});

describe('create network', () => {
  it('rejects uppercase names and bad CIDRs, then submits every option', async () => {
    const user = userEvent.setup();
    const create = vi.spyOn(api, 'createNetwork').mockResolvedValue(undefined);
    const onCreated = vi.fn();
    render(<CreateNetworkModal t={t} onCreated={onCreated} onClose={() => {}} />);
    const submit = screen.getByRole('button', { name: 'Create' });

    await user.type(screen.getByLabelText('Name'), 'Backend');
    expect(screen.getByLabelText('Name')).toBeInvalid();
    expect(submit).toBeDisabled();
    await user.clear(screen.getByLabelText('Name'));
    await user.type(screen.getByLabelText('Name'), 'backend');

    await user.type(screen.getByLabelText(/IPv4 subnet/), '192.168.1.0');
    expect(screen.getByLabelText(/IPv4 subnet/)).toBeInvalid();
    expect(submit).toBeDisabled();
    await user.clear(screen.getByLabelText(/IPv4 subnet/));
    await user.type(screen.getByLabelText(/IPv4 subnet/), '192.168.100.0/24');

    await user.type(screen.getByLabelText(/IPv6 prefix/), 'fd00:1234::/64');
    await user.click(screen.getByLabelText(/Host-only/));
    await user.type(screen.getByLabelText(/Labels/), 'team=infra');
    expect(submit).toBeEnabled();

    await user.click(submit);
    await waitFor(() => expect(create).toHaveBeenCalledWith({
      name: 'backend', subnet: '192.168.100.0/24', subnetV6: 'fd00:1234::/64',
      internal: true, labels: ['team=infra'],
    }));
    await waitFor(() => expect(onCreated).toHaveBeenCalled());
  });

  it('sends just the name by default', async () => {
    const user = userEvent.setup();
    const create = vi.spyOn(api, 'createNetwork').mockResolvedValue(undefined);
    render(<CreateNetworkModal t={t} onCreated={() => {}} onClose={() => {}} />);
    await user.type(screen.getByLabelText('Name'), 'plain-net');
    await user.click(screen.getByRole('button', { name: 'Create' }));
    await waitFor(() => expect(create).toHaveBeenCalledWith({
      name: 'plain-net', subnet: undefined, subnetV6: undefined, internal: undefined, labels: undefined,
    }));
  });
});

describe('TopBar "New…" button', () => {
  const headings = Object.fromEntries(
    ['containers', 'images', 'volumes', 'networks', 'stacks', 'logs'].map(k => [k, { title: k, sub: '' }]),
  ) as Record<Tab, { title: string; sub: string }>;
  const bar = (tab: Tab, onNew: (() => void) | null) => (
    <TopBar tab={tab} t={t} search="" setSearch={() => {}} onPull={() => {}} onCollapse={() => {}}
            dark setDark={() => {}} onUpdate={null} headings={headings} onNew={onNew} />
  );

  it('labels the button per tab and calls onNew', async () => {
    const user = userEvent.setup();
    const onNew = vi.fn();
    const { rerender } = render(bar('volumes', onNew));
    await user.click(screen.getByRole('button', { name: /New volume/ }));
    expect(onNew).toHaveBeenCalledTimes(1);
    rerender(bar('networks', onNew));
    expect(screen.getByRole('button', { name: /New network/ })).toBeInTheDocument();
  });

  it('is absent when the tab has nothing to create', () => {
    render(bar('containers', null));
    expect(screen.queryByRole('button', { name: /New (volume|network)/ })).not.toBeInTheDocument();
  });
});

describe('create-form validators', () => {
  it('volume and network names follow the CLI rules', () => {
    expect(validateVolumeName('data_1.x')).toBeNull();
    expect(validateVolumeName('x'.repeat(255))).toBeNull();
    expect(validateVolumeName('x'.repeat(256))).not.toBeNull();
    expect(validateVolumeName('-x')).not.toBeNull();
    expect(validateNetworkName('my-net_1')).toBeNull();
    expect(validateNetworkName('a')).toBeNull();
    for (const bad of ['Up', '-x', 'x-', 'a b', 'a'.repeat(64), '']) {
      expect(validateNetworkName(bad), bad).not.toBeNull();
    }
  });

  it('sizes, CIDRs and labels', () => {
    for (const ok of ['', '512M', '10g', '1.5G', '2TB', '1048576']) expect(validateSize(ok), ok).toBeNull();
    for (const bad of ['G', 'abc', '-1G', '1G2']) expect(validateSize(bad), bad).not.toBeNull();
    expect(validateCidr('')).toBeNull();
    expect(validateCidr('10.0.0.0/8')).toBeNull();
    for (const bad of ['10.0.0.0', '10.0.0.0/33', '999.0.0.0/8', 'nope']) expect(validateCidr(bad), bad).not.toBeNull();
    expect(validateCidr6('fd00::/64')).toBeNull();
    for (const bad of ['fd00::', 'fd00::/129', '10.0.0.0/8']) expect(validateCidr6(bad), bad).not.toBeNull();
    expect(validateLabel('a=b')).toBeNull();
    expect(validateLabel('novalue')).not.toBeNull();
  });
});

describe('machines: user options, logs, inspect', () => {
  const machine = { id: 'dev1', status: 'running', cpus: 4, memory: 8 * 1024 ** 3, diskSize: 2 * 1024 ** 3, isDefault: true, createdDate: '' };
  const renderPanel = async () => {
    vi.spyOn(api, 'machineList').mockResolvedValue([machine]);
    render(<MachinesPanel t={t} />);
    await screen.findByText('dev1');
  };

  it('passes the user options to machineCreate', async () => {
    const user = userEvent.setup();
    vi.spyOn(api, 'cliVersion').mockResolvedValue('1.7.1');
    const create = vi.spyOn(api, 'machineCreate').mockResolvedValue(undefined);
    await renderPanel();
    await user.click(screen.getByRole('button', { name: 'New machine…' }));
    await user.type(screen.getByPlaceholderText('machine name'), 'dev2');
    await user.click(screen.getByRole('button', { name: /User options/ }));
    await user.type(screen.getByLabelText('Machine user'), 'dev');
    await user.type(screen.getByLabelText('Machine UID'), '501');
    await user.type(screen.getByLabelText('Machine GID'), '20');
    await user.type(screen.getByLabelText('Machine home'), '/home/dev');
    await user.click(screen.getByRole('button', { name: 'Create' }));
    await waitFor(() => expect(create).toHaveBeenCalledWith(
      'dev2', 'alpine:latest', undefined, undefined, true,
      { user: 'dev', uid: 501, gid: 20, home: '/home/dev' },
    ));
  });

  it('sends no user options by default, and blocks a relative home', async () => {
    const user = userEvent.setup();
    vi.spyOn(api, 'cliVersion').mockResolvedValue('1.7.1');
    const create = vi.spyOn(api, 'machineCreate').mockResolvedValue(undefined);
    await renderPanel();
    await user.click(screen.getByRole('button', { name: 'New machine…' }));
    await user.type(screen.getByPlaceholderText('machine name'), 'dev3');
    await user.click(screen.getByRole('button', { name: /User options/ }));
    await user.type(screen.getByLabelText('Machine home'), 'home/dev');
    expect(screen.getByLabelText('Machine home')).toBeInvalid();
    expect(screen.getByRole('button', { name: 'Create' })).toBeDisabled();
    await user.clear(screen.getByLabelText('Machine home'));
    await user.click(screen.getByRole('button', { name: 'Create' }));
    await waitFor(() => expect(create).toHaveBeenCalledWith('dev3', 'alpine:latest', undefined, undefined, true, undefined));
  });

  it('disables user options with a tooltip on a CLI older than 1.6', async () => {
    const user = userEvent.setup();
    vi.spyOn(api, 'cliVersion').mockResolvedValue('1.4.1');
    await renderPanel();
    await user.click(screen.getByRole('button', { name: 'New machine…' }));
    const toggle = screen.getByRole('button', { name: /User options/ });
    await waitFor(() => expect(toggle).toBeDisabled());
    expect(toggle).toHaveAttribute('title', 'Needs Apple container 1.6 or later');
  });

  it('opens the machine inspect JSON', async () => {
    const user = userEvent.setup();
    vi.spyOn(api, 'cliVersion').mockResolvedValue('1.7.1');
    const inspect = vi.spyOn(api, 'machineInspect').mockResolvedValue('[{"id":"dev1","userSetup":{"username":"dev"}}]');
    await renderPanel();
    await user.click(screen.getByRole('button', { name: 'Inspect' }));
    expect(await screen.findByText(/"username":"dev"/)).toBeInTheDocument();
    expect(inspect).toHaveBeenCalledWith('dev1');
  });

  it('shows machine logs, switches to the boot log, and changes the line count', async () => {
    const user = userEvent.setup();
    vi.spyOn(api, 'cliVersion').mockResolvedValue('1.7.1');
    const logs = vi.spyOn(api, 'machineLogs').mockImplementation(async (_n, o) => (o?.boot ? 'boot line 1' : 'stdio line 1'));
    await renderPanel();
    await user.click(screen.getByRole('button', { name: 'Logs' }));
    expect(await screen.findByText('stdio line 1')).toBeInTheDocument();
    expect(logs).toHaveBeenLastCalledWith('dev1', { boot: false, tail: 200 });
    await user.click(screen.getByRole('button', { name: 'Boot log' }));
    expect(await screen.findByText('boot line 1')).toBeInTheDocument();
    expect(logs).toHaveBeenLastCalledWith('dev1', { boot: true, tail: 200 });
    await user.selectOptions(screen.getByLabelText('Lines to show'), '1000');
    await waitFor(() => expect(logs).toHaveBeenLastCalledWith('dev1', { boot: true, tail: 1000 }));
  });

  it('explains a machine that has never booted instead of dumping the CLI error', async () => {
    vi.spyOn(api, 'machineLogs').mockRejectedValue(
      'failed to get logs for container machine dev1 (cause: The file “stdio.log” doesn’t exist.)');
    render(<MachineLogsModal t={t} name="dev1" onClose={() => {}} />);
    expect(await screen.findByText(/dev1 hasn't booted/)).toBeInTheDocument();
  });

  it('shows other log failures verbatim', async () => {
    vi.spyOn(api, 'machineLogs').mockRejectedValue('Plugins are unavailable');
    render(<MachineLogsModal t={t} name="dev1" onClose={() => {}} />);
    expect(await screen.findByText('Plugins are unavailable')).toBeInTheDocument();
  });
});
