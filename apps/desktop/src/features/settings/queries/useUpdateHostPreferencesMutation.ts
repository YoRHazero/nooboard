import { useMutation } from '@tanstack/react-query';
import { getPreferences } from '../preferences';
import type { HostPreferencesPatch } from '../../../shared/types';
export async function updateHostPreferences(args: [patch: HostPreferencesPatch]) {
  return getPreferences().updateHost(args[0]);
}
export function useUpdateHostPreferencesMutation() {
  return useMutation({
    mutationKey: ['settings', 'updateHostPreferences'],
    mutationFn: updateHostPreferences,
  });
}
