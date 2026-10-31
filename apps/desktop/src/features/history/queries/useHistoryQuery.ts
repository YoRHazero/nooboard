import { useEffect, useState } from 'react';
import { infiniteQueryOptions, useInfiniteQuery } from '@tanstack/react-query';
import { backendRequest } from '../../../shared/backend';
import { useSnapshot } from '../../../shared/queries/useBackendSnapshotQuery';
import { useConnection } from '../../../shared/store';
import { toProblem } from '../../../shared/i18n/errors';
import type { HistoryPage, HistoryQuery } from '../../../shared/types';
import { historyKeys } from './keys';
export function historyQueryOptions(
  session: string,
  contains: string,
  source: HistoryQuery['source'],
) {
  return infiniteQueryOptions({
    queryKey: historyKeys.list(session, contains, source),
    initialPageParam: 0,
    queryFn: async ({ pageParam, signal }): Promise<HistoryPage> => {
      const reply = await backendRequest(
        { type: 'queryHistory', data: { query: { contains, source, offset: pageParam } } },
        'history',
      );
      signal.throwIfAborted();
      return {
        hasMore: reply.data.hasMore,
        items: reply.data.items.map((row) => ({
          ...row,
          source: row.source === 'local' ? 'local' : 'remote',
          sourceNoobId: row.source === 'local' ? undefined : row.source,
        })),
      };
    },
    getNextPageParam: (last, pages) =>
      last.hasMore ? pages.reduce((sum, page) => sum + page.items.length, 0) : undefined,
  });
}
export function useHistoryQuery(contains: string, source: HistoryQuery['source']) {
  const session = useSnapshot((snapshot) => snapshot.session);
  const connection = useConnection();
  const [debounced, setDebounced] = useState(contains);
  useEffect(() => {
    const timer = setTimeout(() => setDebounced(contains), contains ? 180 : 0);
    return () => clearTimeout(timer);
  }, [contains]);
  const query = useInfiniteQuery({
    ...historyQueryOptions(session, debounced, source),
    enabled: connection.phase === 'ready',
  });
  const waiting = debounced !== contains;
  return {
    rows: waiting ? [] : (query.data?.pages.flatMap((page) => page.items) ?? []),
    hasMore: !waiting && query.hasNextPage,
    loading: waiting || query.isPending || query.isFetching,
    error: query.error ? toProblem(query.error) : null,
    more: () => query.fetchNextPage({ cancelRefetch: false }),
    retry: () => void query.refetch(),
  };
}
