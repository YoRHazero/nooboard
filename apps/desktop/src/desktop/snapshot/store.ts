import { fail, type Problem } from '../../i18n/errors';
import type { BackendSnapshot } from '../bridge/generated';
import type { Appearance } from '../preferences/model';
import type { DesktopSnapshot, HostState, DesktopEvent } from './model';
import { decode } from './decode';
import { ActivityCues } from '../activity/cues';

// Share unchanged JSON-shaped branches so feature selectors do not rerender on
// unrelated transfer progress. This never mutates a snapshot already published.
export function share<T>(before: T, next: T): T {
  if (Object.is(before, next)) return before;
  if (
    !before ||
    !next ||
    typeof before !== 'object' ||
    typeof next !== 'object' ||
    Array.isArray(before) !== Array.isArray(next)
  )
    return next;
  const previous = before as Record<string, unknown>;
  const incoming = next as Record<string, unknown>;
  const keys = Object.keys(incoming);
  const result = (Array.isArray(next) ? [] : {}) as Record<string, unknown>;
  let equal = keys.length === Object.keys(previous).length;
  for (const key of keys) {
    result[key] = share(previous[key], incoming[key]);
    equal &&= Object.is(result[key], previous[key]);
  }
  return equal ? before : (result as T);
}
export class SnapshotStore {
  private snapshot: DesktopSnapshot | null = null;
  private revision = -1n;
  private cues = new ActivityCues();
  private listeners = new Set<() => void>();
  private events = new Set<(event: DesktopEvent) => void>();
  constructor(private appearance: Appearance) {}
  getSnapshot = () => {
    if (!this.snapshot) throw fail('backendNotConnected');
    return this.snapshot;
  };
  get hasSnapshot() {
    return this.snapshot !== null;
  }
  subscribe = (listener: () => void) => {
    this.listeners.add(listener);
    return () => {
      this.listeners.delete(listener);
    };
  };
  onEvent = (listener: (event: DesktopEvent) => void) => {
    this.events.add(listener);
    return () => {
      this.events.delete(listener);
    };
  };
  private publish(next: DesktopSnapshot) {
    this.snapshot = share(this.snapshot, next);
    this.listeners.forEach((fn) => fn());
  }
  setHost(host: HostState) {
    if (!this.snapshot || host.revision <= this.snapshot.host.revision) return;
    this.publish({ ...this.snapshot, host });
  }
  setAppearance(appearance: Appearance) {
    this.appearance = appearance;
    if (this.snapshot) this.publish({ ...this.snapshot, appearance });
  }
  setError(connectionError?: Problem) {
    if (this.snapshot) this.publish({ ...this.snapshot, connectionError });
  }
  apply(frame: BackendSnapshot, host: HostState, recovering = false) {
    const same = this.snapshot?.session === frame.session;
    if (same && BigInt(frame.revision) <= this.revision) return false;
    const before = this.snapshot;
    const next = decode(frame, host, this.appearance);
    this.revision = BigInt(frame.revision);
    const events = this.cues.project(frame, next, before, recovering);
    this.publish(next);
    events.forEach((event) => this.events.forEach((listener) => listener(event)));
    return true;
  }
  dispose() {
    this.listeners.clear();
    this.events.clear();
  }
}
