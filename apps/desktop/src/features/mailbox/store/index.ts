import { createStore } from 'zustand/vanilla';
import type { Activity } from '../../../shared/types';
import { createInboxSlice } from './inbox.slice';
export function createInboxStore(latest: Activity | null = null) {
  const store = createStore(createInboxSlice);
  store.getState().reset(latest);
  return store;
}
export type InboxStore = ReturnType<typeof createInboxStore>;
