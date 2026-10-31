import { useMutation } from '@tanstack/react-query';
import { backendRequest } from '../../../shared/backend';
import type { SyncSettingsPatch } from '../../../shared/types';
export async function updateSyncSettings(args: [patch: SyncSettingsPatch]) {
  return (await backendRequest({ type: 'updateSyncSettings', data: { patch: args[0] } }, 'saved'))
    .data.revision;
}
export function useUpdateSyncSettingsMutation() {
  return useMutation({
    mutationKey: ['settings', 'updateSyncSettings'],
    mutationFn: updateSyncSettings,
  });
}
