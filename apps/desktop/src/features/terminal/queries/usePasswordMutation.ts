import { useMutation } from '@tanstack/react-query';
import { terminalRequest } from '../../../shared/backend';
import type { TerminalAction } from '../../../shared/types';
export function usePasswordMutation() {
  return useMutation({
    mutationKey: ['terminal', 'password'],
    mutationFn: async (data: Extract<TerminalAction, { type: 'password' }>['data']) => {
      return terminalRequest({ type: 'password', data });
    },
  });
}
