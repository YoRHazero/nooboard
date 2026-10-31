import { expect, it } from 'vitest';
import type { Activity } from '../../shared/types';
import { createInboxStore } from './store';

const copy = (id: number): Activity => ({
  id: String(id),
  kind: 'copied',
  state: 'applied',
  sourceNoobId: null,
  title: `文字 ${id}`,
  at: id,
});

it('shows just the latest standalone activity, grouping the original performance with its additions', () => {
  const inbox = createInboxStore(null);
  inbox.getState().record(copy(1));
  inbox.getState().record(copy(2));
  expect(inbox.getState()).toMatchObject({ latest: copy(2), batch: [] });
  inbox.getState().performance(true, copy(2));
  inbox.getState().record(copy(3));
  inbox.getState().record(copy(4));
  expect(inbox.getState().batch.map((row) => row.id)).toEqual(['4', '3', '2']);
});

it('also groups correctly when playback starts before the activity listener runs', () => {
  const inbox = createInboxStore(null);
  inbox.getState().performance(true, copy(1));
  inbox.getState().record(copy(1));
  expect(inbox.getState().batch).toEqual([]);
  inbox.getState().record(copy(2));
  expect(inbox.getState().batch.map((row) => row.id)).toEqual(['2', '1']);
});

it('updates outcomes in place without counting receipts or duplicate delivery', () => {
  const inbox = createInboxStore(null);
  const sent: Activity = { ...copy(1), kind: 'sent', state: 'pending', sourceNoobId: 'peer' };
  inbox.getState().record(sent);
  inbox.getState().performance(true, sent);
  inbox.getState().record(copy(2));
  inbox.getState().sync([{ ...sent, state: 'applied' }]);
  inbox.getState().record(copy(2));
  expect(inbox.getState().batch).toHaveLength(2);
  expect(inbox.getState().batch[1].state).toBe('applied');
});

it('keeps a batch after animation completion and includes new activity until acknowledged', () => {
  const inbox = createInboxStore(null);
  inbox.getState().record(copy(1));
  inbox.getState().performance(true, copy(1));
  inbox.getState().record(copy(2));
  inbox.getState().performance(false, null);
  inbox.getState().record(copy(3));
  expect(inbox.getState().batch).toHaveLength(3);
  inbox.getState().acknowledge();
  expect(inbox.getState()).toMatchObject({ latest: copy(3), batch: [] });
});

it('does not reintroduce acknowledged activity while the original animation is still playing', () => {
  const inbox = createInboxStore(null);
  inbox.getState().record(copy(1));
  inbox.getState().performance(true, copy(1));
  inbox.getState().record(copy(2));
  inbox.getState().acknowledge();
  inbox.getState().record(copy(3));
  expect(inbox.getState().batch).toEqual([]);
  inbox.getState().record(copy(4));
  expect(inbox.getState().batch.map((row) => row.id)).toEqual(['4', '3']);
});

it('retains all batch summaries when the source recent list rolls over', () => {
  const inbox = createInboxStore(null);
  inbox.getState().performance(true, copy(1));
  for (let id = 1; id <= 105; id++) inbox.getState().record(copy(id));
  inbox.getState().sync(Array.from({ length: 30 }, (_, index) => copy(105 - index)));
  expect(inbox.getState().batch).toHaveLength(105);
  expect(inbox.getState().batch.at(-1)?.id).toBe('1');
});

it('reset discards the prior session batch and allows sequence numbers to restart', () => {
  const inbox = createInboxStore(null);
  inbox.getState().performance(true, copy(1));
  inbox.getState().record(copy(2));
  inbox.getState().reset(copy(0));
  inbox.getState().record(copy(1));
  expect(inbox.getState()).toMatchObject({ latest: copy(1), batch: [] });
});
