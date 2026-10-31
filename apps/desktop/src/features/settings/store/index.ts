import { create } from 'zustand';
import { createAppearanceSlice, type AppearanceSlice } from './appearance.slice';
import { createLanguageSlice, type LanguageSlice } from './language.slice';
export const usePreferenceStore = create<AppearanceSlice & LanguageSlice>()((...args) => ({
  ...createAppearanceSlice(...args),
  ...createLanguageSlice(...args),
}));
export const usePreferences = usePreferenceStore;
