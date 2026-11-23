// parseInspect against real Apple `container inspect` (1.7.1) output.
// Env, mounts and published ports live under `configuration`, not in
// Docker's Config.Env / Mounts / NetworkSettings.Ports.

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { parseInspect } from './inspect';
import { DetailModal } from './DetailModal';
import { getTheme } from '../theme';
import { containers } from '../fixtures';
import { api } from '../api';
import { resetCliVersion } from '../cliVersion';

// `container run -d --name inspect-check -p 8081:80 -e FOO=bar -v /tmp:/host-tmp
//   -v inspect-check-vol:/data docker.io/library/python:3.12-slim python3 -m http.server 80`
// then `container inspect inspect-check` (home directory shortened).
const appleRunning = [{
  configuration: {
    capAdd: [], capDrop: [],
    creationDate: '2026-11-23T13:23:33Z',
    dns: { nameservers: [], options: [], searchDomains: [] },
    id: 'inspect-check',
    image: {
      descriptor: {
        digest: 'sha256:ec948fa5f90f4f8907e89f4800cfd2d2e91e391a4bce4a6afa77ba265bc3a2fe',
        mediaType: 'application/vnd.oci.image.index.v1+json', size: 10373,
      },
      reference: 'docker.io/library/python:3.12-slim',
    },
    initProcess: {
      arguments: ['-m', 'http.server', '80'],
      environment: [
        'LANG=C.UTF-8',
        'PATH=/usr/local/bin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin',
        'PYTHON_SHA256=c08bc65a81971c1dd5783182826503369466c7e67374d1646519adf05207b684',
        'GPG_KEY=7169605F62C751356D054A26A821E680E5FA6305',
        'FOO=bar',
        'PYTHON_VERSION=3.12.13',
      ],
      executable: 'python3', rlimits: [], supplementalGroups: [], terminal: false,
      user: { id: { gid: 0, uid: 0 } }, workingDirectory: '/',
    },
    labels: {},
    mounts: [
      { destination: '/host-tmp', options: [], source: '/tmp', type: { virtiofs: {} } },
      {
        destination: '/data', options: [],
        source: '/Users/me/Library/Application Support/com.apple.container/volumes/inspect-check-vol/volume.img',
        type: { volume: { cache: { on: {} }, format: 'ext4', name: 'inspect-check-vol', sync: { fsync: {} } } },
      },
    ],
    networks: [{ network: 'default', options: { hostname: 'inspect-check', mtu: 1280 } }],
    platform: { architecture: 'arm64', os: 'linux', variant: 'v8' },
    publishedPorts: [{ containerPort: 80, count: 1, hostAddress: '0.0.0.0', hostPort: 8081, proto: 'tcp' }],
    publishedSockets: [], readOnly: false,
    resources: { cpuOverhead: 1, cpus: 4, memoryInBytes: 1073741824 },
    rosetta: false, runtimeHandler: 'container-runtime-linux', ssh: false, sysctls: {}, useInit: false, virtualization: false,
  },
  id: 'inspect-check',
  status: {
    networks: [{
      hostname: 'inspect-check', ipv4Address: '192.168.64.200/24', ipv4Gateway: '192.168.64.1',
      ipv6Address: 'fdea:57c1:ba3e:6cbe:f8ec:9eff:fe99:fe89/64', macAddress: 'fa:ec:9e:99:fe:89',
      mtu: 1280, network: 'default', variant: 'reserved',
    }],
    startedDate: '2026-11-23T13:23:33Z',
    state: 'running',
  },
}];

// `container create -p 127.0.0.1:9000-9002:7000-7002/udp -p 8082:81 -v /tmp:/ro-tmp:ro
//   --tmpfs /scratch --mount type=bind,source=/tmp,target=/ro2,readonly …` (excerpt).
const appleShapes = [{
  configuration: {
    id: 'inspect-check-shapes',
    mounts: [
      { destination: '/scratch', options: [], source: 'tmpfs', type: { tmpfs: {} } },
      { destination: '/ro-tmp', options: ['ro'], source: '/tmp', type: { virtiofs: {} } },
      { destination: '/ro2', options: ['ro'], source: '/tmp', type: { virtiofs: {} } },
    ],
    publishedPorts: [
      { containerPort: 7000, count: 3, hostAddress: '127.0.0.1', hostPort: 9000, proto: 'udp' },
      { containerPort: 81, count: 1, hostAddress: '0.0.0.0', hostPort: 8082, proto: 'tcp' },
    ],
  },
  id: 'inspect-check-shapes',
  status: { networks: [], state: 'stopped' },
}];

describe('parseInspect: Apple container inspect', () => {
  const p = parseInspect(JSON.stringify(appleRunning))!;

  it('reads env from configuration.initProcess.environment', () => {
    expect(p.env).toHaveLength(6);
    expect(p.env).toContainEqual({ key: 'FOO', value: 'bar' });
    // Split on the first '=' only.
    expect(p.env).toContainEqual({
      key: 'PATH', value: '/usr/local/bin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin',
    });
  });

  it('reads a bind mount and a named volume from configuration.mounts', () => {
    expect(p.mounts).toEqual([
      { source: '/tmp', destination: '/host-tmp', type: 'bind', readOnly: undefined },
      // A named volume shows its name, not the backing volume.img path.
      { source: 'inspect-check-vol', destination: '/data', type: 'volume', readOnly: undefined },
    ]);
  });

  it('reads a published port in the list view’s host:container form', () => {
    expect(p.ports).toEqual([{ container: '8081:80/tcp', host: '0.0.0.0' }]);
  });

  it('still reads the network from configuration/status', () => {
    expect(p.network).toEqual([
      { name: 'default', ip: '192.168.64.200/24', mac: 'fa:ec:9e:99:fe:89', aliases: undefined },
    ]);
  });

  it('marks read-only mounts, labels tmpfs, and expands port ranges', () => {
    const s = parseInspect(JSON.stringify(appleShapes))!;
    expect(s.mounts).toEqual([
      { source: 'tmpfs', destination: '/scratch', type: 'tmpfs', readOnly: undefined },
      { source: '/tmp', destination: '/ro-tmp', type: 'bind', readOnly: true },
      { source: '/tmp', destination: '/ro2', type: 'bind', readOnly: true },
    ]);
    expect(s.ports).toEqual([
      { container: '9000-9002:7000-7002/udp', host: '127.0.0.1' },
      { container: '8082:81/tcp', host: '0.0.0.0' },
    ]);
    expect(s.env).toEqual([]);
  });

  it('skips malformed published ports instead of throwing', () => {
    const s = parseInspect(JSON.stringify([{
      configuration: { publishedPorts: [{ hostPort: '8080', containerPort: 80 }, null, { hostPort: 8080, containerPort: 80 }] },
    }]))!;
    expect(s.ports).toEqual([{ container: '8080:80/tcp', host: '' }]);
  });
});

describe('DetailModal tabs for an Apple container', () => {
  const t = getTheme(true);
  beforeEach(() => { resetCliVersion(); });
  afterEach(() => { vi.restoreAllMocks(); });

  it('enables Env, Mounts and Ports and shows their rows', async () => {
    const user = userEvent.setup();
    vi.spyOn(api, 'cliVersion').mockResolvedValue('1.7.1');
    vi.spyOn(api, 'inspectContainer').mockResolvedValue(JSON.stringify(appleRunning));
    render(<DetailModal item={containers.find(c => c.status === 'running')!} t={t} onClose={() => {}} onExec={() => {}} />);

    await waitFor(() => expect(screen.getByRole('button', { name: /^env/i })).toBeEnabled());
    expect(screen.getByRole('button', { name: /^mounts/i })).toBeEnabled();
    expect(screen.getByRole('button', { name: /^ports/i })).toBeEnabled();

    await user.click(screen.getByRole('button', { name: /^env/i }));
    expect(await screen.findByText('FOO')).toBeInTheDocument();
    expect(screen.getByText('bar')).toBeInTheDocument();

    await user.click(screen.getByRole('button', { name: /^mounts/i }));
    expect(await screen.findByText('inspect-check-vol')).toBeInTheDocument();
    expect(screen.getByText('/host-tmp')).toBeInTheDocument();

    await user.click(screen.getByRole('button', { name: /^ports/i }));
    expect(await screen.findByText('8081:80/tcp')).toBeInTheDocument();
    expect(screen.getByText('0.0.0.0')).toBeInTheDocument();
  });
});
