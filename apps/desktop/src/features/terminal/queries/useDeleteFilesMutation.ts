import { useMutation } from '@tanstack/react-query';
import { terminalRequest } from '../../../shared/backend';
import type { TerminalAction } from '../../../shared/types';
export function useDeleteFilesMutation() {
  return useMutation({
    mutationKey: ['terminal', 'deleteFiles'],
    mutationFn: async (data: Extract<TerminalAction, { type: 'deleteFiles' }>['data']) => {
      return terminalRequest({ type: 'deleteFiles', data });
    },
  });
}
