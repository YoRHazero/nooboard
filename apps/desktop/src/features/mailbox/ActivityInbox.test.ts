import { expect, it } from 'vitest';
import type { Activity } from '../../api/contracts';
import { ActivityInbox } from './ActivityInbox';

const copy = (id: number): Activity => ({
  id,
  kind: 'copied',
  state: 'applied',
  sourceNoobId: null,
  title: `文字 ${id}`,
  at: id,
});

it('shows just the latest standalone activity, grouping the original performance with its additions', () => {
  const inbox = new ActivityInbox(null);
  inbox.record(copy(1));
  inbox.record(copy(2));
  expect(inbox.getSnapshot()).toEqual({ latest: copy(2), batch: [] });
  inbox.performance(true, copy(2));
  inbox.record(copy(3));
  inbox.record(copy(4));
  expect(inbox.getSnapshot().batch.map((row) => row.id)).toEqual([4, 3, 2]);
});

it('also groups correctly when playback starts before the activity listener runs', () => {
  const inbox = new ActivityInbox(null);
  inbox.performance(true, copy(1));
  inbox.record(copy(1));
  expect(inbox.getSnapshot().batch).toEqual([]);
  inbox.record(copy(2));
  expect(inbox.getSnapshot().batch.map((row) => row.id)).toEqual([2, 1]);
});

it('updates outcomes in place without counting receipts or duplicate delivery', () => {
  const inbox = new ActivityInbox(null);
  const sent: Activity = { ...copy(1), kind: 'sent', state: 'pending', sourceNoobId: 'peer' };
  inbox.record(sent);
  inbox.performance(true, sent);
  inbox.record(copy(2));
  inbox.sync([{ ...sent, state: 'applied' }]);
  inbox.record(copy(2));
  expect(inbox.getSnapshot().batch).toHaveLength(2);
  expect(inbox.getSnapshot().batch[1].state).toBe('applied');
});

it('keeps a batch after animation completion and includes new activity until acknowledged', () => {
  const inbox = new ActivityInbox(null);
  inbox.record(copy(1));
  inbox.performance(true, copy(1));
  inbox.record(copy(2));
  inbox.performance(false, null);
  inbox.record(copy(3));
  expect(inbox.getSnapshot().batch).toHaveLength(3);
  inbox.acknowledge();
  expect(inbox.getSnapshot()).toEqual({ latest: copy(3), batch: [] });
});

it('does not reintroduce acknowledged activity while the original animation is still playing', () => {
  const inbox = new ActivityInbox(null);
  inbox.record(copy(1));
  inbox.performance(true, copy(1));
  inbox.record(copy(2));
  inbox.acknowledge();
  inbox.record(copy(3));
  expect(inbox.getSnapshot().batch).toEqual([]);
  inbox.record(copy(4));
  expect(inbox.getSnapshot().batch.map((row) => row.id)).toEqual([4, 3]);
});

it('retains all batch summaries when the source recent list rolls over', () => {
  const inbox = new ActivityInbox(null);
  inbox.performance(true, copy(1));
  for (let id = 1; id <= 105; id++) inbox.record(copy(id));
  inbox.sync(Array.from({ length: 30 }, (_, index) => copy(105 - index)));
  expect(inbox.getSnapshot().batch).toHaveLength(105);
  expect(inbox.getSnapshot().batch.at(-1)?.id).toBe(1);
});

it('reset discards the prior session batch and allows sequence numbers to restart', () => {
  const inbox = new ActivityInbox(null);
  inbox.performance(true, copy(1));
  inbox.record(copy(2));
  inbox.reset(copy(0));
  inbox.record(copy(1));
  expect(inbox.getSnapshot()).toEqual({ latest: copy(1), batch: [] });
});
