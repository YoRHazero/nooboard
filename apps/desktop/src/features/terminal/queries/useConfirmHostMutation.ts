import { useMutation } from '@tanstack/react-query';
import { terminalRequest } from '../../../shared/backend';
import type { TerminalAction } from '../../../shared/types';
export function useConfirmHostMutation() {
  return useMutation({
    mutationKey: ['terminal', 'confirmHost'],
    mutationFn: async (data: Extract<TerminalAction, { type: 'confirmHost' }>['data']) => {
      return terminalRequest({ type: 'confirmHost', data });
    },
  });
}
