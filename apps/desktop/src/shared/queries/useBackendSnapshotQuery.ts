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
/**
 * One cache entry for the atomic Core/host snapshot; selectors never create another copy.
 * A selector is required: structural sharing keeps an unchanged selection stable, so a
 * component re-renders only when the part it reads changes, not on every snapshot.
 */
export function useBackendSnapshotQuery<T>(select: (snapshot: DesktopSnapshot) => T) {
  return useQuery<DesktopSnapshot, Error, T>({ ...backendSnapshotOptions, select });
}
/** Used below ConnectionGate, which only mounts features after the first snapshot. */
export function useSnapshot<T>(select: (snapshot: DesktopSnapshot) => T): T {
  const query = useBackendSnapshotQuery(select);
  if (query.data === undefined) throw fail('backendNotConnected');
  return query.data;
}
