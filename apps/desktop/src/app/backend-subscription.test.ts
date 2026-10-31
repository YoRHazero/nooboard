import { copyReceived } from '../features/transfers/queries/useCopyReceivedMutation';
import { selectFiles } from '../features/transfers/queries/useSelectFilesMutation';
import { deleteHistory } from '../features/history/queries/useDeleteHistoryMutation';
import { configurePeer } from '../features/devices/queries/useConfigurePeerMutation';
import { copyHistory } from '../features/history/queries/useCopyHistoryMutation';
import { updateSyncSettings } from '../features/settings/queries/useUpdateSyncSettingsMutation';
import { afterEach, expect, it, vi } from 'vitest';
import { BackendSubscription } from './backend-subscription';
import { backendRequest, getBackendSnapshot, subscribeBackendSnapshot } from '../shared/backend';
import { useApplicationStore } from '../shared/store';
import { getPreferences } from '../features/settings/preferences';
import type { BackendTransport, Session } from '../shared/backend-contract';
import type { Frame, Reply, Request } from '../shared/contracts.generated';
import { snapshotFixture, hostFixture } from '../preview/scenarios/fixtures';
import { PreviewBackend } from '../preview/backend';
function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason: unknown) => void;
  const promise = new Promise<T>((a, b) => {
    resolve = a;
    reject = b;
  });
  return { promise, resolve, reject };
}
class Bridge implements BackendTransport {
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
  const runtime = new BackendSubscription(bridge);
  const events = vi.fn();
  runtime.projection.onEvent(events);
  const start = runtime.start();
  await vi.waitFor(() => expect(bridge.opens).toHaveLength(1));
  const snapshot = snapshotFixture();
  const newer = { ...snapshot, revision: '9007199254740993' };
  bridge.opens[0].receive({ type: 'snapshot', data: newer });
  bridge.ready(0, snapshot);
  await start;
  expect(getBackendSnapshot().revision).toBe(newer.revision);
  expect(events).not.toHaveBeenCalled();
  await runtime.dispose();
});
it('closes a stale connection and ignores its frames after a newer connection wins', async () => {
  const bridge = new Bridge();
  const runtime = new BackendSubscription(bridge);
  const start = runtime.start();
  await vi.waitFor(() => expect(bridge.opens).toHaveLength(1));
  const reconnect = runtime.connect();
  await vi.waitFor(() => expect(bridge.opens).toHaveLength(2));
  const second = bridge.ready(1);
  await reconnect;
  const first = bridge.ready(0);
  await start;
  bridge.opens[0].receive({ type: 'snapshot', data: snapshotFixture() });
  expect(first.close).toHaveBeenCalledOnce();
  expect(bridge.opens[0].signal.aborted).toBe(true);
  expect(getBackendSnapshot().session).toBe(second.initial.snapshot.session);
  await runtime.dispose();
  expect(second.close).toHaveBeenCalledOnce();
  expect(bridge.setupCleanup).toHaveBeenCalledOnce();
});
it('releases a connection that finishes after the owner is disposed', async () => {
  const bridge = new Bridge();
  const runtime = new BackendSubscription(bridge);
  const start = runtime.start();
  await vi.waitFor(() => expect(bridge.opens).toHaveLength(1));
  await runtime.dispose();
  const session = bridge.ready(0);
  await start;
  expect(session.close).toHaveBeenCalledOnce();
  expect(useApplicationStore.getState().connection.phase).toBe('disposed');
  await expect(backendRequest({ type: 'sendCurrent' }, 'sent')).rejects.toMatchObject({
    problem: { code: 'stopped' },
  });
});
it('rejects command results from an earlier core session', async () => {
  const bridge = new Bridge();
  const runtime = new BackendSubscription(bridge);
  const start = runtime.start();
  await vi.waitFor(() => expect(bridge.opens).toHaveLength(1));
  bridge.ready(0);
  await start;
  const reply = deferred<Reply>();
  bridge.request.mockImplementationOnce(() => reply.promise);
  const query = backendRequest(
    { type: 'queryHistory', data: { query: { contains: '', source: 'all', offset: 0 } } },
    'history',
  );
  bridge.opens[0].receive({ type: 'snapshot', data: snapshotFixture() });
  reply.resolve({ type: 'history', data: { items: [], hasMore: false } });
  await expect(query).rejects.toMatchObject({ problem: { code: 'stopped' } });
  await runtime.dispose();
});
it('latches a stopped stream until reconnect and retains the last snapshot for error UI', async () => {
  const bridge = new Bridge();
  const runtime = new BackendSubscription(bridge);
  const start = runtime.start();
  await vi.waitFor(() => expect(bridge.opens).toHaveLength(1));
  bridge.ready(0);
  await start;
  bridge.opens[0].receive({ type: 'stopped', data: { code: 'stopped' } });
  const before = getBackendSnapshot();
  bridge.opens[0].receive({ type: 'snapshot', data: snapshotFixture() });
  expect(getBackendSnapshot()).toBe(before);
  expect(useApplicationStore.getState().connection.phase).toBe('offline');
  await expect(backendRequest({ type: 'sendCurrent' }, 'sent')).rejects.toMatchObject({
    problem: { code: 'backendNotConnected' },
  });
  await runtime.dispose();
});
it('uses the same subscription and Query cache for preview and copying history never synthesizes an automatic send', async () => {
  const bridge = new PreviewBackend();
  const runtime = new BackendSubscription(bridge);
  await runtime.start();
  await updateSyncSettings([{ mode: 'automatic' }]);
  const before = getBackendSnapshot();
  const page = await backendRequest(
    { type: 'queryHistory', data: { query: { contains: '', source: 'all', offset: 0 } } },
    'history',
  );
  await copyHistory([page.data.items[1].id]);
  expect(getBackendSnapshot().current.text).toBe(page.data.items[1].text);
  expect(getBackendSnapshot().activities).toBe(before.activities);
  await runtime.dispose();
});
it('keeps appearance, host preferences and business requests separate', async () => {
  const bridge = new PreviewBackend();
  const spy = vi.spyOn(bridge, 'request');
  const runtime = new BackendSubscription(bridge);
  await runtime.start();
  spy.mockClear();
  await getPreferences().setAppearance({ theme: 'dark' });
  expect(spy).not.toHaveBeenCalled();
  await getPreferences().updateHost({ closeToTray: true });
  expect(spy).toHaveBeenLastCalledWith({
    type: 'hostPreferences',
    data: { patch: { closeToTray: true }, legacyLanguage: undefined },
  });
  await updateSyncSettings([{ paused: true }]);
  expect(spy).toHaveBeenLastCalledWith({
    type: 'updateSyncSettings',
    data: { patch: { paused: true } },
  });
  await runtime.dispose();
});
it('preserves independent peer patches, native dialog actions and full-width history IDs', async () => {
  const bridge = new PreviewBackend();
  const request = vi.spyOn(bridge, 'request');
  const runtime = new BackendSubscription(bridge);
  await runtime.start();
  const noobId = getBackendSnapshot().peers[0].noobId;
  await configurePeer([noobId, { autoSend: false }]);
  expect(request).toHaveBeenLastCalledWith({
    type: 'configurePeer',
    data: { noobId, patch: { autoSend: false, address: undefined } },
  });
  await configurePeer([noobId, { address: null }]);
  expect(request).toHaveBeenLastCalledWith({
    type: 'configurePeer',
    data: { noobId, patch: { autoSend: undefined, address: { type: 'clear' } } },
  });
  await deleteHistory(['9223372036854775807']);
  expect(request).toHaveBeenLastCalledWith({
    type: 'deleteHistory',
    data: { id: '9223372036854775807' },
  });
  await selectFiles([]);
  expect(request).toHaveBeenLastCalledWith({ type: 'selectFiles' });
  await copyReceived(['task-key']);
  expect(request).toHaveBeenLastCalledWith({ type: 'copyReceived', data: { key: 'task-key' } });
  await runtime.dispose();
});
it('starts the lifecycle owner once even when startup is requested concurrently', async () => {
  const bridge = new Bridge();
  const runtime = new BackendSubscription(bridge);
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
  const bridge = new PreviewBackend();
  const runtime = new BackendSubscription(bridge);
  await runtime.start();
  const changed = vi.fn();
  subscribeBackendSnapshot(changed);
  await bridge.sampleFiles();
  expect(changed).toHaveBeenCalled();
  changed.mockClear();
  await runtime.dispose();
  await vi.runAllTimersAsync();
  expect(changed).not.toHaveBeenCalled();
  expect(vi.getTimerCount()).toBe(0);
});
