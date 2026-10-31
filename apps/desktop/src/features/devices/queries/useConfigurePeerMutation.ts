import { useMutation } from '@tanstack/react-query';
import { backendRequest } from '../../../shared/backend';
import type { PeerSettings } from '../../../shared/types';
export async function configurePeer(args: [noobId: string, patch: Partial<PeerSettings>]) {
  const [noobId, patch] = args;
  await backendRequest(
    {
      type: 'configurePeer',
      data: {
        noobId,
        patch: {
          autoSend: patch.autoSend,
          address: Object.hasOwn(patch, 'address')
            ? patch.address
              ? { type: 'set', data: patch.address }
              : { type: 'clear' }
            : undefined,
        },
      },
    },
    'done',
  );
}
export function useConfigurePeerMutation() {
  return useMutation({ mutationKey: ['devices', 'configurePeer'], mutationFn: configurePeer });
}
