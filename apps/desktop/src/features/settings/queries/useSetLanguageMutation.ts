import { useMutation } from '@tanstack/react-query';
import { getPreferences } from '../preferences';
import type { LanguagePreference } from '../../../shared/types';
export async function setLanguage(args: [language: LanguagePreference]) {
  return getPreferences().setLanguage(args[0]);
}
export function useSetLanguageMutation() {
  return useMutation({ mutationKey: ['settings', 'setLanguage'], mutationFn: setLanguage });
}
