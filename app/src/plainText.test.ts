import { describe, it, expect } from 'vitest';
import { plainText, plainError } from './plainText';
import { errMessage } from './loadState';

describe('plainText', () => {
  it('drops backtick markers', () => {
    expect(plainText('`container stop web` timed out')).toBe('container stop web timed out');
  });
  it('leaves plain text alone', () => {
    expect(plainText('nothing to do')).toBe('nothing to do');
  });
});

describe('plainError', () => {
  it('cleans string rejections', () => {
    expect(plainError('failed to spawn `container ls`')).toBe('failed to spawn container ls');
  });
  it('cleans Error messages in place', () => {
    const e = plainError(new Error('`registry login` timed out')) as Error;
    expect(e.message).toBe('registry login timed out');
  });
  it('passes through non-errors', () => {
    expect(plainError(42)).toBe(42);
  });
});

describe('errMessage', () => {
  it('strips backticks from displayed errors', () => {
    expect(errMessage('`container clean` requires 1.4')).toBe('container clean requires 1.4');
    expect(errMessage(new Error('`x` failed'))).toBe('x failed');
  });
});
