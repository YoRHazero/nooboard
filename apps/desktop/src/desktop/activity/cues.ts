import type { BackendSnapshot } from '../bridge/generated';
import type { DesktopSnapshot, DesktopEvent } from '../snapshot/model';
/** A recoverable snapshot updates the watermark without replaying old performances. */
export class ActivityCues {
  private session = '';
  private watermark = 0n;
  project(
    frame: BackendSnapshot,
    next: DesktopSnapshot,
    before: DesktopSnapshot | null,
    recovering: boolean,
  ): DesktopEvent[] {
    const changedSession = this.session !== frame.session;
    const suppress = !before || changedSession || recovering;
    if (changedSession) this.watermark = 0n;
    const events: DesktopEvent[] = changedSession && before ? [{ type: 'reset' }] : [];
    this.session = frame.session;
    for (let i = frame.activities.length - 1; i >= 0; i--) {
      const record = frame.activities[i];
      const activity = next.activities[i];
      if (!suppress && BigInt(record.sequence) > this.watermark) {
        events.push({
          type:
            activity.contentNode === 'finished'
              ? activity.state === 'applied'
                ? activity.kind === 'sent'
                  ? 'applied'
                  : 'received'
                : 'rejected'
              : activity.kind,
          sequence: activity.id,
        });
      } else if (!suppress && activity.kind === 'sent') {
        const previous = before?.activities.find((a) => a.id === activity.id);
        if (previous && previous.state !== activity.state && activity.state === 'applied')
          events.push({ type: 'applied', sequence: activity.id });
        else if (
          previous?.state === 'pending' &&
          ['rejected', 'partial', 'unconfirmed'].includes(activity.state)
        )
          events.push({ type: 'rejected', sequence: activity.id });
      }
    }
    for (const record of frame.activities)
      if (BigInt(record.sequence) > this.watermark) this.watermark = BigInt(record.sequence);
    return events;
  }
}
