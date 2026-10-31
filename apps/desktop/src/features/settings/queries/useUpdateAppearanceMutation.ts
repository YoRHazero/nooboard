import { useMutation } from '@tanstack/react-query';
import { getPreferences } from '../preferences';
import type { Appearance } from '../../../shared/types';
export async function updateAppearance(args: [patch: Partial<Appearance>]) {
  return getPreferences().setAppearance(args[0]);
}
export function useUpdateAppearanceMutation() {
  return useMutation({
    mutationKey: ['settings', 'updateAppearance'],
    mutationFn: updateAppearance,
  });
}
