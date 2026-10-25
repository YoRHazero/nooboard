import { expect, it, vi } from 'vitest';
import { SnapshotStore } from './store';
import { snapshotFixture, hostFixture } from '../../preview/scenarios/fixtures';
const appearance = { theme: 'light' as const, reducedMotion: false };
it('orders full-width revisions and keeps stable activity identity and unchanged branches', () => {
  const store = new SnapshotStore(appearance);
  const host = hostFixture();
  const frame = snapshotFixture();
  frame.revision = '9007199254740992';
  store.apply(frame, host);
  const initial = store.getSnapshot();
  expect(store.apply({ ...frame, revision: '9007199254740991' }, host)).toBe(false);
  expect(store.getSnapshot()).toBe(initial);
  const next = structuredClone(frame);
  next.revision = '9007199254740993';
  next.settings.paused = true;
  store.apply(next, host);
  expect(store.getSnapshot().activities).toBe(initial.activities);
  expect(store.getSnapshot().localDevice).toBe(initial.localDevice);
  expect(store.getSnapshot().settings.paused).toBe(true);
});
it('suppresses recovery cues while preserving real subsequent events and resets on a new session', () => {
  const store = new SnapshotStore(appearance);
  const host = hostFixture();
  const frame = snapshotFixture();
  const events = vi.fn();
  store.onEvent(events);
  store.apply(frame, host);
  frame.revision = '2';
  frame.activities.unshift({
    sequence: '18446744073709551614',
    kind: 'Copied',
    summary: 'hidden',
    at: 0,
  });
  store.apply(frame, host, true);
  expect(events).not.toHaveBeenCalled();
  frame.revision = '3';
  frame.activities.unshift({
    sequence: '18446744073709551615',
    kind: 'Copied',
    summary: 'visible',
    at: 1,
  });
  store.apply(frame, host);
  expect(events).toHaveBeenLastCalledWith({
    type: 'copied',
    sequence: `${frame.session}:18446744073709551615`,
  });
  store.apply(snapshotFixture(), host);
  expect(events).toHaveBeenLastCalledWith({ type: 'reset' });
});
it('derives delivery results from receipts and preserves one activity identity', () => {
  const store = new SnapshotStore(appearance);
  const host = hostFixture();
  const frame = snapshotFixture();
  const id = { session: frame.session, sequence: '7' };
  frame.transfers = [
    {
      id,
      automatic: false,
      bytes: 4,
      targets: [{ noobId: 'peer', deviceName: 'remote', state: 'Sending' }],
    },
  ];
  frame.activities = [{ sequence: '7', messageId: id, kind: 'Sent', summary: 'hello', at: 0 }];
  store.apply(frame, host);
  const activityId = store.getSnapshot().activities[0].id;
  expect(store.getSnapshot().activities[0].state).toBe('pending');
  frame.revision = '2';
  frame.transfers[0].targets[0].state = 'Applied';
  store.apply(frame, host);
  expect(store.getSnapshot().activities[0]).toMatchObject({ id: activityId, state: 'applied' });
});
it('does not convert byte completion into a successful file receipt or mutate mailbox nodes', () => {
  const store = new SnapshotStore(appearance);
  const host = hostFixture();
  const frame = snapshotFixture();
  frame.contentTransfers = [
    {
      key: 'task',
      id: { session: frame.session, sequence: '8' },
      peer: 'peer',
      deviceName: 'remote',
      incoming: true,
      kind: 'Files',
      names: ['file.txt'],
      totalBytes: 100,
      completedBytes: 20,
      stage: 'Receiving',
      savedPaths: [],
      at: 0,
    },
  ];
  frame.activities = [
    {
      sequence: '8',
      kind: 'Received',
      summary: 'file.txt',
      contentTask: 'task',
      contentNode: 'started',
      contentStage: 'Receiving',
      at: 0,
    },
  ];
  store.apply(frame, host);
  const activity = store.getSnapshot().activities[0];
  frame.revision = '2';
  frame.contentTransfers[0].completedBytes = 100;
  frame.contentTransfers[0].stage = 'Verifying';
  store.apply(frame, host);
  expect(store.getSnapshot().activities[0]).toBe(activity);
  expect(activity.state).toBe('pending');
  frame.revision = '3';
  frame.activities.unshift({
    sequence: '9',
    kind: 'Received',
    summary: 'file.txt',
    contentTask: 'task',
    contentNode: 'finished',
    contentStage: 'Completed',
    at: 1,
  });
  store.apply(frame, host);
  expect(store.getSnapshot().activities[0].state).toBe('applied');
  expect(store.getSnapshot().activities[1]).toEqual(activity);
});
it('treats pairing and live interface data as authoritative without inventing trusted devices', () => {
  const store = new SnapshotStore(appearance);
  const host = hostFixture();
  const frame = snapshotFixture();
  frame.peers = [];
  frame.onboarding.session = {
    id: 'pairing',
    incoming: true,
    stage: 'ShowingCode',
    code: '12345678',
    deviceName: 'pending',
    noobId: 'peer',
    expiresAt: 120000,
    attemptsLeft: 3,
  };
  frame.localDevice.syncPort = 30000;
  frame.localDevice.pairingPort = 30001;
  store.apply(frame, host);
  expect(store.getSnapshot().peers).toEqual([]);
  expect(store.getSnapshot().onboarding.session?.code).toBe('12345678');
  expect(store.getSnapshot().localDevice).toMatchObject({ syncPort: 30000, pairingPort: 30001 });
  frame.revision = '2';
  frame.onboarding.session = null;
  frame.localDevice.addresses = [];
  store.apply(frame, host);
  expect(store.getSnapshot().onboarding.session).toBeNull();
  expect(store.getSnapshot().localDevice.addresses).toEqual([]);
});
