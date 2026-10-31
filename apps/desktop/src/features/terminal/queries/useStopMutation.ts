import { useMutation } from '@tanstack/react-query';
import { terminalRequest } from '../../../shared/backend';
import type { TerminalAction } from '../../../shared/types';
export function useStopMutation() {
  return useMutation({
    mutationKey: ['terminal', 'stop'],
    mutationFn: async (data: Extract<TerminalAction, { type: 'stop' }>['data']) => {
      return terminalRequest({ type: 'stop', data });
    },
  });
}
