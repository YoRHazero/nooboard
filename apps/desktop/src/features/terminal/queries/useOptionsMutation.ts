import { useMutation } from '@tanstack/react-query';
import { terminalRequest } from '../../../shared/backend';
import type { TerminalAction } from '../../../shared/types';
export function useOptionsMutation() {
  return useMutation({
    mutationKey: ['terminal', 'options'],
    mutationFn: async (data: Extract<TerminalAction, { type: 'options' }>['data']) => {
      return terminalRequest({ type: 'options', data });
    },
  });
}
