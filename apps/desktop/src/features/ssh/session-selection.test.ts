import { expect, it } from 'vitest';
import { sessionSelection } from './session-selection';

it('keeps the new session selected when the command reply precedes its snapshot', () => {
  let selection = sessionSelection(undefined, { type: 'connected', id: 'first' });
  selection = sessionSelection(selection, { type: 'snapshot', ids: [] });
  selection = sessionSelection(selection, { type: 'snapshot', ids: ['older'] });
  expect(selection).toEqual({ id: 'first', awaitingSnapshot: true });

  selection = sessionSelection(selection, { type: 'snapshot', ids: ['older', 'first'] });
  expect(selection).toEqual({ id: 'first', awaitingSnapshot: false });
});

it('recognizes a workspace that was already published before the command replied', () => {
  let selection = sessionSelection(undefined, { type: 'snapshot', ids: ['first'] });
  selection = sessionSelection(selection, { type: 'connected', id: 'first' });
  selection = sessionSelection(selection, { type: 'snapshot', ids: ['first'] });
  expect(selection).toEqual({ id: 'first', awaitingSnapshot: false });
});

it('returns to the login form when a previously observed session is closed', () => {
  let selection = sessionSelection(undefined, { type: 'connected', id: 'first' });
  selection = sessionSelection(selection, { type: 'snapshot', ids: ['first'] });
  selection = sessionSelection(selection, { type: 'snapshot', ids: [] });
  expect(selection).toBeUndefined();
});

it('does not steal focus after the user switches tabs or explicitly starts another session', () => {
  for (const id of ['older', undefined]) {
    let selection = sessionSelection(undefined, { type: 'connected', id: 'first' });
    selection = sessionSelection(selection, { type: 'select', id });
    const chosen = selection;
    selection = sessionSelection(selection, { type: 'snapshot', ids: ['older', 'first'] });
    expect(selection).toBe(chosen);
  }
});
