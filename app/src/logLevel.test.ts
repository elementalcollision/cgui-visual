import { describe, it, expect } from 'vitest';
import { classifyLevel, stripAnsi } from './logLevel';

const E = '\x1b[31m';
const Y = '\x1b[33m';
const R = '\x1b[0m';

const cases: Array<[string, string, 'info' | 'warn' | 'error']> = [
  // bracketed tags
  ['[ERROR]', 'x [ERROR] boom', 'error'],
  ['[ERR]', '[ERR] boom', 'error'],
  ['[FATAL]', '[FATAL] boom', 'error'],
  ['[CRIT]', '[CRIT] boom', 'error'],
  ['[CRITICAL]', '[CRITICAL] boom', 'error'],
  ['[PANIC]', '[PANIC] boom', 'error'],
  ['[error] nginx', '2026/11/23 12:00:00 [error] 29#29: *1 connect() failed', 'error'],
  ['[WARN]', '[WARN] slow', 'warn'],
  ['[WARNING]', '[WARNING] slow', 'warn'],
  ['[warn] nginx', '2026/11/23 12:00:00 [warn] 29#29: low on buffers', 'warn'],
  // upper-case words
  ['ISO ERROR', '2026-11-23T12:09:11Z ERROR upstream timeout', 'error'],
  ['ERR', '12:00 ERR failed', 'error'],
  ['FATAL', 'FATAL: out of memory', 'error'],
  ['CRIT', 'CRIT disk full', 'error'],
  ['CRITICAL', 'CRITICAL disk full', 'error'],
  ['PANIC', 'PANIC: runtime error', 'error'],
  ['WARN padded', 'WARN  slow request', 'warn'],
  ['WARNING', '12:00 WARNING cache cold', 'warn'],
  // key=value
  ['level=error', 'time=x level=error msg="boom"', 'error'],
  ['level=ERROR', 'level=ERROR msg=boom', 'error'],
  ['lvl=crit', 'lvl=crit msg=boom', 'error'],
  ['severity=ERROR', 'severity=ERROR something', 'error'],
  ['level="fatal"', 'level="fatal" msg=x', 'error'],
  ['level=warn', 'level=warn msg=slow', 'warn'],
  ['level=warning', 'level=warning msg=slow', 'warn'],
  ["lvl='warn'", "lvl='warn' msg=slow", 'warn'],
  ['severity=WARNING', 'severity=WARNING x', 'warn'],
  // JSON
  ['json error', '{"level":"error","msg":"boom"}', 'error'],
  ['json ERROR spaced', '{"ts":1, "level" : "ERROR", "m":"x"}', 'error'],
  ['json severity', '{"severity":"critical","m":"x"}', 'error'],
  ['json lvl', '{"lvl":"panic"}', 'error'],
  ['json warn', '{"level":"warn","msg":"slow"}', 'warn'],
  ['json severity WARNING', '{"severity":"WARNING"}', 'warn'],
  // Python logging
  ['python ERROR:', 'ERROR:root:something failed', 'error'],
  ['python CRITICAL:', 'CRITICAL:app:down', 'error'],
  ['python WARNING:', 'WARNING:root:careful', 'warn'],
  // klog / glog
  ['klog E', 'E1123 12:00:00.000000       1 main.go:42] failed', 'error'],
  ['klog W', 'W1123 12:00:00.000000       1 main.go:42] hmm', 'warn'],
  ['klog I', 'I1123 12:00:00.000000       1 main.go:42] ok', 'info'],
  // ANSI-coloured
  ['ansi ERROR', `2026-11-23 ${E}ERROR${R} upstream timeout`, 'error'],
  ['ansi [ERROR]', `${E}[ERROR]${R} boom`, 'error'],
  ['ansi WARN', `${Y}WARN${R}  slow`, 'warn'],
  ['ansi level=error', `level=${E}error${R} msg=x`, 'error'],
  ['ansi klog', `${E}E1123 12:00:00.000000${R} x`, 'error'],
  ['ansi split in word', `${E}ERR${R}OR`, 'error'],
  // precedence
  ['error beats warn', 'WARN retrying after ERROR', 'error'],
  ['warn then error key', 'level=warn prev level=error', 'error'],
  // info / false positives
  ['plain', 'server listening on :8080', 'info'],
  ['prose errors', 'no errors found', 'info'],
  ['error_count=0', 'stats error_count=0 warn_count=0', 'info'],
  ['warning prose', 'note: warning: none in the middle of a sentence', 'info'],
  ['error prose', 'an error occurred earlier but recovered', 'info'],
  ['ERROR_COUNT upper', 'ERROR_COUNT=0', 'info'],
  ['ERR_ glued', 'ERR_CONNECTION_REFUSED handled', 'info'],
  ['word embedded', 'TERROR and WARNING_LEVEL', 'info'],
  ['level=info', 'level=info msg="error handling enabled"', 'info'],
  ['json info', '{"level":"info","msg":"error budget ok"}', 'info'],
  ['klog not at start', 'x E1123 12:00:00 foo', 'info'],
  ['E word', 'E2E tests passed 1123 12:00:00', 'info'],
  ['empty', '', 'info'],
];

describe('classifyLevel', () => {
  it.each(cases)('%s', (_name, line, expected) => {
    expect(classifyLevel(line)).toBe(expected);
  });
});

describe('stripAnsi', () => {
  it('removes colour codes', () => {
    expect(stripAnsi(`${E}hi${R}`)).toBe('hi');
  });
});
