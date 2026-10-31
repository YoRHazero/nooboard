import { useMutation } from '@tanstack/react-query';
import { backendRequest } from '../../../shared/backend';
export async function sendCurrent(_args: []) {
  return (await backendRequest({ type: 'sendCurrent' }, 'sent')).data;
}
export function useSendCurrentMutation() {
  return useMutation({ mutationKey: ['transfers', 'sendCurrent'], mutationFn: sendCurrent });
}
