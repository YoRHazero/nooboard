import { useMutation } from '@tanstack/react-query';
import { backendRequest } from '../../../shared/backend';
export async function copyReceived(args: [key: string]) {
  await backendRequest({ type: 'copyReceived', data: { key: args[0] } }, 'done');
}
export function useCopyReceivedMutation() {
  return useMutation({ mutationKey: ['transfers', 'copyReceived'], mutationFn: copyReceived });
}
