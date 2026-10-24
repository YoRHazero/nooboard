import { afterEach, expect, it, vi } from 'vitest';
import { createDesktop } from './api';
import type { DesktopBridge, Session } from './bridge/port';
import type { Frame, Reply, Request } from './bridge/generated';
import { snapshotFixture, hostFixture } from '../preview/scenarios/fixtures';
import { PreviewBridge } from '../preview/bridge';
function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason: unknown) => void;
  const promise = new Promise<T>((a, b) => {
    resolve = a;
    reject = b;
  });
  return { promise, resolve, reject };
}
class Bridge implements DesktopBridge {
  readonly mode = 'preview';
  opens: {
    receive(frame: Frame): void;
    signal: AbortSignal;
    pending: ReturnType<typeof deferred<Session>>;
  }[] = [];
  dispose = vi.fn();
  setupCleanup = vi.fn();
  async setup() {
    return this.setupCleanup;
  }
  async locale() {
    return 'en';
  }
  request = vi.fn(async (request: Request): Promise<Reply> =>
    request.type === 'hostPreferences' ? { type: 'host', data: hostFixture() } : { type: 'done' },
  );
  connect(receive: (frame: Frame) => void, signal: AbortSignal) {
    const pending = deferred<Session>();
    this.opens.push({ receive, signal, pending });
    return pending.promise;
  }
  ready(index: number, snapshot = snapshotFixture()) {
    const session = {
      initial: { snapshot, host: hostFixture(), diagnostic: false },
      close: vi.fn(async () => {}),
    };
    this.opens[index].pending.resolve(session);
    return session;
  }
}
afterEach(() => vi.useRealTimers());
it('takes a frame arriving before connect returns and suppresses historical cues', async () => {
  const bridge = new Bridge();
  const { runtime, desktop } = createDesktop(bridge);
  const events = vi.fn();
  desktop.onEvent(events);
  const start = runtime.start();
  await vi.waitFor(() => expect(bridge.opens).toHaveLength(1));
  const snapshot = snapshotFixture();
  const newer = { ...snapshot, revision: '9007199254740993' };
  bridge.opens[0].receive({ type: 'snapshot', data: newer });
  bridge.ready(0, snapshot);
  await start;
  expect(desktop.getSnapshot().revision).toBe(newer.revision);
  expect(events).not.toHaveBeenCalled();
  await runtime.dispose();
});
it('closes a stale connection and ignores its frames after a newer connection wins', async () => {
  const bridge = new Bridge();
  const { runtime, desktop } = createDesktop(bridge);
  const start = runtime.start();
  await vi.waitFor(() => expect(bridge.opens).toHaveLength(1));
  const reconnect = desktop.reconnect();
  await vi.waitFor(() => expect(bridge.opens).toHaveLength(2));
  const second = bridge.ready(1);
  await reconnect;
  const first = bridge.ready(0);
  await start;
  bridge.opens[0].receive({ type: 'snapshot', data: snapshotFixture() });
  expect(first.close).toHaveBeenCalledOnce();
  expect(bridge.opens[0].signal.aborted).toBe(true);
  expect(desktop.getSnapshot().session).toBe(second.initial.snapshot.session);
  await runtime.dispose();
  expect(second.close).toHaveBeenCalledOnce();
  expect(bridge.setupCleanup).toHaveBeenCalledOnce();
});
it('releases a connection that finishes after the owner is disposed', async () => {
  const bridge = new Bridge();
  const { runtime, desktop } = createDesktop(bridge);
  const start = runtime.start();
  await vi.waitFor(() => expect(bridge.opens).toHaveLength(1));
  await runtime.dispose();
  const session = bridge.ready(0);
  await start;
  expect(session.close).toHaveBeenCalledOnce();
  expect(desktop.getConnection().phase).toBe('disposed');
  await expect(desktop.sendCurrent()).rejects.toMatchObject({ problem: { code: 'stopped' } });
});
it('rejects command results from an earlier core session', async () => {
  const bridge = new Bridge();
  const { runtime, desktop } = createDesktop(bridge);
  const start = runtime.start();
  await vi.waitFor(() => expect(bridge.opens).toHaveLength(1));
  bridge.ready(0);
  await start;
  const reply = deferred<Reply>();
  bridge.request.mockImplementationOnce(() => reply.promise);
  const query = desktop.queryHistory({ contains: '', source: 'all', offset: 0 });
  bridge.opens[0].receive({ type: 'snapshot', data: snapshotFixture() });
  reply.resolve({ type: 'history', data: { items: [], hasMore: false } });
  await expect(query).rejects.toMatchObject({ problem: { code: 'stopped' } });
  await runtime.dispose();
});
it('latches a stopped stream until reconnect and retains the last snapshot for error UI', async () => {
  const bridge = new Bridge();
  const { runtime, desktop } = createDesktop(bridge);
  const start = runtime.start();
  await vi.waitFor(() => expect(bridge.opens).toHaveLength(1));
  bridge.ready(0);
  await start;
  bridge.opens[0].receive({ type: 'stopped', data: { code: 'stopped' } });
  const before = desktop.getSnapshot();
  bridge.opens[0].receive({ type: 'snapshot', data: snapshotFixture() });
  expect(desktop.getSnapshot()).toBe(before);
  expect(desktop.getConnection().phase).toBe('offline');
  await expect(desktop.sendCurrent()).rejects.toMatchObject({
    problem: { code: 'backendNotConnected' },
  });
  await runtime.dispose();
});
it('uses the same runtime for preview and copying history never synthesizes an automatic send', async () => {
  const bridge = new PreviewBridge();
  const { runtime, desktop } = createDesktop(bridge);
  await runtime.start();
  await desktop.updateSyncSettings({ mode: 'automatic' });
  const before = desktop.getSnapshot();
  const page = await desktop.queryHistory({ contains: '', source: 'all', offset: 0 });
  await desktop.copyHistory(page.items[1].id);
  expect(desktop.getSnapshot().current.text).toBe(page.items[1].text);
  expect(desktop.getSnapshot().activities).toBe(before.activities);
  await runtime.dispose();
});
it('keeps appearance, host preferences and business requests separate', async () => {
  const bridge = new PreviewBridge();
  const spy = vi.spyOn(bridge, 'request');
  const { runtime, desktop } = createDesktop(bridge);
  await runtime.start();
  spy.mockClear();
  await desktop.updateAppearance({ theme: 'dark' });
  expect(spy).not.toHaveBeenCalled();
  await desktop.updateHostPreferences({ closeToTray: true });
  expect(spy).toHaveBeenLastCalledWith({
    type: 'hostPreferences',
    data: { patch: { closeToTray: true }, legacyLanguage: undefined },
  });
  await desktop.updateSyncSettings({ paused: true });
  expect(spy).toHaveBeenLastCalledWith({
    type: 'updateSyncSettings',
    data: { patch: { paused: true } },
  });
  await runtime.dispose();
});
it('preserves independent peer patches, native dialog actions and full-width history IDs', async () => {
  const bridge = new PreviewBridge();
  const request = vi.spyOn(bridge, 'request');
  const { runtime, desktop } = createDesktop(bridge);
  await runtime.start();
  const noobId = desktop.getSnapshot().peers[0].noobId;
  await desktop.configurePeer(noobId, { autoSend: false });
  expect(request).toHaveBeenLastCalledWith({
    type: 'configurePeer',
    data: { noobId, patch: { autoSend: false, address: undefined } },
  });
  await desktop.configurePeer(noobId, { address: null });
  expect(request).toHaveBeenLastCalledWith({
    type: 'configurePeer',
    data: { noobId, patch: { autoSend: undefined, address: { type: 'clear' } } },
  });
  await desktop.deleteHistory('9223372036854775807');
  expect(request).toHaveBeenLastCalledWith({
    type: 'deleteHistory',
    data: { id: '9223372036854775807' },
  });
  await desktop.selectFiles();
  expect(request).toHaveBeenLastCalledWith({ type: 'selectFiles' });
  await desktop.copyReceived('task-key');
  expect(request).toHaveBeenLastCalledWith({ type: 'copyReceived', data: { key: 'task-key' } });
  await runtime.dispose();
});
it('starts the lifecycle owner once even when startup is requested concurrently', async () => {
  const bridge = new Bridge();
  const { runtime } = createDesktop(bridge);
  const first = runtime.start();
  const second = runtime.start();
  await vi.waitFor(() => expect(bridge.opens).toHaveLength(1));
  bridge.ready(0);
  await Promise.all([first, second]);
  await runtime.start();
  expect(bridge.opens).toHaveLength(1);
  await runtime.dispose();
});
it('disposes preview scenario timers and event subscriptions', async () => {
  vi.useFakeTimers();
  const bridge = new PreviewBridge();
  const { runtime, desktop } = createDesktop(bridge);
  await runtime.start();
  const changed = vi.fn();
  desktop.subscribe(changed);
  await bridge.sampleFiles();
  expect(changed).toHaveBeenCalled();
  changed.mockClear();
  await runtime.dispose();
  await vi.runAllTimersAsync();
  expect(changed).not.toHaveBeenCalled();
  expect(vi.getTimerCount()).toBe(0);
});
