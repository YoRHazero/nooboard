import { useMutation } from '@tanstack/react-query';
import { backendRequest } from '../../../shared/backend';
export async function acceptPairing(args: [id: string]) {
  await backendRequest({ type: 'acceptPairing', data: { id: args[0] } }, 'done');
}
export function useAcceptPairingMutation() {
  return useMutation({ mutationKey: ['devices', 'acceptPairing'], mutationFn: acceptPairing });
}
