import { queryOptions, useQuery } from '@tanstack/react-query';
import { terminalRequest } from '../../../shared/backend';
import { useSnapshot } from '../../../shared/queries/useBackendSnapshotQuery';
import { useConnection } from '../../../shared/store';
import { fail } from '../../../shared/i18n/errors';
import { terminalKeys } from './keys';
export function remoteDirectoryQueryOptions(session: string, id: string, path?: string) {
  return queryOptions({
    queryKey: terminalKeys.directory(session, id, path),
    staleTime: 0,
    queryFn: async ({ signal }) => {
      const reply = await terminalRequest({ type: 'browse', data: { id, path } });
      signal.throwIfAborted();
      if (reply.type !== 'terminalDirectory') throw fail('generic');
      return reply.data;
    },
  });
}
export function useRemoteDirectoryQuery(
  id: string,
  path?: string,
  options: { enabled?: boolean; staleTime?: number } = {},
) {
  const session = useSnapshot((snapshot) => snapshot.session);
  const running = useSnapshot((snapshot) =>
    snapshot.terminal.runs.some((run) => run.target.id === id && run.phase === 'running'),
  );
  const connection = useConnection();
  return useQuery({
    ...remoteDirectoryQueryOptions(session, id, path),
    staleTime: options.staleTime ?? 10_000,
    enabled: options.enabled !== false && running && connection.phase === 'ready',
  });
}
