import { expect, it, vi } from 'vitest';
import { SnapshotStore } from '../../desktop/snapshot/store';
import { snapshotFixture, hostFixture } from '../../preview/scenarios/fixtures';
import type { ContentKind, ContentStage } from '../../desktop/bridge/generated';
import { ActivityInbox } from '../mailbox/ActivityInbox';
import { Playback } from './playback';

function setup() {
  const frame = snapshotFixture();
  frame.activities = [];
  frame.contentTransfers = [];
  const host = hostFixture();
  const store = new SnapshotStore({ theme: 'light', reducedMotion: false });
  store.apply(frame, host);
  const play = vi.fn();
  const inbox = new ActivityInbox(null);
  const playback = new Playback(play, (state) =>
    inbox.performance(
      state.activityId !== null,
      store.getSnapshot().activities.find((row) => row.id === state.activityId) ?? null,
    ),
  );
  store.subscribe(() => inbox.sync(store.getSnapshot().activities));
  store.onEvent((event) => {
    if (event.type === 'reset') inbox.reset(store.getSnapshot().activities[0] ?? null);
    else if (
      ['copied', 'sent', 'received'].includes(event.type) ||
      event.contentNode === 'finished'
    )
      inbox.record(store.getSnapshot().activities.find((row) => row.id === event.sequence)!);
    playback.event(event);
  });
  const publish = (recovering = false) => {
    frame.revision = String(BigInt(frame.revision) + 1n);
    store.apply(frame, host, recovering);
  };
  const finish = () => playback.finish(play.mock.lastCall![1]);
  return { frame, store, play, inbox, publish, finish };
}

it.each(['Files', 'Image'] as ContentKind[])(
  'plays %s receipt only after completion, with truthful mailbox state',
  (kind) => {
    const { frame, play, inbox, publish } = setup();
    frame.contentTransfers.push({
      key: 'task',
      id: { session: frame.session, sequence: '1' },
      peer: 'peer',
      deviceName: 'Remote',
      incoming: true,
      kind,
      names: ['sample'],
      totalBytes: 100,
      completedBytes: 0,
      stage: 'Receiving',
      savedPaths: [],
      at: 0,
    });
    frame.activities.unshift({
      sequence: '1',
      kind: 'Received',
      summary: 'sample',
      at: 0,
      contentTask: 'task',
      contentNode: 'started',
      contentStage: 'Receiving',
    });
    publish();
    expect(play).not.toHaveBeenCalled();
    expect(inbox.getSnapshot().latest?.state).toBe('pending');
    frame.contentTransfers[0].completedBytes = 100;
    frame.contentTransfers[0].stage = 'Verifying';
    publish();
    expect(play).not.toHaveBeenCalled();
    frame.contentTransfers[0].stage = 'Completed';
    frame.activities.unshift({
      ...frame.activities[0],
      sequence: '2',
      contentNode: 'finished',
      contentStage: 'Completed',
    });
    publish();
    expect(play).toHaveBeenLastCalledWith('receive_file', expect.any(Number));
    expect(inbox.getSnapshot().latest).toMatchObject({
      contentKind: kind,
      state: 'applied',
      contentNode: 'finished',
    });
  },
);

it.each(['Saved', 'Failed', 'Cancelled'] as ContentStage[])(
  'preserves %s as a distinct receipt outcome',
  (stage) => {
    const { frame, play, inbox, publish } = setup();
    frame.activities.unshift({
      sequence: '1',
      kind: 'Received',
      summary: 'sample',
      at: 0,
      contentTask: 'task',
      contentNode: 'started',
      contentStage: 'Receiving',
    });
    publish();
    frame.activities.unshift({
      ...frame.activities[0],
      sequence: '2',
      contentNode: 'finished',
      contentStage: stage,
    });
    publish();
    expect(inbox.getSnapshot().latest?.contentStage).toBe(stage);
    if (stage === 'Saved') {
      expect(play).toHaveBeenLastCalledWith('receive_file', expect.any(Number));
      expect(inbox.getSnapshot().latest?.state).toBe('partial');
    } else if (stage === 'Failed')
      expect(play).toHaveBeenLastCalledWith('error', expect.any(Number));
    else expect(play).not.toHaveBeenCalled();
  },
);

it.each(['Applied', 'Unconfirmed', 'Rejected'] as const)(
  'does not lose a %s text receipt delivered in the first snapshot',
  (state) => {
    const { frame, play, publish, finish, inbox } = setup();
    const id = { session: frame.session, sequence: '1' };
    frame.transfers = [
      {
        id,
        automatic: false,
        bytes: 4,
        targets: [{ noobId: 'peer', deviceName: 'Remote', state }],
      },
    ];
    frame.activities = [{ sequence: '1', messageId: id, kind: 'Sent', summary: 'text', at: 0 }];
    publish();
    expect(play).toHaveBeenLastCalledWith('send', expect.any(Number));
    finish();
    expect(play).toHaveBeenLastCalledWith(
      state === 'Applied' ? 'success' : 'error',
      expect.any(Number),
    );
    expect(inbox.getSnapshot().latest?.state).toBe(state.toLowerCase());
  },
);

it('updates clipboard state immediately and groups extra activity without replay after recovery', () => {
  const { frame, store, play, inbox, publish, finish } = setup();
  frame.activities = [{ sequence: '1', kind: 'Received', summary: 'one', at: 0 }];
  frame.current.text = 'one';
  publish();
  expect(play).toHaveBeenLastCalledWith('receive_text', expect.any(Number));
  frame.activities.unshift({ sequence: '2', kind: 'Copied', summary: 'two', at: 1 });
  frame.current.text = 'two';
  publish();
  expect(store.getSnapshot().current.text).toBe('two');
  expect(inbox.getSnapshot().batch.map((row) => row.title)).toEqual(['two', 'one']);
  expect(play).toHaveBeenCalledTimes(1);
  finish();
  frame.activities.unshift({ sequence: '3', kind: 'Received', summary: 'recovered', at: 2 });
  publish(true);
  expect(play).toHaveBeenCalledTimes(2);
  expect(play).toHaveBeenLastCalledWith('idle', null);
});
