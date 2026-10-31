import { useMutation } from '@tanstack/react-query';
import { backendRequest } from '../../../shared/backend';
export async function submitPairingCode(args: [id: string, code: string]) {
  const [id, code] = args;
  await backendRequest({ type: 'pairingCode', data: { id, code } }, 'done');
}
export function useSubmitPairingCodeMutation() {
  return useMutation({
    mutationKey: ['devices', 'submitPairingCode'],
    mutationFn: submitPairingCode,
  });
}
