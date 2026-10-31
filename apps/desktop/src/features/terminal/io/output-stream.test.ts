import { expect, it, vi } from 'vitest';
import type { TerminalRunView, TerminalTarget } from '../../../shared/types';
import type { Reply } from '../../../shared/contracts.generated';
import type { TerminalTransport } from '../../../shared/backend-contract';
import { terminalFixture } from '../../../preview/terminal';
import { createTerminalStore } from '../store';
import { TerminalStreams } from './output-stream';
import { TerminalBuffer } from './replay-buffer';
const run = (target: TerminalTarget): TerminalRunView => ({
  target,
  name: target.id,
  phase: 'running',
  readyMarkerReceived: false,
});
function pendingTransport() {
  let finish!: (reply: Reply) => void;
  let signal!: AbortSignal;
  const transport: TerminalTransport = {
    request: vi.fn(async (): Promise<Reply> => ({ type: 'done' })),
    open: vi.fn((_action, _receive, abort) => {
      signal = abort;
      return new Promise<Reply>((resolve) => {
        finish = resolve;
      });
    }),
  };
  return {
    transport,
    finish: (target: TerminalTarget) => finish({ type: 'terminalOpened', data: { target } }),
    aborted: () => signal.aborted,
  };
}
it('does not reveal a command or retain its stream when collapsed before its start reply arrives', async () => {
  const workspace = terminalFixture();
  const ui = createTerminalStore();
  const pending = pendingTransport();
  const io = new TerminalStreams(
    pending.transport,
    () => true,
    () => 64,
    ui,
    () => workspace,
  );
  const target: TerminalTarget = { kind: 'command', id: 'run' };
  const opening = io.start({ kind: 'command', id: 'config' });
  workspace.runs = [run(target)];
  io.reconcile(workspace);
  ui.getState().collapse(target, workspace);
  pending.finish(target);
  await opening;
  expect(ui.getState().selected).toBeNull();
  expect(ui.getState().collapsed).toContain('command:run');
  expect(io.buffer(target)).toBeUndefined();
  expect(pending.aborted()).toBe(true);
  expect(pending.transport.request).not.toHaveBeenCalled();
  io.dispose();
});
it('an attachment finishing late does not take focus from another terminal', async () => {
  const workspace = terminalFixture();
  const first: TerminalTarget = { kind: 'terminal', id: 'first' };
  const second: TerminalTarget = { kind: 'terminal', id: 'second' };
  workspace.runs = [run(first), run(second)];
  const ui = createTerminalStore();
  const pending = pendingTransport();
  const io = new TerminalStreams(
    pending.transport,
    () => true,
    () => 64,
    ui,
    () => workspace,
  );
  io.reconcile(workspace);
  ui.getState().select(first);
  const attaching = io.attach(first);
  ui.getState().select(second);
  pending.finish(first);
  await attaching;
  expect(ui.getState().selected).toEqual(second);
  expect(io.buffer(first)).toBeDefined();
  io.dispose();
  expect(pending.aborted()).toBe(true);
});
it('an ended event received before the start reply cannot resurrect an output subscription', async () => {
  const workspace = terminalFixture();
  const ui = createTerminalStore();
  const pending = pendingTransport();
  const io = new TerminalStreams(
    pending.transport,
    () => true,
    () => 64,
    ui,
    () => workspace,
  );
  const opening = io.startLocal('shell');
  const target: TerminalTarget = { kind: 'terminal', id: 'ended' };
  io.ended({ kind: target.kind, id: target.id, name: 'shell', result: 'exited', cleanup: [] });
  pending.finish(target);
  await opening;
  expect(io.buffer(target)).toBeUndefined();
  expect(ui.getState().pending).toEqual([]);
  expect(pending.aborted()).toBe(true);
  io.dispose();
});
it('waits for renderer consumption and keeps only the bounded replay window', async () => {
  const buffer = new TerminalBuffer('subscription', new AbortController(), () => 3);
  let release!: () => void;
  const observed = buffer.observe(
    () =>
      new Promise<void>((resolve) => {
        release = resolve;
      }),
  );
  let acknowledged = false;
  const received = buffer
    .accept({ type: 'data', data: { offset: '0', sequence: '1', bytes: [1, 2, 3, 4, 5] } })
    .then(() => {
      acknowledged = true;
    });
  await Promise.resolve();
  expect(acknowledged).toBe(false);
  release();
  await received;
  observed.unsubscribe();
  const replay = buffer.observe(() => {});
  expect([...replay.bytes]).toEqual([3, 4, 5]);
  expect(replay.offset).toBe(2n);
  buffer.dispose();
  expect(buffer.abort.signal.aborted).toBe(true);
});
