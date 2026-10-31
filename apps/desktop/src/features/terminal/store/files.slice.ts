import type { StateCreator } from 'zustand';
import type { TerminalConflict } from '../../../shared/types';
export interface FileView {
  path?: string;
  expanded: string[];
  selected: string[];
  selecting: boolean;
}
export const emptyFileView: FileView = { expanded: [], selected: [], selecting: false };
export type FolderClickMode = 'navigate' | 'expand';
const settingsKey = 'nooboard.terminal.files';
function savedSettings(): { folderClick: FolderClickMode; conflict: TerminalConflict } {
  try {
    const saved = JSON.parse(localStorage.getItem(settingsKey) ?? '{}');
    return {
      folderClick: saved.folderClick === 'navigate' ? 'navigate' : 'expand',
      conflict: ['fail', 'skip', 'overwrite'].includes(saved.conflict) ? saved.conflict : 'fail',
    };
  } catch {
    return { folderClick: 'expand', conflict: 'fail' };
  }
}
export interface FilesSlice {
  filesOpen: boolean;
  folderClick: FolderClickMode;
  conflict: TerminalConflict;
  fileViews: Record<string, FileView>;
  toggleFiles(): void;
  closeFiles(): void;
  setFileSettings(settings: Partial<Pick<FilesSlice, 'folderClick' | 'conflict'>>): void;
  updateFileView(id: string, update: Partial<FileView>): void;
  forgetFileView(id?: string): void;
}
export const createFilesSlice: StateCreator<FilesSlice, [], [], FilesSlice> = (set, get) => ({
  filesOpen: false,
  ...savedSettings(),
  fileViews: {},
  toggleFiles: () => set((state) => ({ filesOpen: !state.filesOpen })),
  closeFiles: () => set({ filesOpen: false }),
  setFileSettings: (settings) => {
    set(settings);
    const { folderClick, conflict } = get();
    try {
      localStorage.setItem(settingsKey, JSON.stringify({ folderClick, conflict }));
    } catch {
      /* Private browsing can disable storage. */
    }
  },
  updateFileView: (id, update) =>
    set((state) => ({
      fileViews: {
        ...state.fileViews,
        [id]: { ...(state.fileViews[id] ?? emptyFileView), ...update },
      },
    })),
  forgetFileView: (id) =>
    set((state) => {
      const fileViews = { ...state.fileViews };
      if (id) delete fileViews[id];
      return { fileViews: id ? fileViews : {} };
    }),
});
