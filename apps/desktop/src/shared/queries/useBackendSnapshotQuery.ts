import { skipToken, useQuery } from '@tanstack/react-query';
import type { DesktopSnapshot } from '../types';
import { fail } from '../i18n/errors';
export const backendSnapshotKey = ['backend', 'snapshot'] as const;
export const backendSnapshotOptions = {
  queryKey: backendSnapshotKey,
  queryFn: skipToken,
  staleTime: Infinity,
  gcTime: Infinity,
} as const;
const identity = (snapshot: DesktopSnapshot) => snapshot;
/** One cache entry for the atomic Core/host snapshot; selectors never create another copy. */
export function useBackendSnapshotQuery<T = DesktopSnapshot>(
  select: (snapshot: DesktopSnapshot) => T = identity as (snapshot: DesktopSnapshot) => T,
) {
  return useQuery<DesktopSnapshot, Error, T>({ ...backendSnapshotOptions, select });
}
/** Used below ConnectionGate, which only mounts features after the first snapshot. */
export function useSnapshot<T = DesktopSnapshot>(
  select: (snapshot: DesktopSnapshot) => T = identity as (snapshot: DesktopSnapshot) => T,
): T {
  const query = useBackendSnapshotQuery(select);
  if (query.data === undefined) throw fail('backendNotConnected');
  return query.data;
}
