import { configUsage } from '../configuration';
import { terminalStore } from '../store';
import { fail } from '../../../shared/i18n/errors';
import { useMutation } from '@tanstack/react-query';
import { getBackendSnapshot, terminalRequest } from '../../../shared/backend';
import type { TerminalAction } from '../../../shared/types';
export function useDeleteConfigMutation() {
  return useMutation({
    mutationKey: ['terminal', 'deleteConfig'],
    mutationFn: async (data: Extract<TerminalAction, { type: 'deleteConfig' }>['data']) => {
      const id = data.target.id;
      if (
        id &&
        configUsage(id, getBackendSnapshot().terminal, terminalStore.getState().pending).locked
      )
        throw fail('terminalConfigInUse');
      return terminalRequest({ type: 'deleteConfig', data });
    },
  });
}
