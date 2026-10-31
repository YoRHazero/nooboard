import { useMutation } from '@tanstack/react-query';
import { terminalRequest } from '../../../shared/backend';
import type { TerminalAction } from '../../../shared/types';
export function useRenameMutation() {
  return useMutation({
    mutationKey: ['terminal', 'rename'],
    mutationFn: async (data: Extract<TerminalAction, { type: 'rename' }>['data']) => {
      return terminalRequest({ type: 'rename', data });
    },
  });
}
