import { type QueryClient } from '@tanstack/react-query';
import { backendSnapshotKey } from '../shared/queries/useBackendSnapshotQuery';
import { fail } from '../shared/i18n/errors';
import type { BackendSnapshot } from '../shared/contracts.generated';
import type { DesktopSnapshot, HostState, DesktopEvent } from '../shared/models';
import { decode } from '../shared/decode-snapshot';
import { ActivityCues } from '../shared/activity-cues';

export class SnapshotProjection {
  private cues = new ActivityCues();
  private events = new Set<(event: DesktopEvent) => void>();
  constructor(readonly queryClient: QueryClient) {}
  private get snapshot() {
    return this.queryClient.getQueryData<DesktopSnapshot>(backendSnapshotKey) ?? null;
  }
  getSnapshot = () => {
    if (!this.snapshot) throw fail('backendNotConnected');
    return this.snapshot;
  };
  get hasSnapshot() {
    return this.snapshot !== null;
  }
  onEvent = (listener: (event: DesktopEvent) => void) => {
    this.events.add(listener);
    return () => {
      this.events.delete(listener);
    };
  };
  private publish(next: DesktopSnapshot) {
    this.queryClient.setQueryData(backendSnapshotKey, next);
  }
  setHost(host: HostState) {
    if (!this.snapshot || host.revision <= this.snapshot.host.revision) return;
    this.publish({ ...this.snapshot, host });
  }
  apply(frame: BackendSnapshot, host: HostState, recovering = false) {
    const same = this.snapshot?.session === frame.session;
    if (same && BigInt(frame.revision) <= BigInt(this.snapshot!.revision)) return false;
    const before = this.snapshot;
    const next = decode(frame, host);
    const events = this.cues.project(frame, next, before, recovering);
    this.publish(next);
    if (before && before.session !== next.session) {
      this.queryClient.removeQueries({
        predicate: (query) =>
          query.queryKey[0] !== 'backend' && query.queryKey.includes(before.session),
      });
    } else if (before && before.historyRevision !== next.historyRevision) {
      void this.queryClient.invalidateQueries({ queryKey: ['history', next.session] });
    }
    const live = new Set(next.terminal.runs.map((run) => run.target.id));
    this.queryClient.removeQueries({
      predicate: (query) =>
        query.queryKey[0] === 'terminal' &&
        query.queryKey[1] === 'directory' &&
        !live.has(String(query.queryKey[3])),
    });
    events.forEach((event) => this.events.forEach((listener) => listener(event)));
    return true;
  }
  dispose() {
    this.events.clear();
  }
}
