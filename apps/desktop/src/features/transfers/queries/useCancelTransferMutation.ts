import { useMutation } from '@tanstack/react-query';
import { backendRequest } from '../../../shared/backend';
export async function cancelTransfer(args: [key: string]) {
  await backendRequest({ type: 'cancelTransfer', data: { key: args[0] } }, 'done');
}
export function useCancelTransferMutation() {
  return useMutation({ mutationKey: ['transfers', 'cancelTransfer'], mutationFn: cancelTransfer });
}
