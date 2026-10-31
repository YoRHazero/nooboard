import { createStore } from 'zustand/vanilla';
import { useStore } from 'zustand';
import { createSessionSlice, type SessionSlice } from './session.slice';
import { createFilesSlice, type FilesSlice } from './files.slice';
import { createTreeSlice, type TreeSlice } from './tree.slice';
export type TerminalUiState = SessionSlice & FilesSlice & TreeSlice;
export const createTerminalStore = () =>
  createStore<TerminalUiState>()((...args) => ({
    ...createSessionSlice(...args),
    ...createFilesSlice(...args),
    ...createTreeSlice(...args),
  }));
export type TerminalStore = ReturnType<typeof createTerminalStore>;
export const terminalStore = createTerminalStore();
export function useTerminalStore<T>(selector: (state: TerminalUiState) => T) {
  return useStore(terminalStore, selector);
}
