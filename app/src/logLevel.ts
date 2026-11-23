// Log-line severity classification for the Logs view level pills.
// Runs per line at render time, so every pattern is precompiled once.

export type LineLevel = 'info' | 'warn' | 'error';

// Strip CSI / OSC ANSI escape sequences. Covers \x1b[…m colour codes,
// cursor movement, and the 8-bit CSI variant some shells emit. Used
// when the user opts into "no colour" rendering, on download, and before
// classifying a line (colour codes would otherwise split level tokens).
// Not a full state machine but enough for log streams.
// eslint-disable-next-line no-control-regex
const ANSI_RE = /\x1B(?:\[[0-?]*[ -/]*[@-~]|\][^\x07\x1B]*(?:\x07|\x1B\\))/g;

export function stripAnsi(s: string): string {
  return s.replace(ANSI_RE, '');
}

const ERR_WORDS = 'ERROR|ERR|FATAL|CRIT|CRITICAL|PANIC';
const WARN_WORDS = 'WARN|WARNING';
const ERR_VALUES = 'error|err|fatal|crit|critical|panic';
const WARN_VALUES = 'warn|warning';

// Standalone upper-case token: not glued to letters, digits or underscores
// (so ERROR_COUNT, TERROR and ERR_CONNECTION_REFUSED don't match).
const word = (alts: string) => `(?<![A-Za-z0-9_])(?:${alts})(?![A-Za-z0-9_])`;
// [ERROR] style tags; case-insensitive because nginx etc. emit [error].
const bracket = (alts: string) => `\\[(?:${alts})\\]`;
// level=error / lvl="warn" / severity=ERROR (key=value, value quoted or not).
const kv = (vals: string) => `(?<![A-Za-z0-9_])(?:level|lvl|severity)=["']?(?:${vals})(?![A-Za-z0-9_])`;
// {"level":"warn"} / "severity": "ERROR"
const json = (vals: string) => `"(?:level|severity|lvl)"\\s*:\\s*"(?:${vals})"`;

// Case-insensitive structural positions (brackets, key=value, JSON key).
const ERROR_CI = new RegExp(`${bracket(ERR_WORDS)}|${kv(ERR_VALUES)}|${json(ERR_VALUES)}`, 'i');
const WARN_CI = new RegExp(`${bracket(WARN_WORDS)}|${kv(WARN_VALUES)}|${json(WARN_VALUES)}`, 'i');
// Case-sensitive bare upper-case words (also covers Python `ERROR:` /
// `WARNING:` prefixes); lower-case prose never matches.
const ERROR_WORD = new RegExp(word(ERR_WORDS));
const WARN_WORD = new RegExp(word(WARN_WORDS));
// klog / glog: `E1123 12:00:00.000000 …` / `W1123 12:00:00 …` at line start.
const KLOG_ERROR = /^E\d{4}\s+\d{2}:\d{2}:\d{2}/;
const KLOG_WARN = /^W\d{4}\s+\d{2}:\d{2}:\d{2}/;

export function classifyLevel(raw: string): LineLevel {
  const line = stripAnsi(raw);
  if (ERROR_WORD.test(line) || ERROR_CI.test(line) || KLOG_ERROR.test(line)) return 'error';
  if (WARN_WORD.test(line) || WARN_CI.test(line) || KLOG_WARN.test(line)) return 'warn';
  return 'info';
}
