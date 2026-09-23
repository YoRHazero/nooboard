import type { HistoryPage, HistoryQuery, TextItem } from '../../desktop/api';
import { toProblem, type Problem } from '../../i18n/errors';
interface QueryState {
  rows: TextItem[];
  hasMore: boolean;
  loading: boolean;
  error: Problem | null;
}
/** Owns one page query, including invalidation and pagination; never owns stored history. */
export class HistoryQueryState {
  private generation = 0;
  private state: QueryState = { rows: [], hasMore: false, loading: true, error: null };
  private query: HistoryQuery = { contains: '', source: 'all', offset: 0 };
  private timer: ReturnType<typeof setTimeout> | undefined;
  private listeners = new Set<() => void>();
  constructor(private fetch: (query: HistoryQuery) => Promise<HistoryPage>) {}
  getSnapshot = () => this.state;
  subscribe = (listener: () => void) => {
    this.listeners.add(listener);
    return () => {
      this.listeners.delete(listener);
    };
  };
  private publish(next: QueryState) {
    this.state = next;
    this.listeners.forEach((fn) => fn());
  }
  refresh(contains: string, source: HistoryQuery['source'], delay = 0) {
    this.cancel();
    this.query = { contains, source, offset: 0 };
    this.publish({ rows: [], hasMore: false, loading: true, error: null });
    const generation = this.generation;
    this.timer = setTimeout(() => {
      this.timer = undefined;
      void this.load(generation, 0);
    }, delay);
  }
  more = async () => {
    if (this.state.loading || !this.state.hasMore) return;
    this.publish({ ...this.state, loading: true, error: null });
    await this.load(this.generation, this.state.rows.length);
  };
  private async load(generation: number, offset: number) {
    try {
      const page = await this.fetch({ ...this.query, offset });
      if (generation !== this.generation) return;
      this.publish({
        rows: offset ? [...this.state.rows, ...page.items] : page.items,
        hasMore: page.hasMore,
        loading: false,
        error: null,
      });
    } catch (error) {
      if (generation === this.generation)
        this.publish({ ...this.state, loading: false, error: toProblem(error) });
    }
  }
  cancel() {
    this.generation++;
    clearTimeout(this.timer);
    this.timer = undefined;
  }
}
