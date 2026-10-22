export type SessionSelection = { id: string; awaitingSnapshot: boolean } | undefined;

type Event =
  | { type: 'select'; id?: string }
  | { type: 'connected'; id: string }
  | { type: 'snapshot'; ids: readonly string[] };

/** Command replies and the throttled workspace snapshots can arrive in either order. */
export function sessionSelection(state: SessionSelection, event: Event): SessionSelection {
  switch (event.type) {
    case 'select':
      return event.id ? { id: event.id, awaitingSnapshot: false } : undefined;
    case 'connected':
      return { id: event.id, awaitingSnapshot: true };
    case 'snapshot':
      if (!state) return state;
      if (event.ids.includes(state.id))
        return state.awaitingSnapshot ? { ...state, awaitingSnapshot: false } : state;
      // An unseen workspace is still arriving; only a previously observed one
      // can be treated as closed when it disappears from a later snapshot.
      return state.awaitingSnapshot ? state : undefined;
  }
}
