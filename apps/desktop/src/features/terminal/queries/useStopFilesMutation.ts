import { useMutation } from '@tanstack/react-query';
import { terminalRequest } from '../../../shared/backend';
import type { TerminalAction } from '../../../shared/types';
export function useStopFilesMutation() {
  return useMutation({
    mutationKey: ['terminal', 'stopFiles'],
    mutationFn: async (data: Extract<TerminalAction, { type: 'stopFiles' }>['data']) => {
      return terminalRequest({ type: 'stopFiles', data });
    },
  });
}
