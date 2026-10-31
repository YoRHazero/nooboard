import { useMutation } from '@tanstack/react-query';
import { terminalRequest } from '../../../shared/backend';
import type { TerminalAction } from '../../../shared/types';
export function useUploadMutation() {
  return useMutation({
    mutationKey: ['terminal', 'upload'],
    mutationFn: async (data: Extract<TerminalAction, { type: 'upload' }>['data']) => {
      return terminalRequest({ type: 'upload', data });
    },
  });
}
