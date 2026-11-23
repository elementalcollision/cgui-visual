// Tabs available in the inspect drawer. Each maps to a different
// projection of the inspect JSON; "raw" always falls back to the verbatim
// blob so users can still grep odd fields the parser hasn't extracted.
export type InspectTab = 'trends' | 'env' | 'mounts' | 'network' | 'ports' | 'health' | 'raw';

export interface ParsedInspect {
  env: { key: string; value: string }[];
  mounts: { source: string; destination: string; type: string; readOnly?: boolean }[];
  network: { name: string; ip: string; mac?: string; aliases?: string[] }[];
  ports: { container: string; host: string }[];
  health?: { status: string; failingStreak?: number; log: { exitCode: number; output: string }[] };
}

// Permissive inspect-JSON parser. Apple's `container inspect` output isn't
// 100% docker-compatible, so we look at both Docker's PascalCase keys and
// Apple's camelCase variants. Missing fields are silently skipped — the
// Raw tab is the safety net.
export function parseInspect(text: string): ParsedInspect | null {
  let root: any;
  try {
    const v = JSON.parse(text);
    // `container inspect` returns an array; `docker inspect` returns either.
    root = Array.isArray(v) ? v[0] : v;
  } catch {
    return null;
  }
  if (!root || typeof root !== 'object') return null;

  const config = root.Config ?? root.config ?? {};
  const ns = root.NetworkSettings ?? root.networkSettings ?? {};
  const hostConfig = root.HostConfig ?? root.hostConfig ?? {};
  const stateBlock = root.State ?? root.state ?? {};

  // ── Env: ["KEY=value", ...] split once on first '='. Keys without '='
  //         become bare keys with empty value rather than getting dropped.
  const envList: string[] = config.Env ?? config.env ?? [];
  const env = envList.map((line: string) => {
    const i = line.indexOf('=');
    return i === -1 ? { key: line, value: '' } : { key: line.slice(0, i), value: line.slice(i + 1) };
  });

  // ── Mounts: docker shape is {Source,Destination,Type,RW}; apple variant
  //         exposes camelCase. Read-only is RW=false (docker) or
  //         readOnly=true (apple).
  const rawMounts: any[] = root.Mounts ?? root.mounts ?? [];
  const mounts = rawMounts.map((m: any) => ({
    source: m.Source ?? m.source ?? '',
    destination: m.Destination ?? m.destination ?? '',
    type: m.Type ?? m.type ?? '',
    readOnly: typeof m.RW === 'boolean' ? !m.RW : (m.readOnly === true ? true : undefined),
  }));

  // ── Network: docker NetworkSettings.Networks is { netName: { IPAddress } };
  //         older payloads have just IPAddress at top level.
  const networks: { name: string; ip: string; mac?: string; aliases?: string[] }[] = [];
  const dockerNets = ns.Networks ?? ns.networks;
  // Apple `container inspect`: `configuration.networks[]` holds the per-network
  // options (DNS `aliases`, hostname, mtu; aliases need container 1.6+) and
  // `status.networks[]` the live address once running.
  const appleCfgNets: any[] = Array.isArray(root.configuration?.networks) ? root.configuration.networks : [];
  const appleStatusNets: any[] = Array.isArray(root.status?.networks) ? root.status.networks : [];
  const cleanAliases = (a: unknown): string[] | undefined =>
    Array.isArray(a) && a.length ? a.filter((x): x is string => typeof x === 'string') : undefined;
  if (dockerNets && typeof dockerNets === 'object') {
    for (const [name, info] of Object.entries(dockerNets as Record<string, any>)) {
      networks.push({
        name,
        ip: info?.IPAddress ?? info?.ipAddress ?? '',
        mac: info?.MacAddress ?? info?.macAddress,
        aliases: cleanAliases(info?.Aliases ?? info?.aliases),
      });
    }
  } else if (appleCfgNets.length || appleStatusNets.length) {
    const names = [...new Set([
      ...appleCfgNets.map(n => n?.network),
      ...appleStatusNets.map(n => n?.network),
    ].filter((n): n is string => typeof n === 'string'))];
    for (const name of names) {
      const cfg = appleCfgNets.find(n => n?.network === name);
      const live = appleStatusNets.find(n => n?.network === name);
      networks.push({
        name,
        ip: live?.ipv4Address ?? '',
        mac: live?.macAddress,
        aliases: cleanAliases(cfg?.options?.aliases),
      });
    }
  } else if (ns.IPAddress || ns.ipAddress) {
    networks.push({
      name: 'default',
      ip: ns.IPAddress ?? ns.ipAddress ?? '',
      mac: ns.MacAddress ?? ns.macAddress,
    });
  }

  // ── Ports: docker shape is { "8080/tcp": [{ HostPort: "8080" }, …] }.
  //         A port with a null mapping is "exposed but not published".
  const ports: { container: string; host: string }[] = [];
  const portMap = ns.Ports ?? ns.ports ?? config.ExposedPorts ?? config.exposedPorts ?? {};
  for (const [containerPort, bindings] of Object.entries(portMap as Record<string, any>)) {
    if (Array.isArray(bindings) && bindings.length > 0) {
      for (const b of bindings) {
        const hp = b?.HostPort ?? b?.hostPort ?? '';
        const hi = b?.HostIp ?? b?.hostIp ?? '';
        ports.push({ container: containerPort, host: hi ? `${hi}:${hp}` : hp });
      }
    } else {
      ports.push({ container: containerPort, host: '— (exposed)' });
    }
  }

  // ── Health: only present when the image declared a HEALTHCHECK. The
  //         log entries are kept short (last 3) since the modal is small.
  const healthBlock = stateBlock.Health ?? stateBlock.health;
  let health: ParsedInspect['health'];
  if (healthBlock && typeof healthBlock === 'object') {
    const log = (healthBlock.Log ?? healthBlock.log ?? []).slice(-3).map((entry: any) => ({
      exitCode: entry?.ExitCode ?? entry?.exitCode ?? 0,
      output: entry?.Output ?? entry?.output ?? '',
    }));
    health = {
      status: healthBlock.Status ?? healthBlock.status ?? 'unknown',
      failingStreak: healthBlock.FailingStreak ?? healthBlock.failingStreak,
      log,
    };
  }

  // Suppress unused-warning when restart policy parsing is added later;
  // we keep the read here so the dependency is documented.
  void hostConfig;

  return { env, mounts, network: networks, ports, health };
}

// ─── Image layer inspector (A7) ───────────────────────────────────────
//
// Apple's `container image inspect` returns the OCI image config, which
// includes a `history[]` array describing each build step (Dockerfile
// instruction → resulting layer). We surface that as a "Layers" view
// alongside the raw JSON. Sizes aren't in the config — they live in the
// manifest — so the layer rows show command + age + empty-layer flag and
// the diff_id when available, but `size` stays optional.

export interface ImageLayer {
  /** Wall-clock timestamp the layer was created. ISO-8601 from the OCI
   *  config; empty when the build step is missing it. */
  created: string;
  /** The build command (Dockerfile RUN/COPY/CMD/etc.) that produced this
   *  layer. Often prefixed with `/bin/sh -c #(nop)` for non-RUN steps. */
  createdBy: string;
  /** Optional comment shown next to the command. */
  comment?: string;
  /** OCI calls a step "empty" when no filesystem diff was produced
   *  (typical of CMD / ENV / LABEL). We render those dim. */
  emptyLayer?: boolean;
  /** sha256 diff id from rootfs.diff_ids. Aligned to non-empty layers in
   *  history order; undefined when the alignment is ambiguous. */
  diffId?: string;
}

export interface ParsedImage {
  layers: ImageLayer[];
  digest?: string;
  os?: string;
  architecture?: string;
}

// Pull a layer summary out of an `image inspect` JSON payload. Tolerant
// to docker (PascalCase) vs apple/OCI (camelCase) and to the top-level
// array form `[{ ... }]`. Returns null on un-parseable input so the
// caller can fall back to a raw-JSON view.
export function parseImageInspect(text: string): ParsedImage | null {
  let root: any;
  try {
    const v = JSON.parse(text);
    root = Array.isArray(v) ? v[0] : v;
  } catch {
    return null;
  }
  if (!root || typeof root !== 'object') return null;

  const history: any[] = root.history ?? root.History ?? [];
  const diffIds: string[] = root?.rootfs?.diff_ids ?? root?.RootFS?.DiffIDs ?? [];

  // Walk history; for every non-empty step, claim the next diff id off
  // the queue. Empty steps don't consume a diff id (per OCI spec).
  let dq = 0;
  const layers: ImageLayer[] = history.map((h: any) => {
    const empty = h.empty_layer === true || h.emptyLayer === true;
    const diffId = empty ? undefined : diffIds[dq];
    if (!empty) dq++;
    return {
      created: h.created ?? h.Created ?? '',
      createdBy: h.created_by ?? h.createdBy ?? h.CreatedBy ?? '',
      comment: h.comment ?? h.Comment ?? undefined,
      emptyLayer: empty || undefined,
      diffId,
    };
  });

  return {
    layers,
    digest: root.config?.digest ?? root.Id ?? root.id,
    os: root.os ?? root.Os,
    architecture: root.architecture ?? root.Architecture,
  };
}

// Squash leading `/bin/sh -c #(nop) ` so the visible command focuses on
// the Dockerfile instruction. Falls back to the full string when the
// prefix isn't present.
export function prettyCmd(s: string): string {
  return s.replace(/^\/bin\/sh -c #\(nop\)\s*/, '').trim() || s;
}
