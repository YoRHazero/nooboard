import { useMutation } from '@tanstack/react-query';
import { getBackend, reconnectBackend } from '../../../shared/backend';
import { fail } from '../../../shared/i18n/errors';

export async function installUpdate() {
  try {
    // This host action deliberately spans Core shutdown and session replacement.
    const reply = await getBackend().request({ type: 'installUpdate' });
    if (reply.type !== 'done') throw fail('updateInstall');
  } catch (error) {
    // A failed installer resumes Core. Attach to its new session before retrying.
    await reconnectBackend().catch(() => {});
    throw error;
  }
}
export function useInstallUpdateMutation() {
  return useMutation({ mutationKey: ['update', 'install'], mutationFn: installUpdate });
}
