import { useMutation } from '@tanstack/react-query';
import { backendRequest } from '../backend';
export async function acknowledgeNavigation(args: [id: number]) {
  await backendRequest({ type: 'acknowledgeNavigation', data: { id: args[0] } }, 'done');
}
export function useAcknowledgeNavigationMutation() {
  return useMutation({
    mutationKey: ['shared', 'acknowledgeNavigation'],
    mutationFn: acknowledgeNavigation,
  });
}
