import type { Activity } from '../../desktop/api';

interface InboxSnapshot {
  latest: Activity | null;
  batch: readonly Activity[];
}

/** Session summaries; playback supplies grouping boundaries, never transfer outcomes. */
export class ActivityInbox {
  private state: InboxSnapshot;
  private performing = false;
  private anchor: Activity | null = null;
  private listeners = new Set<() => void>();
  constructor(latest: Activity | null) {
    this.state = { latest, batch: [] };
  }
  getSnapshot = () => this.state;
  subscribe = (listener: () => void) => {
    this.listeners.add(listener);
    return () => {
      this.listeners.delete(listener);
    };
  };
  private publish(state: InboxSnapshot) {
    this.state = state;
    this.listeners.forEach((listener) => listener());
  }
  performance(active: boolean, activity: Activity | null) {
    this.performing = active;
    this.anchor = active ? activity : null;
  }
  record(activity: Activity) {
    if (
      this.state.latest?.id === activity.id ||
      this.state.batch.some((row) => row.id === activity.id)
    ) {
      this.sync([activity]);
      return;
    }
    let batch = this.state.batch;
    if (batch.length) batch = [activity, ...batch];
    else if (this.performing && this.anchor && this.anchor.id !== activity.id)
      batch = [activity, this.anchor];
    if (this.performing && !this.anchor) this.anchor = activity;
    this.publish({ latest: activity, batch });
  }
  sync(activities: readonly Activity[]) {
    const byId = new Map(activities.map((row) => [row.id, row]));
    const update = (row: Activity) => byId.get(row.id) ?? row;
    const latest = this.state.latest ? update(this.state.latest) : null;
    const batch = this.state.batch.map(update);
    if (this.anchor) this.anchor = update(this.anchor);
    if (latest !== this.state.latest || batch.some((row, index) => row !== this.state.batch[index]))
      this.publish({ latest, batch });
  }
  acknowledge = () => {
    this.anchor = null;
    if (this.state.batch.length) this.publish({ ...this.state, batch: [] });
  };
  reset(latest: Activity | null) {
    this.performing = false;
    this.anchor = null;
    this.publish({ latest, batch: [] });
  }
}
