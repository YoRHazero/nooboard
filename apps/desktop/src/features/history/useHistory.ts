import { useEffect, useMemo, useState, useSyncExternalStore } from 'react';
import { useDesktop, useSnapshot, useConnection, type HistoryQuery } from '../../desktop/api';
import { HistoryQueryState } from './query';
export function useHistory(contains: string, source: HistoryQuery['source']) {
  const desktop = useDesktop();
  const revision = useSnapshot((snapshot) => snapshot.historyRevision);
  const connection = useConnection();
  const query = useMemo(
    () => new HistoryQueryState((request) => desktop.queryHistory(request)),
    [desktop],
  );
  const [retry, setRetry] = useState(0);
  useEffect(() => {
    query.refresh(contains, source, contains ? 180 : 0);
    return () => query.cancel();
  }, [query, contains, source, revision, connection.phase, retry]);
  const state = useSyncExternalStore(query.subscribe, query.getSnapshot);
  return { ...state, more: query.more, retry: () => setRetry((n) => n + 1) };
}
