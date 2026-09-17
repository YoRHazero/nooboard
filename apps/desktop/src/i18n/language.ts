import { useSyncExternalStore } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { i18n } from './index';

export type LanguagePreference = 'system' | 'zh-CN' | 'en';
const key = 'nooboard.language.v1';
export const isLanguagePreference = (value: unknown): value is LanguagePreference =>
  value === 'system' || value === 'zh-CN' || value === 'en';
export function resolveLanguage(preference: LanguagePreference, detected?: string | null) {
  if (preference !== 'system') return preference;
  return /^zh(?:[-_]|$)/i.test(detected ?? '') ? 'zh-CN' : 'en';
}
export function loadLanguage(): LanguagePreference {
  try {
    const value = localStorage.getItem(key);
    return isLanguagePreference(value) ? value : 'system';
  } catch {
    return 'system';
  }
}
let preference = loadLanguage();
let generation = 0;
type NativePreferences = (
  args: Record<string, unknown>,
) => Promise<{ language: LanguagePreference }>;
let nativePreferences: NativePreferences | null = null;
let nativeWrites = Promise.resolve<unknown>(undefined);
let detect = async (): Promise<string | null> => navigator.languages?.[0] ?? navigator.language;
const listeners = new Set<() => void>();
const subscribe = (listener: () => void) => {
  listeners.add(listener);
  return () => {
    listeners.delete(listener);
  };
};
async function apply(next: LanguagePreference, save: boolean, syncNative = true) {
  const request = ++generation;
  const detected = next === 'system' ? await detect().catch(() => null) : null;
  if (request !== generation) return;
  if (nativePreferences && syncNative) {
    const bridge = nativePreferences;
    nativeWrites = nativeWrites
      .catch(() => undefined)
      .then(() =>
        request === generation ? bridge({ patch: save ? { language: next } : null }) : undefined,
      );
    await nativeWrites;
    if (request !== generation) return;
  }
  await i18n.changeLanguage(resolveLanguage(next, detected));
  if (request !== generation) return;
  preference = next;
  if (save) {
    try {
      localStorage.setItem(key, next);
    } catch {
      /* Retain the choice for this session. */
    }
  }
  document.documentElement.lang = i18n.resolvedLanguage ?? 'en';
  listeners.forEach((listener) => listener());
}
export async function initializeLanguage(native: boolean) {
  let initial = loadLanguage();
  if (native) {
    const { locale } = await import('@tauri-apps/plugin-os');
    detect = async () => await locale().catch(() => navigator.language);
    nativePreferences = (args) => invoke('desktop_preferences', args);
    try {
      const saved = await nativePreferences({ legacyLanguage: initial });
      if (isLanguagePreference(saved.language)) initial = saved.language;
    } catch (error) {
      // Keep the error/retry UI accessible if native preferences cannot be loaded.
      console.error('Desktop language initialization failed', error);
    }
  }
  await apply(initial, false, false);
  const refresh = () => {
    if (preference === 'system')
      void apply('system', false).catch((error: unknown) =>
        console.error('Language refresh failed', error),
      );
  };
  window.addEventListener('languagechange', refresh);
  window.addEventListener('focus', refresh);
  return () => {
    generation++;
    nativePreferences = null;
    nativeWrites = Promise.resolve();
    window.removeEventListener('languagechange', refresh);
    window.removeEventListener('focus', refresh);
  };
}
export const changeLanguage = (next: LanguagePreference) => apply(next, true);
export const useLanguagePreference = () => useSyncExternalStore(subscribe, () => preference);
