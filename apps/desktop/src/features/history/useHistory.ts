import { type Problem, toProblem } from '../../i18n/errors';
import { useEffect, useRef, useState } from 'react';
import { useClient, useSnapshot } from '../../api/NooboardProvider';
import type { HistoryQuery, TextItem } from '../../api/contracts';
/** Filter before pagination; old responses cannot replace a newer query. */
export function useHistory(contains: string, source: HistoryQuery['source']) {
  const client = useClient();
  const snapshot = useSnapshot();
  const revision = snapshot.historyRevision ?? snapshot.history;
  const [rows, setRows] = useState<TextItem[]>([]);
  const [hasMore, setHasMore] = useState(false);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<Problem | null>(null);
  const [retry, setRetry] = useState(0);
  const generation = useRef(0);
  useEffect(() => {
    const current = ++generation.current;
    setLoading(true);
    setError(null);
    const timer = setTimeout(
      () => {
        void client
          .queryHistory({ contains, source, offset: 0 })
          .then((page) => {
            if (generation.current !== current) return;
            setRows(page.items);
            setHasMore(page.hasMore);
          })
          .catch((reason) => {
            if (generation.current === current) setError(toProblem(reason));
          })
          .finally(() => {
            if (generation.current === current) setLoading(false);
          });
      },
      contains ? 180 : 0,
    );
    return () => {
      clearTimeout(timer);
      generation.current++;
    };
  }, [client, contains, source, revision, retry]);
  const more = async () => {
    if (loading) return;
    const current = generation.current;
    setLoading(true);
    setError(null);
    try {
      const page = await client.queryHistory({ contains, source, offset: rows.length });
      if (generation.current === current) {
        setRows((before) => [...before, ...page.items]);
        setHasMore(page.hasMore);
      }
    } catch (reason) {
      if (generation.current === current) setError(toProblem(reason));
    } finally {
      if (generation.current === current) setLoading(false);
    }
  };
  return { rows, hasMore, loading, error, more, retry: () => setRetry((n) => n + 1) };
}
