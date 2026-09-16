import { sampleNote } from './seed';
import { fail } from '../i18n/errors';
import type {
  Activity,
  EntryId,
  HistoryQuery,
  HistoryPage,
  DesktopClient,
  DesktopEvent,
  DesktopSnapshot,
  DeliveryState,
  LocalDevicePatch,
  Peer,
  PeerSettings,
  Settings,
  TextItem,
} from '../api/contracts';
import { createSeed, examples } from './seed';
import { activitySummary } from '../api/activity';
import { batchState, deliveryPending } from '../api/deliveries';
import { canSend, validAddress, parsePort, validName } from '../api/devices';
import { PreviewOnboarding } from './PreviewOnboarding';
import { PreviewTransfers } from './PreviewTransfers';

/** In-memory protocol-v2 interaction preview; never opens sockets or native clipboard access. */
export class PreviewClient implements DesktopClient {
  readonly mode = 'preview' as const;
  async queryHistory(query: HistoryQuery): Promise<HistoryPage> {
    const rows = this.snapshot.history.filter(
      (item) =>
        (query.source === 'all' || item.source === query.source) &&
        item.text.includes(query.contains),
    );
    return {
      items: rows.slice(query.offset, query.offset + 32),
      hasMore: rows.length > query.offset + 32,
    };
  }
  private snapshot = createSeed();
  private listeners = new Set<() => void>();
  private events = new Set<(event: DesktopEvent) => void>();
  private sequence = 100;
  private sample = 0;
  private timers = new Set<ReturnType<typeof setTimeout>>();
  private generations = new Map<string, number>();
  private transfers = new PreviewTransfers((activity, peer, state) =>
    this.delivery(activity, peer, state),
  );
  private onboarding = new PreviewOnboarding(
    () => this.snapshot,
    (onboarding) => this.publish({ onboarding }),
    (peer) => this.savePeer(peer),
  );
  discover = () => this.onboarding.discover();
  beginPairing = (address: string, expected?: string) => this.onboarding.begin(address, expected);
  acceptPairing = (id: string) => this.onboarding.accept(id);
  submitPairingCode = (id: string, code: string) => this.onboarding.submit(id, code);
  dismissPairing = (id: string) => this.onboarding.dismiss(id);
  getSnapshot = () => this.snapshot;
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
  private publish(patch: Partial<DesktopSnapshot>) {
    this.snapshot = { ...this.snapshot, ...patch };
    this.listeners.forEach((listener) => listener());
  }
  private emit(event: DesktopEvent) {
    this.events.forEach((listener) => listener(event));
  }
  private withActivity(activity: Activity) {
    return [activity, ...this.snapshot.activities].filter(
      (row, index) => index < 30 || row.state === 'pending',
    );
  }
  private remember(item: TextItem) {
    if (!this.snapshot.settings.history) return;
    this.publish({
      history: [item, ...this.snapshot.history.filter((row) => row.text !== item.text)].slice(
        0,
        this.snapshot.settings.maxHistoryEntries,
      ),
    });
  }
  private async copy(text: string) {
    const item: TextItem = { id: ++this.sequence, text, source: 'local', copiedAt: Date.now() };
    this.publish({
      current: item,
      activities: this.withActivity({
        id: Number(item.id),
        sourceNoobId: null,
        kind: 'copied',
        state: 'applied',
        title: activitySummary(text),
        at: item.copiedAt,
      }),
    });
    this.remember(item);
    this.emit({ type: 'copied', sequence: Number(item.id) });
    if (this.snapshot.settings.mode === 'automatic' && !this.snapshot.settings.paused) {
      const targets = this.snapshot.peers
        .filter((p) => p.settings.autoSend && canSend(p))
        .map((p) => p.noobId);
      if (targets.length) this.send(targets, true);
    }
  }
  private send(noobIds: readonly string[], automatic: boolean) {
    if (this.snapshot.settings.paused) throw fail('paused');
    if (!noobIds.length) throw fail('noTargets');
    if (!this.snapshot.current.text) throw fail('ineligible');
    const sequence = ++this.sequence;
    const targets = noobIds.map((noobId) => {
      const peer = this.snapshot.peers.find((p) => p.noobId === noobId);
      if (!peer) throw fail('peerRemoved');
      return {
        noobId,
        deviceName: peer.deviceName,
        state: canSend(peer) ? ('queued' as const) : ('offline' as const),
      };
    });
    const activity: Activity = {
      id: sequence,
      sourceNoobId: null,
      targets,
      automatic,
      kind: 'sent',
      state: batchState(targets),
      title: activitySummary(this.snapshot.current.text),
      at: Date.now(),
    };
    this.publish({ activities: this.withActivity(activity) });
    this.emit({ type: 'sent', sequence });
    targets
      .filter((d) => d.state === 'queued')
      .forEach((d) => this.transfers.enqueue({ activity: sequence, peer: d.noobId, automatic }));
  }
  private delivery(id: number, peer: string, state: DeliveryState) {
    const before = this.snapshot.activities.find((a) => a.id === id);
    if (!before || !before.targets?.some((d) => d.noobId === peer && deliveryPending(d.state)))
      return;
    const targets = before.targets.map((d) => (d.noobId === peer ? { ...d, state } : d));
    const next = { ...before, targets, state: batchState(targets) };
    this.publish({ activities: this.snapshot.activities.map((a) => (a.id === id ? next : a)) });
    if (before.state === 'pending' && next.state !== 'pending' && next.state !== 'cancelled')
      this.emit({ type: next.state === 'applied' ? 'applied' : 'rejected', sequence: id });
  }
  async sendCurrent() {
    this.send(this.snapshot.manualTargets, false);
  }
  async selectTargets(noobIds: string[]) {
    if (
      new Set(noobIds).size !== noobIds.length ||
      noobIds.some((id) => !this.snapshot.peers.some((p) => p.noobId === id))
    )
      throw fail('invalidTargets');
    this.publish({ manualTargets: [...noobIds] });
  }
  async configurePeer(noobId: string, patch: Partial<PeerSettings>) {
    const peer = this.snapshot.peers.find((p) => p.noobId === noobId);
    if (!peer) throw fail('peerRemoved');
    const settings = { ...peer.settings, ...patch };
    if (settings.address && !validAddress(settings.address)) throw fail('invalidAddress');
    this.publish({
      peers: this.snapshot.peers.map((p) => (p.noobId === noobId ? { ...p, settings } : p)),
    });
    if (!settings.autoSend) this.transfers.cancelQueued(noobId, true);
  }
  async updateLocalDevice(patch: LocalDevicePatch) {
    const localDevice = { ...this.snapshot.localDevice, ...patch };
    if (!validName(localDevice.deviceName)) throw fail('invalidName');
    if (!parsePort(String(localDevice.pairingPort))) throw fail('invalidPort');
    if (localDevice.syncPort === localDevice.pairingPort) throw fail('portInUse');
    localDevice.addresses = localDevice.addresses.map((address) => ({
      ...address,
      pairingAddress: `${address.ip.includes(':') ? `[${address.ip}]` : address.ip}:${localDevice.pairingPort}`,
    }));
    this.publish({ localDevice });
  }
  async copyHistory(id: EntryId) {
    const item = this.snapshot.history.find((row) => row.id === id);
    if (!item) throw fail('historyMissing');
    await this.copy(item.text);
  }
  async deleteHistory(id: EntryId) {
    this.publish({ history: this.snapshot.history.filter((row) => row.id !== id) });
  }
  async clearHistory() {
    this.publish({ history: [] });
  }
  async updateSettings(patch: Partial<Settings>) {
    const settings = { ...this.snapshot.settings, ...patch };
    const earliest = Date.now() - settings.historyDays * 86_400_000;
    this.publish({
      settings,
      history: this.snapshot.history
        .filter((row) => row.copiedAt >= earliest)
        .slice(0, settings.maxHistoryEntries),
    });
    if (settings.paused || settings.mode === 'manual')
      this.snapshot.peers.forEach((p) => this.transfers.cancelQueued(p.noobId, !settings.paused));
  }
  private async savePeer(request: Omit<Peer, 'online' | 'accepting'>) {
    if (request.noobId === this.snapshot.localDevice.noobId) throw fail('selfPairing');
    const existing = this.snapshot.peers.find((p) => p.noobId === request.noobId);
    if (!existing && this.snapshot.peers.length >= 64) throw fail('peerLimit');
    if (
      !validName(request.deviceName) ||
      !/^[a-f0-9]{64}$/.test(request.noobId) ||
      !/^[a-f0-9]{64}$/.test(request.fingerprint) ||
      (request.settings.address && !validAddress(request.settings.address))
    )
      throw fail('pairingProtocol');
    const { settings, ...identity } = request;
    const generation = (this.generations.get(request.noobId) ?? 0) + 1;
    this.generations.set(request.noobId, generation);
    this.publish({
      peers: [
        ...this.snapshot.peers.filter((p) => p.noobId !== request.noobId),
        {
          ...identity,
          settings: { ...settings, autoSend: existing?.settings.autoSend ?? false },
          online: false,
          accepting: false,
        },
      ],
    });
    const timer = setTimeout(() => {
      this.timers.delete(timer);
      if (generation !== this.generations.get(request.noobId)) return;
      this.publish({
        peers: this.snapshot.peers.map((p) =>
          p.noobId === request.noobId ? { ...p, online: true, accepting: true } : p,
        ),
      });
    }, 1000);
    this.timers.add(timer);
  }
  async unpair(noobId: string) {
    this.disconnect(noobId);
    this.publish({
      peers: this.snapshot.peers.filter((p) => p.noobId !== noobId),
      manualTargets: this.snapshot.manualTargets.filter((id) => id !== noobId),
    });
  }
  private disconnect(noobId: string) {
    this.generations.set(noobId, (this.generations.get(noobId) ?? 0) + 1);
    this.transfers.disconnect(noobId);
    this.publish({
      peers: this.snapshot.peers.map((p) =>
        p.noobId === noobId ? { ...p, online: false, accepting: false } : p,
      ),
    });
    for (const activity of this.snapshot.activities) {
      const target = activity.targets?.find((d) => d.noobId === noobId && deliveryPending(d.state));
      if (target)
        this.delivery(activity.id, noobId, target.state === 'queued' ? 'cancelled' : 'unconfirmed');
    }
  }
  // The following controls simulate external events without touching real devices.
  async sampleCopy() {
    this.sample = (this.sample + 1) % examples.length;
    await this.copy(examples[this.sample]);
  }
  async sampleReceive(noobId = this.snapshot.peers.find((p) => p.online)?.noobId) {
    const peer = this.snapshot.peers.find((p) => p.noobId === noobId);
    const { settings } = this.snapshot;
    if (!peer?.online || settings.paused || !settings.receive) throw fail('receivingUnavailable');
    const item: TextItem = {
      id: ++this.sequence,
      text: sampleNote(peer.deviceName),
      source: 'remote',
      sourceNoobId: peer.noobId,
      copiedAt: Date.now(),
    };
    this.publish({
      current: item,
      activities: this.withActivity({
        id: Number(item.id),
        sourceNoobId: peer.noobId,
        sourceName: peer.deviceName,
        kind: 'received',
        state: 'applied',
        title: activitySummary(item.text),
        at: item.copiedAt,
      }),
    });
    this.remember(item);
    this.emit({ type: 'received', sequence: Number(item.id) });
  }
  async toggleConnection(noobId = this.snapshot.peers[0]?.noobId) {
    const peer = this.snapshot.peers.find((p) => p.noobId === noobId);
    if (!peer) throw fail('noSampleDevice');
    if (peer.online) this.disconnect(peer.noobId);
    else
      this.publish({
        peers: this.snapshot.peers.map((p) =>
          p.noobId === peer.noobId ? { ...p, online: true, accepting: true } : p,
        ),
      });
  }
  async toggleAccepting(noobId: string) {
    this.publish({
      peers: this.snapshot.peers.map((p) =>
        p.noobId === noobId ? { ...p, accepting: !p.accepting } : p,
      ),
    });
    if (!this.snapshot.peers.find((p) => p.noobId === noobId)?.accepting)
      this.transfers.cancelQueued(noobId, false);
  }
  async reset() {
    this.onboarding.dispose();
    this.transfers.dispose();
    this.timers.forEach(clearTimeout);
    this.timers.clear();
    this.generations.clear();
    this.sample = 0;
    this.sequence = 100;
    this.publish({ ...createSeed(), onboarding: undefined });
    this.emit({ type: 'reset' });
  }
  dispose() {
    this.onboarding.dispose();
    this.transfers.dispose();
    this.timers.forEach(clearTimeout);
    this.timers.clear();
    this.listeners.clear();
    this.events.clear();
  }
}
