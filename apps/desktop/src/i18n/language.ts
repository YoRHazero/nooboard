import { useSyncExternalStore } from 'react';
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
let detect = async (): Promise<string | null> => navigator.languages?.[0] ?? navigator.language;
const listeners = new Set<() => void>();
const subscribe = (listener: () => void) => {
  listeners.add(listener);
  return () => {
    listeners.delete(listener);
  };
};
async function apply(next: LanguagePreference, save: boolean) {
  const request = ++generation;
  const detected = next === 'system' ? await detect().catch(() => null) : null;
  if (request !== generation) return;
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
  if (native) {
    const { locale } = await import('@tauri-apps/plugin-os');
    detect = async () => await locale().catch(() => navigator.language);
  }
  await apply(preference, false);
  const refresh = () => {
    if (preference === 'system') void apply('system', false);
  };
  window.addEventListener('languagechange', refresh);
  window.addEventListener('focus', refresh);
  return () => {
    window.removeEventListener('languagechange', refresh);
    window.removeEventListener('focus', refresh);
  };
}
export const changeLanguage = (next: LanguagePreference) => apply(next, true);
export const useLanguagePreference = () => useSyncExternalStore(subscribe, () => preference);
