export type LanguagePreference = 'system' | 'en' | 'zh-CN';
export const isLanguagePreference = (value: unknown): value is LanguagePreference =>
  value === 'system' || value === 'zh-CN' || value === 'en';
export function resolveLanguage(preference: LanguagePreference, detected?: string | null) {
  if (preference !== 'system') return preference;
  return /^zh(?:[-_]|$)/i.test(detected ?? '') ? 'zh-CN' : 'en';
}
