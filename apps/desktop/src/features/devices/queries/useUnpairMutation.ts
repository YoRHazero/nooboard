import { useMutation } from '@tanstack/react-query';
import { backendRequest } from '../../../shared/backend';
export async function unpair(args: [id: string]) {
  await backendRequest({ type: 'unpair', data: { noobId: args[0] } }, 'done');
}
export function useUnpairMutation() {
  return useMutation({ mutationKey: ['devices', 'unpair'], mutationFn: unpair });
}
