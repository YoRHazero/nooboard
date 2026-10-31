import { useMutation } from '@tanstack/react-query';
import { terminalRequest } from '../../../shared/backend';
import type { TerminalAction } from '../../../shared/types';
export function useCancelPromptMutation() {
  return useMutation({
    mutationKey: ['terminal', 'cancelPrompt'],
    mutationFn: async (data: Extract<TerminalAction, { type: 'cancelPrompt' }>['data']) => {
      return terminalRequest({ type: 'cancelPrompt', data });
    },
  });
}
