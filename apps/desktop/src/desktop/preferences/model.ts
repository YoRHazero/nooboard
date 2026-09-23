export type Theme = 'system' | 'light' | 'dark';
import type { LanguagePreference } from '../../i18n/language';
export type { LanguagePreference } from '../../i18n/language';
export interface Appearance {
  theme: Theme;
  reducedMotion: boolean;
}
export interface Preferences {
  appearance: Appearance;
  language: LanguagePreference;
}
