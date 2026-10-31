import type { StateCreator } from 'zustand';
import type { LanguagePreference } from '../../../shared/preferences';
export interface LanguageSlice {
  language: LanguagePreference;
  setAppliedLanguage(language: LanguagePreference): void;
}
export const createLanguageSlice: StateCreator<LanguageSlice, [], [], LanguageSlice> = (set) => ({
  language: 'system',
  setAppliedLanguage: (language) => set({ language }),
});
