import { useMutation } from '@tanstack/react-query';
import { backendRequest } from '../../../shared/backend';
export async function discover(_args: []) {
  await backendRequest({ type: 'discover' }, 'done');
}
export function useDiscoverMutation() {
  return useMutation({ mutationKey: ['devices', 'discover'], mutationFn: discover });
}
