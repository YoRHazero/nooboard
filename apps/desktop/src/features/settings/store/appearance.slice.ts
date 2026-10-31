import type { StateCreator } from 'zustand';
import type { Appearance } from '../../../shared/preferences';
export interface AppearanceSlice {
  appearance: Appearance;
  setAppearance(appearance: Appearance): void;
}
export const createAppearanceSlice: StateCreator<AppearanceSlice, [], [], AppearanceSlice> = (
  set,
) => ({
  appearance: { theme: 'system', reducedMotion: false },
  setAppearance: (appearance) => set({ appearance }),
});
