import type { StateCreator } from 'zustand';
export interface TreeSlice {
  configurationsOpen: boolean;
  foldedNodes: readonly string[];
  selectedConfig: string | null;
  clearConfigSelection(): void;
  toggleConfigurations(): void;
  toggleNode(id: string): void;
  revealConfig(id: string, parent: string): void;
}
export const createTreeSlice: StateCreator<TreeSlice, [], [], TreeSlice> = (set) => ({
  configurationsOpen: true,
  foldedNodes: [],
  selectedConfig: null,
  clearConfigSelection: () => set({ selectedConfig: null }),
  toggleConfigurations: () => set((s) => ({ configurationsOpen: !s.configurationsOpen })),
  toggleNode: (id) =>
    set((s) => ({
      foldedNodes: s.foldedNodes.includes(id)
        ? s.foldedNodes.filter((key) => key !== id)
        : [...s.foldedNodes, id],
    })),
  revealConfig: (id, parent) =>
    set((s) => ({
      configurationsOpen: true,
      selectedConfig: id,
      foldedNodes: s.foldedNodes.filter((key) => key !== parent && key !== id),
    })),
});
