import { useMutation } from '@tanstack/react-query';
import { backendRequest } from '../../../shared/backend';
export async function copyHistory(args: [id: string]) {
  await backendRequest({ type: 'copyHistory', data: { id: args[0] } }, 'done');
}
export function useCopyHistoryMutation() {
  return useMutation({ mutationKey: ['history', 'copyHistory'], mutationFn: copyHistory });
}
