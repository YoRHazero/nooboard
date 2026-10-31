import { expect, it, vi } from 'vitest';
import { PreviewTerminalFiles } from './terminal-files';
import { PreviewTerminal } from './terminal';
import type { Frame } from '../shared/contracts.generated';
it('applies nested filesystem changes and rejects nonexistent directories', () => {
  const files = new PreviewTerminalFiles();
  files.add('/home/demo/new', true);
  files.add('/home/demo/new/child', true);
  files.rename('/home/demo/new', '/home/demo/renamed');
  expect(files.browse('/home/demo/renamed').entries[0].name).toBe('child');
  expect(files.browse('/home/demo/renamed/child').entries).toEqual([]);
  files.remove('/home/demo/renamed');
  expect(() => files.browse('/home/demo/renamed/child')).toThrow();
  expect(() => files.browse('/missing')).toThrow();
});
it('emits authoritative file completion and cancellation results, not just disappearing snapshots', async () => {
  vi.useFakeTimers();
  const frames: Frame[] = [];
  let current: import('../shared/contracts.generated').TerminalWorkspaceView | undefined;
  const preview = new PreviewTerminal(
    (workspace) => {
      current = workspace;
    },
    (frame) => frames.push(frame),
  );
  try {
    await preview.request({
      type: 'upload',
      data: { id: 'test', directory: '/home/demo', folder: false, conflict: 'fail' },
    });
    expect(current!.files).toHaveLength(1);
    await vi.advanceTimersByTimeAsync(3000);
    expect(frames.at(-1)).toMatchObject({
      type: 'terminalEnded',
      data: { kind: 'file', result: 'succeeded' },
    });
    await preview.request({
      type: 'upload',
      data: { id: 'test', directory: '/home/demo', folder: false, conflict: 'fail' },
    });
    await vi.advanceTimersByTimeAsync(3000);
    expect(frames.at(-1)).toMatchObject({ type: 'terminalEnded', data: { result: 'failed' } });
    await preview.request({
      type: 'download',
      data: { id: 'test', paths: ['/home/demo/notes.txt'], conflict: 'fail' },
    });
    await preview.request({ type: 'stopFiles', data: { id: current!.files[0].id } });
    await vi.advanceTimersByTimeAsync(3000);
    expect(current!.files).toEqual([]);
    expect(frames.at(-1)).toMatchObject({ type: 'terminalEnded', data: { result: 'stopped' } });
  } finally {
    preview.dispose();
    vi.useRealTimers();
  }
});
