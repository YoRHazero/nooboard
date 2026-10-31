import { useMutation } from '@tanstack/react-query';
import { backendRequest } from '../../../shared/backend';
export async function selectTargets(args: [targets: string[]]) {
  await backendRequest({ type: 'selectTargets', data: { targets: args[0] } }, 'done');
}
export function useSelectTargetsMutation() {
  return useMutation({ mutationKey: ['transfers', 'selectTargets'], mutationFn: selectTargets });
}
