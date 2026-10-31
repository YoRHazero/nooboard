import { afterEach, expect, it, vi } from 'vitest';
import { QueryClient } from '@tanstack/react-query';
import { backendRequest } from '../../../shared/backend';
import { historyQueryOptions } from './useHistoryQuery';
vi.mock('../../../shared/backend', () => ({ backendRequest: vi.fn() }));
const clients: QueryClient[] = [];
const client = () => {
  const value = new QueryClient({
    defaultOptions: { queries: { retry: false, networkMode: 'always', gcTime: Infinity } },
  });
  clients.push(value);
  return value;
};
const page = (text: string, hasMore = false) => ({
  type: 'history' as const,
  data: { items: [{ id: text, text, copiedAt: 0, source: 'local' }], hasMore },
});
afterEach(() => {
  clients.forEach((query) => query.clear());
  clients.length = 0;
  vi.resetAllMocks();
});
it('deduplicates requests for the same history filter and isolates a late result after changing filters', async () => {
  const queryClient = client();
  let finish!: (value: ReturnType<typeof page>) => void;
  vi.mocked(backendRequest).mockImplementationOnce(
    () =>
      new Promise((resolve) => {
        finish = resolve;
      }),
  );
  vi.mocked(backendRequest).mockResolvedValueOnce(page('new'));
  const oldOptions = historyQueryOptions('session', '', 'all');
  const first = queryClient.fetchInfiniteQuery(oldOptions);
  const duplicate = queryClient.fetchInfiniteQuery(oldOptions);
  expect(backendRequest).toHaveBeenCalledTimes(1);
  const nextOptions = historyQueryOptions('session', 'new', 'remote');
  const next = await queryClient.fetchInfiniteQuery(nextOptions);
  finish(page('old'));
  await Promise.all([first, duplicate]);
  expect(next.pages[0].items[0].text).toBe('new');
  expect(queryClient.getQueryData(nextOptions.queryKey)).toBe(next);
  expect(backendRequest).toHaveBeenLastCalledWith(
    { type: 'queryHistory', data: { query: { contains: 'new', source: 'remote', offset: 0 } } },
    'history',
  );
});
it('cancels a query without letting late IPC completion populate its cache', async () => {
  const queryClient = client();
  let finish!: (value: ReturnType<typeof page>) => void;
  vi.mocked(backendRequest).mockImplementationOnce(
    () =>
      new Promise((resolve) => {
        finish = resolve;
      }),
  );
  const options = historyQueryOptions('session', '', 'all');
  const result = queryClient.fetchInfiniteQuery(options).catch(() => undefined);
  await queryClient.cancelQueries({ queryKey: options.queryKey });
  finish(page('late'));
  await result;
  expect(queryClient.getQueryData(options.queryKey)).toBeUndefined();
});
