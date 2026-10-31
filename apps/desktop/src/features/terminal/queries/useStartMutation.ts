import { useMutation } from '@tanstack/react-query';
import { getTerminalStreams } from '../io/output-stream';
import type { TerminalTarget, TerminalReuse } from '../../../shared/types';
export function useStartMutation() {
  return useMutation({
    mutationKey: ['terminal', 'start'],
    mutationFn: async (args: [target: TerminalTarget, reuse?: TerminalReuse]) => {
      return getTerminalStreams().start(...args);
    },
  });
}
