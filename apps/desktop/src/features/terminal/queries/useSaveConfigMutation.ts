import { configUsage } from '../configuration';
import { terminalStore } from '../store';
import { fail } from '../../../shared/i18n/errors';
import { useMutation } from '@tanstack/react-query';
import { getBackendSnapshot, terminalRequest } from '../../../shared/backend';
import type { TerminalAction } from '../../../shared/types';
export function useSaveConfigMutation() {
  return useMutation({
    mutationKey: ['terminal', 'saveConfig'],
    mutationFn: async (data: Extract<TerminalAction, { type: 'saveConfig' }>['data']) => {
      const id = data.id;
      if (
        id &&
        configUsage(id, getBackendSnapshot().terminal, terminalStore.getState().pending).locked
      )
        throw fail('terminalConfigInUse');
      return terminalRequest({ type: 'saveConfig', data });
    },
  });
}
