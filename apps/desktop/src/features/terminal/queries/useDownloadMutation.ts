import { useMutation } from '@tanstack/react-query';
import { terminalRequest } from '../../../shared/backend';
import type { TerminalAction } from '../../../shared/types';
export function useDownloadMutation() {
  return useMutation({
    mutationKey: ['terminal', 'download'],
    mutationFn: async (data: Extract<TerminalAction, { type: 'download' }>['data']) => {
      return terminalRequest({ type: 'download', data });
    },
  });
}
