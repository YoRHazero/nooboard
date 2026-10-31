import { useMutation } from '@tanstack/react-query';
import { backendRequest } from '../../../shared/backend';
export async function dismissPairing(args: [id: string]) {
  await backendRequest({ type: 'dismissPairing', data: { id: args[0] } }, 'done');
}
export function useDismissPairingMutation() {
  return useMutation({ mutationKey: ['devices', 'dismissPairing'], mutationFn: dismissPairing });
}
