import type { StateCreator } from 'zustand';
import type { Activity } from '../../../shared/types';
export interface InboxSlice {
  latest: Activity | null;
  batch: readonly Activity[];
  performing: boolean;
  anchor: Activity | null;
  performance(active: boolean, activity: Activity | null): void;
  record(activity: Activity): void;
  sync(activities: readonly Activity[]): void;
  acknowledge(): void;
  reset(latest: Activity | null): void;
}
/** Retained presentation summaries; never used as the source of current transfer state. */
export const createInboxSlice: StateCreator<InboxSlice, [], [], InboxSlice> = (set, get) => ({
  latest: null,
  batch: [],
  performing: false,
  anchor: null,
  performance: (active, activity) => set({ performing: active, anchor: active ? activity : null }),
  record: (activity) => {
    const state = get();
    if (state.latest?.id === activity.id || state.batch.some((row) => row.id === activity.id)) {
      state.sync([activity]);
      return;
    }
    let batch = state.batch;
    if (batch.length) batch = [activity, ...batch];
    else if (state.performing && state.anchor && state.anchor.id !== activity.id)
      batch = [activity, state.anchor];
    set({
      latest: activity,
      batch,
      anchor: state.performing && !state.anchor ? activity : state.anchor,
    });
  },
  sync: (activities) => {
    const state = get();
    const byId = new Map(activities.map((row) => [row.id, row]));
    const update = (row: Activity) => byId.get(row.id) ?? row;
    const latest = state.latest ? update(state.latest) : null;
    const batch = state.batch.map(update);
    const anchor = state.anchor ? update(state.anchor) : null;
    if (
      latest !== state.latest ||
      anchor !== state.anchor ||
      batch.some((row, i) => row !== state.batch[i])
    )
      set({ latest, batch, anchor });
  },
  acknowledge: () => set({ anchor: null, batch: [] }),
  reset: (latest) => set({ latest, batch: [], performing: false, anchor: null }),
});
