import { useMutation } from '@tanstack/react-query';
import { terminalRequest } from '../../../shared/backend';
import type { TerminalAction } from '../../../shared/types';
export function useLifetimeMutation() {
  return useMutation({
    mutationKey: ['terminal', 'lifetime'],
    mutationFn: async (data: Extract<TerminalAction, { type: 'lifetime' }>['data']) => {
      return terminalRequest({ type: 'lifetime', data });
    },
  });
}
