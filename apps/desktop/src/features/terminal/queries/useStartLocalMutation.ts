import { useMutation } from '@tanstack/react-query';
import { getTerminalStreams } from '../io/output-stream';
export function useStartLocalMutation() {
  return useMutation({
    mutationKey: ['terminal', 'startLocal'],
    mutationFn: async (args: [name: string]) => {
      return getTerminalStreams().startLocal(...args);
    },
  });
}
