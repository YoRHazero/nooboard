import { type Problem, fail } from '../i18n/errors';
import type {
  Activity,
  DeliveryState,
  DesktopEvent,
  DesktopSnapshot,
  Settings,
} from '../api/contracts';
import { batchState } from '../api/deliveries';
import { messageKey, type NativeDeliveryState, type NativeSnapshot } from './wire';

const deliveryStates: Record<NativeDeliveryState, DeliveryState> = {
  Queued: 'queued',
  Sending: 'sending',
  AwaitingReceipt: 'awaitingReceipt',
  Applied: 'applied',
  Rejected: 'rejected',
  Unconfirmed: 'unconfirmed',
  Offline: 'offline',
  Cancelled: 'cancelled',
  Superseded: 'superseded',
  QueueFull: 'queueFull',
};
export type Appearance = Pick<Settings, 'theme' | 'reducedMotion'>;
/** Maps authoritative snapshots to presentation identities; never infers receipt success. */
export class NativeStore {
  private snapshot: DesktopSnapshot | null = null;
  private session = '';
  private revision = -1n;
  private watermark = 0n;
  private identities = new Map<string, number>();
  private sequence = 0;
  private listeners = new Set<() => void>();
  private events = new Set<(event: DesktopEvent) => void>();
  constructor(private appearance: Appearance) {}
  getSnapshot = () => {
    if (!this.snapshot) throw fail('backendNotConnected');
    return this.snapshot;
  };
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
  private notify() {
    this.listeners.forEach((fn) => fn());
  }
  private emit(event: DesktopEvent) {
    this.events.forEach((fn) => fn(event));
  }
  setAppearance(appearance: Appearance) {
    this.appearance = appearance;
    if (this.snapshot) {
      this.snapshot = { ...this.snapshot, settings: { ...this.snapshot.settings, ...appearance } };
      this.notify();
    }
  }
  connected() {
    if (this.snapshot?.connectionError) {
      this.snapshot = { ...this.snapshot, connectionError: undefined };
      this.notify();
    }
  }
  stopped(message: Problem) {
    if (this.snapshot) {
      this.snapshot = { ...this.snapshot, connectionError: message };
      this.notify();
    }
  }
  apply(frame: NativeSnapshot, recovering = false) {
    const same = this.session === frame.session;
    if (same && BigInt(frame.revision) <= this.revision) return;
    const first = !this.snapshot;
    const before = this.snapshot;
    if (!same) {
      this.identities.clear();
      this.watermark = 0n;
    }
    const initial = first || !same || recovering;
    const transfers = new Map(frame.status.transfers.map((t) => [messageKey(t.id), t]));
    const ids = new Map<string, number>();
    const activities: Activity[] = frame.activities.map((record) => {
      const id = this.identities.get(record.sequence) ?? ++this.sequence;
      ids.set(record.sequence, id);
      const transfer =
        record.kind === 'Sent' && record.message_id
          ? transfers.get(messageKey(record.message_id))
          : undefined;
      const targets = transfer?.targets.map((t) => ({
        noobId: t.noob_id,
        deviceName: t.device_name,
        state: deliveryStates[t.state],
      }));
      return {
        id,
        messageId: record.message_id ?? undefined,
        kind: record.kind === 'Sent' ? 'sent' : record.kind === 'Copied' ? 'copied' : 'received',
        sourceNoobId: record.source,
        sourceName: record.device_name ?? undefined,
        title: record.summary,
        at: record.at_ms,
        targets,
        automatic: transfer?.automatic,
        state: record.kind === 'Sent' ? (targets ? batchState(targets) : 'unconfirmed') : 'applied',
      };
    });
    this.identities = ids;
    this.session = frame.session;
    this.revision = BigInt(frame.revision);
    const { status, current } = frame;
    const onboarding = frame.onboarding;
    this.snapshot = {
      onboarding: onboarding
        ? {
            nearby: onboarding.nearby.map((d) => ({
              key: d.key,
              noobId: d.noob_id,
              deviceName: d.device_name,
              addresses: d.addresses,
              syncPort: d.sync_port,
            })),
            error: onboarding.discovery_error,
            session: onboarding.session
              ? {
                  id: onboarding.session.id,
                  incoming: onboarding.session.incoming,
                  deviceName: onboarding.session.device_name,
                  noobId: onboarding.session.noob_id,
                  stage: onboarding.session.stage,
                  code: onboarding.session.code,
                  expiresAt: onboarding.session.expires_at_ms,
                  attemptsLeft: onboarding.session.attempts_left,
                  error: onboarding.session.error,
                }
              : null,
          }
        : undefined,
      localDevice: {
        platform: frame.platform ?? undefined,
        noobId: status.noob_id,
        deviceName: status.settings.device_name,
        fingerprint: status.fingerprint,
        syncPort: frame.local_network.sync_port,
        pairingPort: frame.local_network.pairing_port,
        addresses: frame.local_network.addresses.map((address) => ({
          interface: address.interface,
          ip: address.ip,
          pairingAddress: address.pairing_address,
        })),
        addressError: frame.local_network.error,
      },
      current: {
        id: `${frame.session}:${current.revision}`,
        text: current.text ?? '',
        kind: current.kind,
        source: current.source ? 'remote' : 'local',
        sourceNoobId: current.source ?? undefined,
        copiedAt: current.copied_at_ms,
      },
      peers: status.peers.map((p) => ({
        noobId: p.noob_id,
        deviceName: p.device_name,
        fingerprint: p.fingerprint,
        settings: { address: p.settings.address, autoSend: p.settings.auto_send },
        online: p.online,
        accepting: p.accepting,
      })),
      manualTargets: status.manual_targets,
      history: [],
      historyRevision: `${frame.session}:${frame.history_revision}`,
      activities,
      settings: {
        discoverable: status.settings.discoverable,
        mode: status.settings.mode === 'Automatic' ? 'automatic' : 'manual',
        receive: status.settings.receive,
        paused: status.settings.paused,
        history: status.settings.history,
        historyDays: status.settings.history_days,
        maxHistoryEntries: status.settings.max_history_entries,
        ...this.appearance,
      },
      notice: frame.fault
        ? { id: `${frame.session}:${frame.fault.sequence}`, message: frame.fault.message }
        : undefined,
    };
    this.notify();
    if (!same && !first) this.emit({ type: 'reset' });
    for (let i = frame.activities.length - 1; i >= 0; i--) {
      const record = frame.activities[i];
      const activity = activities[i];
      const sequence = BigInt(record.sequence);
      if (!initial && sequence > this.watermark)
        this.emit({
          type:
            activity.kind === 'copied' ? 'copied' : activity.kind === 'sent' ? 'sent' : 'received',
          sequence: activity.id,
        });
      else if (!initial && activity.kind === 'sent') {
        const previous = before?.activities.find((a) => a.id === activity.id);
        if (previous && previous.state !== activity.state && activity.state === 'applied')
          this.emit({ type: 'applied', sequence: activity.id });
        else if (
          previous?.state === 'pending' &&
          ['rejected', 'partial', 'unconfirmed'].includes(activity.state)
        )
          this.emit({ type: 'rejected', sequence: activity.id });
      }
    }
    for (const record of frame.activities)
      if (BigInt(record.sequence) > this.watermark) this.watermark = BigInt(record.sequence);
  }
}
