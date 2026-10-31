import { useMutation } from '@tanstack/react-query';
import { backendRequest } from '../../../shared/backend';
import { useQueryClient } from '@tanstack/react-query';
export async function clearHistory(_args: []) {
  await backendRequest({ type: 'clearHistory' }, 'done');
}
export function useClearHistoryMutation() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationKey: ['history', 'clearHistory'],
    mutationFn: clearHistory,
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: ['history'] });
    },
  });
}
