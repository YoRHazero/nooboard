import { useMutation } from '@tanstack/react-query';
import { terminalRequest } from '../../../shared/backend';
import type { TerminalAction } from '../../../shared/types';
export function useContinueMutation() {
  return useMutation({
    mutationKey: ['terminal', 'continue'],
    mutationFn: async (data: Extract<TerminalAction, { type: 'continue' }>['data']) => {
      return terminalRequest({ type: 'continue', data });
    },
  });
}
