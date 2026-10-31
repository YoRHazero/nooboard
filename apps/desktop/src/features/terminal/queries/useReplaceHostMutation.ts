import { useMutation } from '@tanstack/react-query';
import { terminalRequest } from '../../../shared/backend';
import type { TerminalAction } from '../../../shared/types';
export function useReplaceHostMutation() {
  return useMutation({
    mutationKey: ['terminal', 'replaceHost'],
    mutationFn: async (data: Extract<TerminalAction, { type: 'replaceHost' }>['data']) => {
      return terminalRequest({ type: 'replaceHost', data });
    },
  });
}
