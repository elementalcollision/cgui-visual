// Client-side validators for values typed into the 1.6/1.7 forms. They mirror
// the backend checks (runtime.rs) so the user gets the reason inline instead
// of a CLI error toast. Each returns an error message, or null when valid.

/** New image reference: `[host[:port]/]repo[:tag][@digest]`, lowercase repo. */
export function validateImageRef(ref: string): string | null {
  const r = ref.trim();
  if (!r) return 'Enter an image reference';
  if (r.startsWith('-')) return 'A reference must not start with "-"';
  if (r.length > 255) return 'A reference is at most 255 characters';
  if (/[\s\\\x00-\x1f]/.test(r)) return 'A reference must not contain spaces';
  const noDigest = r.split('@')[0];
  const i = noDigest.lastIndexOf(':');
  const hasTag = i >= 0 && !noDigest.slice(i).includes('/');
  const name = hasTag ? noDigest.slice(0, i) : noDigest;
  const tag = hasTag ? noDigest.slice(i + 1) : null;
  if (!name || name.startsWith('/') || name.endsWith('/')) return 'The reference has no repository name';
  if (!/^[a-z0-9._:/-]+$/.test(name)) return 'The repository name must be lowercase letters, digits, ".", "_", "-" and "/"';
  if (tag !== null && !/^[A-Za-z0-9._-]{1,128}$/.test(tag)) return 'Invalid tag (letters, digits, ".", "_", "-")';
  return null;
}

/** One DNS label: 1-63 chars, alphanumerics and inner hyphens. */
const DNS_LABEL = /^[a-zA-Z0-9]([a-zA-Z0-9-]{0,61}[a-zA-Z0-9])?$/;

/**
 * Network alias for `--network net,alias=NAME`: a hostname made of DNS
 * labels. The CLI qualifies it against the network's DNS domain, so a
 * single label or an FQDN are both fine.
 */
export function validateAlias(alias: string): string | null {
  const a = alias.trim();
  if (!a) return 'Enter an alias';
  if (a.length > 253) return 'An alias is at most 253 characters';
  const labels = a.replace(/\.$/, '').split('.');
  for (const l of labels) {
    if (!DNS_LABEL.test(l)) return `"${a}" is not a valid DNS name (labels are letters, digits and inner "-", up to 63 characters)`;
  }
  return null;
}

/** Split a comma/newline separated alias list, dropping blanks. */
export function splitList(text: string): string[] {
  return text.split(/[,\n]+/).map(s => s.trim()).filter(Boolean);
}

/** Volume name (same rule as containers: 2-63 chars, alphanumeric start). */
export function validateVolumeName(name: string): string | null {
  const n = name.trim();
  if (!n) return 'Enter a volume name';
  if (n.length < 2) return 'A volume name needs at least 2 characters';
  if (n.length > 63) return 'A volume name is at most 63 characters';
  if (!/^[a-zA-Z0-9][a-zA-Z0-9_.-]+$/.test(n)) return 'Use letters, digits, "_", "." and "-", starting with a letter or digit';
  return null;
}

/** Network name: lowercase `^[a-z0-9](?:[a-z0-9._-]{0,61}[a-z0-9])?$`. */
export function validateNetworkName(name: string): string | null {
  const n = name.trim();
  if (!n) return 'Enter a network name';
  if (!/^[a-z0-9](?:[a-z0-9._-]{0,61}[a-z0-9])?$/.test(n)) {
    return 'Use lowercase letters, digits, "." "_" "-" (up to 63 characters), starting and ending with a letter or digit';
  }
  return null;
}

/** Volume size: digits with an optional K/M/G/T/P suffix (empty = CLI default). */
export function validateSize(size: string): string | null {
  const s = size.trim();
  if (!s) return null;
  return /^[1-9][0-9]*[KkMmGgTtPp]?[Bb]?$/.test(s) ? null : 'Use a number with an optional suffix, e.g. 512M or 10G';
}

/** IPv4 CIDR such as 192.168.100.0/24 (empty = CLI default). */
export function validateCidr(cidr: string): string | null {
  const c = cidr.trim();
  if (!c) return null;
  const m = /^(\d{1,3})\.(\d{1,3})\.(\d{1,3})\.(\d{1,3})\/(\d{1,2})$/.exec(c);
  if (!m || m.slice(1, 5).some(o => Number(o) > 255) || Number(m[5]) > 32) return 'Use CIDR notation, e.g. 192.168.100.0/24';
  return null;
}

/** IPv6 prefix such as fd00:1234::/64 (empty = CLI default). */
export function validateCidr6(cidr: string): string | null {
  const c = cidr.trim();
  if (!c) return null;
  const m = /^([0-9a-fA-F:]+)\/(\d{1,3})$/.exec(c);
  if (!m || !m[1].includes(':') || Number(m[2]) > 128) return 'Use an IPv6 prefix, e.g. fd00:1234::/64';
  return null;
}

/** `key=value` label (value may be empty). */
export function validateLabel(label: string): string | null {
  const l = label.trim();
  return /^[^=\s]+=.*$/.test(l) ? null : 'Labels are key=value';
}
