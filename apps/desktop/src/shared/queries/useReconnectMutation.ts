import { useMutation } from '@tanstack/react-query';
import { reconnectBackend } from '../backend';
export async function reconnect(_args: []) {
  return reconnectBackend();
}
export function useReconnectMutation() {
  return useMutation({ mutationKey: ['shared', 'reconnect'], mutationFn: reconnect });
}
