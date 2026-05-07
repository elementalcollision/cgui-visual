import { describe, it, expect } from 'vitest';
import { stripAnsi } from './views';

describe('stripAnsi', () => {
  it('removes simple SGR colour codes', () => {
    // bold + green hello + reset
    expect(stripAnsi('[1;32mhello[0m')).toBe('hello');
  });

  it('removes cursor-movement and erase-line sequences', () => {
    // CSI 2K = erase line, CSI G = move cursor to column 1
    expect(stripAnsi('foo[2K[Gbar')).toBe('foobar');
  });

  it('removes OSC sequences (terminal title etc.)', () => {
    // ESC ] 0 ; title BEL — common terminal-title set sequence
    expect(stripAnsi('keep]0;some titleme')).toBe('keepme');
  });

  it('passes through plain text untouched', () => {
    expect(stripAnsi('plain log line 42')).toBe('plain log line 42');
  });
});
