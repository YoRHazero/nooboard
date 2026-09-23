import { afterEach, expect, it, vi } from 'vitest';
import { HistoryQueryState } from './query';
import type { HistoryPage } from '../../desktop/api';
const page = (text: string, hasMore = false): HistoryPage => ({
  items: [{ id: text, text, copiedAt: 0, source: 'local' }],
  hasMore,
});
afterEach(() => vi.useRealTimers());
it('discards old pages after filters change and prevents duplicate pagination requests', async () => {
  vi.useFakeTimers();
  let old!: (page: HistoryPage) => void;
  const fetch = vi
    .fn()
    .mockResolvedValueOnce(page('first', true))
    .mockImplementationOnce(
      () =>
        new Promise((resolve) => {
          old = resolve;
        }),
    )
    .mockResolvedValueOnce(page('new'));
  const query = new HistoryQueryState(fetch);
  query.refresh('', 'all');
  await vi.runAllTimersAsync();
  const pending = query.more();
  void query.more();
  expect(fetch).toHaveBeenCalledTimes(2);
  query.refresh('new', 'remote', 180);
  expect(query.getSnapshot().rows).toEqual([]);
  await vi.runAllTimersAsync();
  old(page('old'));
  await pending;
  expect(query.getSnapshot().rows[0].text).toBe('new');
  expect(fetch).toHaveBeenLastCalledWith({ contains: 'new', source: 'remote', offset: 0 });
});
it('cancels pending debounce and ignores results after unmount', async () => {
  vi.useFakeTimers();
  let resolve!: (page: HistoryPage) => void;
  const fetch = vi.fn(
    () =>
      new Promise<HistoryPage>((done) => {
        resolve = done;
      }),
  );
  const query = new HistoryQueryState(fetch);
  query.refresh('a', 'all', 180);
  query.cancel();
  await vi.runAllTimersAsync();
  expect(fetch).not.toHaveBeenCalled();
  query.refresh('', 'all');
  await vi.runAllTimersAsync();
  query.cancel();
  resolve(page('late'));
  await Promise.resolve();
  expect(query.getSnapshot().rows).toEqual([]);
});
