import { useMutation } from '@tanstack/react-query';
import { backendRequest } from '../../../shared/backend';
import { useQueryClient } from '@tanstack/react-query';
export async function deleteHistory(args: [id: string]) {
  await backendRequest({ type: 'deleteHistory', data: { id: args[0] } }, 'done');
}
export function useDeleteHistoryMutation() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationKey: ['history', 'deleteHistory'],
    mutationFn: deleteHistory,
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: ['history'] });
    },
  });
}
