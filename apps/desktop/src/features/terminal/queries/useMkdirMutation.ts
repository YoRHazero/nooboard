import { useMutation } from '@tanstack/react-query';
import { terminalRequest } from '../../../shared/backend';
import type { TerminalAction } from '../../../shared/types';
export function useMkdirMutation() {
  return useMutation({
    mutationKey: ['terminal', 'mkdir'],
    mutationFn: async (data: Extract<TerminalAction, { type: 'mkdir' }>['data']) => {
      return terminalRequest({ type: 'mkdir', data });
    },
  });
}
