import type { DesktopBridge, Session } from '../desktop/bridge/port';
import type { Frame, Reply, Request } from '../desktop/bridge/generated';
import { fail } from '../i18n/errors';
import {
  examples,
  exampleReceived,
  exampleFileNames,
  examplePeer,
  nearbySample,
  snapshotFixture,
  hostFixture,
  historyFixture,
} from './scenarios/fixtures';
/** Scripted UI fixtures. Core owns transfer policy, retry, pairing security and retention. */
export class PreviewBridge implements DesktopBridge {
  readonly mode = 'preview';
  private snapshot = snapshotFixture();
  private host = hostFixture();
  private history = historyFixture();
  private listeners = new Set<(frame: Frame) => void>();
  private timers = new Set<ReturnType<typeof setTimeout>>();
  private disposed = false;
  private sequence = 1;
  private initializedPreferences = false;
  async locale() {
    return typeof navigator === 'undefined' ? 'en' : navigator.language;
  }
  async setup() {
    return () => {};
  }
  async connect(receive: (frame: Frame) => void, signal: AbortSignal): Promise<Session> {
    this.ensureOpen();
    if (!signal.aborted) this.listeners.add(receive);
    const close = async () => {
      this.listeners.delete(receive);
      signal.removeEventListener('abort', close);
    };
    signal.addEventListener('abort', close, { once: true });
    return {
      initial: structuredClone({ snapshot: this.snapshot, host: this.host, diagnostic: false }),
      close,
    };
  }
  private ensureOpen() {
    if (this.disposed) throw fail('stopped');
  }
  private publish() {
    this.snapshot.revision = String(BigInt(this.snapshot.revision) + 1n);
    for (const receive of this.listeners)
      receive({ type: 'snapshot', data: structuredClone(this.snapshot) });
  }
  private later(action: () => void) {
    const timer = setTimeout(() => {
      this.timers.delete(timer);
      if (!this.disposed) {
        action();
        this.publish();
      }
    }, 900);
    this.timers.add(timer);
  }
  private record(kind: 'Copied' | 'Received' | 'Sent', summary: string, source?: string) {
    const activity = {
      sequence: String(++this.sequence),
      kind,
      summary,
      source,
      deviceName: source ? examplePeer.deviceName : undefined,
      at: Date.now(),
    };
    this.snapshot.activities.unshift(activity);
    return activity;
  }
  async request(request: Request): Promise<Reply> {
    this.ensureOpen();
    switch (request.type) {
      case 'hostPreferences':
        if (!this.initializedPreferences && request.data.legacyLanguage)
          this.host.language = request.data.legacyLanguage;
        this.initializedPreferences = true;
        this.host = {
          ...this.host,
          ...Object.fromEntries(Object.entries(request.data.patch).filter(([, v]) => v != null)),
          revision: this.host.revision + 1,
        };
        return { type: 'host', data: structuredClone(this.host) };
      case 'acknowledgeNavigation':
        this.host.navigation = null;
        break;
      case 'queryHistory': {
        const q = request.data.query;
        const rows = this.history.filter(
          (row) =>
            row.text.includes(q.contains) &&
            (q.source === 'all' || (q.source === 'local') === (row.source === 'local')),
        );
        return {
          type: 'history',
          data: {
            items: structuredClone(rows.slice(q.offset, q.offset + 32)),
            hasMore: q.offset + 32 < rows.length,
          },
        };
      }
      case 'copyHistory': {
        const row = this.history.find((row) => row.id === request.data.id);
        if (!row) throw fail('historyMissing');
        this.snapshot.current = {
          ...this.snapshot.current,
          kind: 'Text',
          text: row.text,
          files: [],
          source: row.source === 'local' ? null : row.source,
          revision: String(++this.sequence),
          copiedAt: Date.now(),
        };
        break;
      }
      case 'deleteHistory':
        this.history = this.history.filter((row) => row.id !== request.data.id);
        this.changedHistory();
        break;
      case 'clearHistory':
        this.history = [];
        this.changedHistory();
        break;
      case 'updateSyncSettings':
      case 'updateLocalDevice': {
        const patch = Object.fromEntries(
          Object.entries(request.data.patch).filter(([, v]) => v != null),
        );
        if (request.type === 'updateSyncSettings') Object.assign(this.snapshot.settings, patch);
        else Object.assign(this.snapshot.localDevice, patch);
        const revision = String(BigInt(this.snapshot.configuration.savedRevision) + 1n);
        this.snapshot.configuration = {
          savedRevision: revision,
          effectiveRevision: revision,
          restartRequired: false,
        };
        this.publish();
        return { type: 'saved', data: { revision } };
      }
      case 'selectTargets':
        this.snapshot.manualTargets = [...request.data.targets];
        break;
      case 'configurePeer': {
        const peer = this.snapshot.peers.find((p) => p.noobId === request.data.noobId);
        if (!peer) throw fail('offline');
        const patch = request.data.patch;
        if (patch.autoSend != null) peer.settings.autoSend = patch.autoSend;
        if (patch.address)
          peer.settings.address = patch.address.type === 'clear' ? null : patch.address.data;
        break;
      }
      case 'unpair':
        this.snapshot.peers = this.snapshot.peers.filter((p) => p.noobId !== request.data.noobId);
        this.snapshot.manualTargets = this.snapshot.manualTargets.filter(
          (id) => id !== request.data.noobId,
        );
        break;
      case 'sendCurrent': {
        const id = { session: this.snapshot.session, sequence: String(++this.sequence) };
        if (this.snapshot.settings.paused) throw fail('paused');
        if (!this.snapshot.manualTargets.length) throw fail('noTargets');
        this.snapshot.transfers.unshift({
          id,
          bytes: 64,
          automatic: false,
          targets: this.snapshot.manualTargets.map((noobId) => ({
            noobId,
            deviceName: this.snapshot.peers.find((p) => p.noobId === noobId)?.deviceName ?? noobId,
            state: 'Sending',
          })),
        });
        this.snapshot.activities.unshift({
          sequence: String(++this.sequence),
          kind: 'Sent',
          summary: this.snapshot.current.text ?? exampleFileNames.join(', '),
          at: Date.now(),
          messageId: id,
        });
        this.later(() => {
          const transfer = this.snapshot.transfers.find((t) => t.id.sequence === id.sequence);
          transfer?.targets.forEach((target) => {
            target.state = 'Applied';
          });
        });
        this.publish();
        return { type: 'sent', data: id };
      }
      case 'selectFiles':
        await this.sampleFiles();
        return { type: 'done' };
      case 'selectReceiveDirectory':
        this.snapshot.settings.receiveDirectory = '/Downloads/Nooboard';
        break;
      case 'cancelTransfer': {
        const task = this.snapshot.contentTransfers.find((t) => t.key === request.data.key);
        if (task) task.stage = 'Cancelled';
        break;
      }
      case 'copyReceived':
        break;
      case 'discover':
        this.snapshot.onboarding.nearby = [
          {
            key: 'nearby-example',
            noobId: nearbySample.noobId,
            deviceName: nearbySample.deviceName,
            syncPort: 24816,
            addresses: ['192.168.1.64:24817'],
          },
        ];
        break;
      case 'beginPairing':
        this.snapshot.onboarding.session = {
          id: 'pairing-example',
          stage: 'EnteringCode',
          incoming: false,
          noobId: nearbySample.noobId,
          deviceName: nearbySample.deviceName,
          attemptsLeft: 3,
          expiresAt: Date.now() + 120_000,
        };
        break;
      case 'acceptPairing':
        if (this.snapshot.onboarding.session)
          Object.assign(this.snapshot.onboarding.session, { stage: 'ShowingCode', code: '123456' });
        break;
      case 'pairingCode':
        if (this.snapshot.onboarding.session) this.snapshot.onboarding.session.stage = 'Completed';
        if (!this.snapshot.peers.some((p) => p.noobId === nearbySample.noobId))
          this.snapshot.peers.push({
            ...nearbySample,
            settings: { address: '192.168.1.64:24816', autoSend: false },
            online: true,
            accepting: true,
          });
        break;
      case 'dismissPairing':
        this.snapshot.onboarding.session = null;
        break;
      case 'probe':
        if (request.data.action === 'copy') await this.sampleCopy();
        else await this.sampleReceive();
        return { type: 'done' };
      default: {
        const exhaustive: never = request;
        throw new Error(`Unsupported preview request: ${exhaustive}`);
      }
    }
    this.publish();
    return { type: 'done' };
  }
  private changedHistory() {
    this.snapshot.historyRevision = String(BigInt(this.snapshot.historyRevision) + 1n);
  }
  async sampleCopy() {
    this.ensureOpen();
    const text = examples[this.sequence % examples.length];
    const activity = this.record('Copied', text.split('\n')[0]);
    this.snapshot.current = {
      revision: activity.sequence,
      kind: 'Text',
      text,
      files: [],
      source: null,
      copiedAt: activity.at,
    };
    this.history.unshift({
      id: String(1000 + Number(activity.sequence)),
      text,
      source: 'local',
      copiedAt: activity.at,
    });
    this.changedHistory();
    this.publish();
  }
  async sampleReceive(noobId = examplePeer.noobId) {
    this.ensureOpen();
    const text = exampleReceived;
    const activity = this.record('Received', text, noobId);
    this.snapshot.current = {
      revision: activity.sequence,
      kind: 'Text',
      text,
      files: [],
      source: noobId,
      copiedAt: activity.at,
    };
    this.history.unshift({
      id: String(1000 + Number(activity.sequence)),
      text,
      source: noobId,
      copiedAt: activity.at,
    });
    this.changedHistory();
    this.publish();
  }
  async sampleFiles() {
    this.ensureOpen();
    const id = { session: this.snapshot.session, sequence: String(++this.sequence) };
    this.snapshot.contentTransfers.unshift({
      key: `fixture-${id.sequence}`,
      id,
      peer: examplePeer.noobId,
      deviceName: examplePeer.deviceName,
      incoming: true,
      kind: 'Files',
      names: [...exampleFileNames],
      at: Date.now(),
      stage: 'Receiving',
      completedBytes: 512,
      totalBytes: 1024,
      savedPaths: [],
    });
    this.snapshot.activities.unshift({
      sequence: String(++this.sequence),
      kind: 'Received',
      summary: exampleFileNames.join(', '),
      source: examplePeer.noobId,
      deviceName: examplePeer.deviceName,
      contentTask: `fixture-${id.sequence}`,
      contentNode: 'started',
      contentStage: 'Receiving',
      at: Date.now(),
    });
    this.later(() => {
      const task = this.snapshot.contentTransfers.find((t) => t.id.sequence === id.sequence);
      if (task?.stage === 'Receiving') {
        this.snapshot.activities.unshift({
          sequence: String(++this.sequence),
          kind: 'Received',
          summary: exampleFileNames.join(', '),
          source: examplePeer.noobId,
          deviceName: examplePeer.deviceName,
          contentTask: task.key,
          contentNode: 'finished',
          contentStage: 'Completed',
          at: Date.now(),
        });
        Object.assign(task, {
          stage: 'Completed',
          completedBytes: 1024,
          savedPaths: exampleFileNames.map((name) => `/Downloads/Nooboard/${name}`),
        });
      }
    });
    this.publish();
  }
  async toggleConnection(noobId = examplePeer.noobId) {
    const p = this.snapshot.peers.find((p) => p.noobId === noobId);
    if (p) p.online = !p.online;
    this.publish();
  }
  async toggleAccepting(noobId: string) {
    const p = this.snapshot.peers.find((p) => p.noobId === noobId);
    if (p) p.accepting = !p.accepting;
    this.publish();
  }
  async reset() {
    this.ensureOpen();
    this.clearTimers();
    this.snapshot = snapshotFixture();
    this.history = historyFixture();
    this.sequence = 1;
    this.publish();
  }
  private clearTimers() {
    this.timers.forEach(clearTimeout);
    this.timers.clear();
  }
  dispose() {
    this.disposed = true;
    this.clearTimers();
    this.listeners.clear();
  }
}
export type PreviewControls = Pick<
  PreviewBridge,
  'sampleCopy' | 'sampleReceive' | 'sampleFiles' | 'toggleConnection' | 'toggleAccepting' | 'reset'
>;
