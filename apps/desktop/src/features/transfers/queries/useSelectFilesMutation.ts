import { useMutation } from '@tanstack/react-query';
import { backendRequest } from '../../../shared/backend';
export async function selectFiles(_args: []) {
  await backendRequest({ type: 'selectFiles' }, 'done');
}
export function useSelectFilesMutation() {
  return useMutation({ mutationKey: ['transfers', 'selectFiles'], mutationFn: selectFiles });
}
