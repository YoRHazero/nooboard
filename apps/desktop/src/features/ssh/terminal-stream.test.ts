import { expect, it, vi } from 'vitest';
import { TerminalStream } from './terminal-stream';
it('merges replay and overlapping live bytes without losing split UTF-8 or escape sequences', () => {
  const chunks: Uint8Array[] = [];
  const stream = new TerminalStream((b) => chunks.push(b), vi.fn());
  const bytes = [...new TextEncoder().encode('你\x1b[32m好')];
  stream.replay({ start: '0', end: '2', gap: false, bytes: bytes.slice(0, 2) });
  expect(stream.accept('0', bytes.slice(0, 4))).toBe(true);
  expect(stream.accept('4', bytes.slice(4))).toBe(true);
  expect(new TextDecoder().decode(Uint8Array.from(chunks.flatMap((b) => [...b])))).toBe(
    '你\x1b[32m好',
  );
  expect(stream.offset).toBe(BigInt(bytes.length));
});
it('requires replay on missing output and marks bounded-buffer truncation explicitly', () => {
  const gap = vi.fn();
  const write = vi.fn();
  const stream = new TerminalStream(write, gap);
  expect(stream.accept('9007199254740993', [65])).toBe(false);
  expect(write).not.toHaveBeenCalled();
  stream.replay({ start: '9007199254740993', end: '9007199254740994', gap: true, bytes: [65] });
  expect(gap).toHaveBeenCalledOnce();
  expect(stream.offset).toBe(9007199254740994n);
});
