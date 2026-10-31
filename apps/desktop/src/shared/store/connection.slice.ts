import type { StateCreator } from 'zustand';
import type { ConnectionState } from '../types';
export interface ConnectionSlice {
  connection: ConnectionState;
  setConnection(connection: ConnectionState): void;
}
export const createConnectionSlice: StateCreator<ConnectionSlice, [], [], ConnectionSlice> = (
  set,
) => ({
  connection: { phase: 'idle', error: null, hasSnapshot: false },
  setConnection: (connection) => set({ connection }),
});
