import { useMutation } from '@tanstack/react-query';
import { terminalRequest } from '../../../shared/backend';
import type { TerminalAction } from '../../../shared/types';
export function useCopyConfigMutation() {
  return useMutation({
    mutationKey: ['terminal', 'copyConfig'],
    mutationFn: async (data: Extract<TerminalAction, { type: 'copyConfig' }>['data']) => {
      return terminalRequest({ type: 'copyConfig', data });
    },
  });
}
