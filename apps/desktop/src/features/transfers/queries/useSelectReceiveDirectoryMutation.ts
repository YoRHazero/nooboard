import { useMutation } from '@tanstack/react-query';
import { backendRequest } from '../../../shared/backend';
export async function selectReceiveDirectory(_args: []) {
  await backendRequest({ type: 'selectReceiveDirectory' }, 'done');
}
export function useSelectReceiveDirectoryMutation() {
  return useMutation({
    mutationKey: ['transfers', 'selectReceiveDirectory'],
    mutationFn: selectReceiveDirectory,
  });
}
