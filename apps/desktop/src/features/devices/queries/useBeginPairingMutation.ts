import { useMutation } from '@tanstack/react-query';
import { backendRequest } from '../../../shared/backend';
export async function beginPairing(args: [address: string, expected?: string]) {
  const [address, expected] = args;
  await backendRequest({ type: 'beginPairing', data: { address, expected } }, 'done');
}
export function useBeginPairingMutation() {
  return useMutation({ mutationKey: ['devices', 'beginPairing'], mutationFn: beginPairing });
}
