import { type Problem, toProblem, ProblemError } from '../i18n/errors';
import { Channel, invoke } from '@tauri-apps/api/core';
import type {
  DesktopClient,
  DesktopEvent,
  DesktopState,
  EntryId,
  HistoryQuery,
  HistoryPage,
  LocalDevicePatch,
  PeerSettings,
  Settings,
} from '../api/contracts';
import { NativeStore } from './NativeStore';
import { loadAppearance, saveAppearance } from './appearance';
import type { NativeConnection, NativeFrame, NativeHistoryPage, NativeSnapshot } from './wire';

export interface NativeTransport {
  connect(receive: (frame: NativeFrame) => void): Promise<NativeConnection>;
  invoke<T>(command: string, args?: Record<string, unknown>): Promise<T>;
}
const transport: NativeTransport = {
  invoke,
  connect(receive) {
    const channel = new Channel<NativeFrame>();
    channel.onmessage = receive;
    return invoke('desktop_connect', { onFrame: channel });
  },
};
/** Native transport and commands. No simulation or clipboard fallback is used here. */
export class NativeClient implements DesktopClient {
  readonly mode = 'native' as const;
  readonly store: NativeStore;
  diagnostic = false;
  private generation = 0;
  constructor(
    private bridge: NativeTransport = transport,
    appearance = loadAppearance(),
  ) {
    this.store = new NativeStore(appearance);
  }
  getSnapshot = () => this.store.getSnapshot();
  subscribe = (listener: () => void) => this.store.subscribe(listener);
  onEvent = (listener: (event: DesktopEvent) => void) => this.store.onEvent(listener);
  async connect() {
    const generation = ++this.generation;
    let pending: NativeSnapshot | null = null;
    let pendingDesktop: DesktopState | undefined;
    let connected = false;
    let stopped: Problem | undefined;
    const result = await this.bridge.connect((frame) => {
      if (generation !== this.generation) return;
      if (frame.type === 'desktop') {
        if (connected) this.store.setDesktop(frame.data);
        else if (!pendingDesktop || frame.data.revision > pendingDesktop.revision)
          pendingDesktop = frame.data;
        return;
      }
      if (frame.type === 'stopped') {
        stopped = frame.data;
        if (connected) this.store.stopped(frame.data);
        return;
      }
      if (connected) this.store.apply(frame.data, frame.type === 'recovered');
      else if (
        !pending ||
        pending.session !== frame.data.session ||
        BigInt(frame.data.revision) > BigInt(pending.revision)
      )
        pending = frame.data;
    });
    if (generation !== this.generation) return;
    this.diagnostic = result.diagnostic;
    if (result.desktop) this.store.setDesktop(result.desktop);
    if (pendingDesktop) this.store.setDesktop(pendingDesktop);
    this.store.apply(result.snapshot, true);
    if (pending) this.store.apply(pending, true);
    connected = true;
    if (stopped) {
      this.store.stopped(stopped);
      throw new ProblemError(stopped);
    }
    this.store.connected();
  }
  private async command<T>(command: string, args?: Record<string, unknown>): Promise<T> {
    try {
      return await this.bridge.invoke<T>(command, args);
    } catch (reason) {
      throw new ProblemError(toProblem(reason));
    }
  }
  async discover() {
    await this.command('desktop_discover');
  }
  async beginPairing(address: string, expected?: string) {
    await this.command('desktop_begin_pairing', { address, expected: expected ?? null });
  }
  async acceptPairing(id: string) {
    await this.command('desktop_accept_pairing', { id });
  }
  async submitPairingCode(id: string, code: string) {
    await this.command('desktop_pairing_code', { id, code });
  }
  async dismissPairing(id: string) {
    await this.command('desktop_dismiss_pairing', { id });
  }
  async sendCurrent() {
    await this.command('desktop_send');
  }
  async selectFiles() {
    await this.command('desktop_select_files');
  }
  async selectReceiveDirectory() {
    await this.command('desktop_receive_directory');
  }
  async cancelTransfer(key: string) {
    await this.command('desktop_transfer_action', { key, action: 'cancel' });
  }
  async copyReceived(key: string) {
    await this.command('desktop_transfer_action', { key, action: 'copy' });
  }
  async selectTargets(targets: string[]) {
    await this.command('desktop_select_targets', { targets });
  }
  async configurePeer(noobId: string, patch: Partial<PeerSettings>) {
    await this.command('desktop_peer_settings', {
      noobId,
      patch: {
        address: patch.address ?? null,
        autoSend: patch.autoSend ?? null,
        hasAddress: Object.hasOwn(patch, 'address'),
      },
    });
  }
  async updateLocalDevice(patch: LocalDevicePatch) {
    await this.command('desktop_settings', { patch });
  }
  async updateSettings(settings: Partial<Settings>) {
    const { theme, reducedMotion, closeToTray, ...business } = settings;
    if (closeToTray !== undefined) {
      const desktop = await this.command<DesktopState>('desktop_preferences', {
        patch: { closeToTray },
      });
      this.store.setDesktop(desktop);
    }
    if (Object.keys(business).length)
      await this.command('desktop_settings', {
        patch: {
          ...business,
          mode: business.mode
            ? business.mode === 'automatic'
              ? 'Automatic'
              : 'Manual'
            : undefined,
        },
      });
    if (theme !== undefined || reducedMotion !== undefined) {
      const current = this.getSnapshot().settings;
      const appearance = {
        theme: theme ?? current.theme,
        reducedMotion: reducedMotion ?? current.reducedMotion,
      };
      saveAppearance(appearance);
      this.store.setAppearance(appearance);
    }
  }
  async unpair(noobId: string) {
    await this.command('desktop_unpair', { noobId });
  }
  async acknowledgeNavigation(id: number) {
    await this.command('desktop_navigation_ack', { id });
  }
  async queryHistory(query: HistoryQuery): Promise<HistoryPage> {
    const page = await this.command<NativeHistoryPage>('desktop_history', { ...query });
    return {
      items: page.items.map((row) => ({
        id: row.id,
        text: row.text,
        source: row.source === 'local' ? 'local' : 'remote',
        sourceNoobId: row.source === 'local' ? undefined : row.source,
        copiedAt: row.copied_at_ms,
      })),
      hasMore: page.has_more,
    };
  }
  async copyHistory(id: EntryId) {
    await this.command('desktop_history_action', { action: 'copy', id: String(id) });
  }
  async deleteHistory(id: EntryId) {
    await this.command('desktop_history_action', { action: 'delete', id: String(id) });
  }
  async clearHistory() {
    await this.command('desktop_history_action', { action: 'clear', id: null });
  }
  async probe(action: 'copy' | 'receive') {
    await this.command('desktop_probe', { action });
  }
  dispose() {
    this.generation++;
  }
}
