import { useMutation } from '@tanstack/react-query';
import { backendRequest } from '../../../shared/backend';
import type { LocalDevicePatch } from '../../../shared/types';
export async function updateLocalDevice(args: [patch: LocalDevicePatch]) {
  return (await backendRequest({ type: 'updateLocalDevice', data: { patch: args[0] } }, 'saved'))
    .data.revision;
}
export function useUpdateLocalDeviceMutation() {
  return useMutation({
    mutationKey: ['devices', 'updateLocalDevice'],
    mutationFn: updateLocalDevice,
  });
}
