import type { Desktop, DesktopRuntime } from './api';
import { ProblemError, fail, toProblem, type Problem } from '../i18n/errors';
import type { DesktopBridge, Session } from './bridge/port';
import type { Request, Reply, BackendSnapshot } from './bridge/generated';
import { SnapshotStore } from './snapshot/store';
import { PreferenceRuntime, type PreferenceStorage } from './preferences/runtime';
import type { ConnectionState, HostState } from './snapshot/model';

export function createRuntime(
  bridge: DesktopBridge,
  storage?: PreferenceStorage,
): { runtime: DesktopRuntime; desktop: Desktop } {
  const owner = new Runtime(bridge, storage);
  return { runtime: owner, desktop: owner.desktop };
}
class Runtime implements DesktopRuntime {
  readonly desktop: Desktop;
  diagnostic = false;
  private disposed = false;
  private generation = 0;
  private session: Session | null = null;
  private abort: AbortController | null = null;
  private state: ConnectionState = { phase: 'idle', error: null, hasSnapshot: false };
  private listeners = new Set<() => void>();
  private host: HostState | undefined;
  private store: SnapshotStore;
  private preferences: PreferenceRuntime;
  private cleanup: (() => void) | undefined;
  private preferenceSubscription: () => void;
  private initialization: Promise<void> | undefined;
  private starting: Promise<void> | undefined;
  constructor(
    private bridge: DesktopBridge,
    storage?: PreferenceStorage,
  ) {
    this.preferences = new PreferenceRuntime(bridge, storage, (host) => this.acceptHost(host));
    this.store = new SnapshotStore(this.preferences.getSnapshot().appearance);
    this.preferenceSubscription = this.preferences.subscribe(() =>
      this.store.setAppearance(this.preferences.getSnapshot().appearance),
    );
    const done = async (request: Request) => {
      await this.command(request, 'done');
    };
    this.desktop = Object.freeze({
      mode: bridge.mode,
      getSnapshot: this.store.getSnapshot,
      subscribe: this.store.subscribe,
      onEvent: this.store.onEvent,
      getConnection: () => this.state,
      subscribeConnection: (listener: () => void) => {
        this.listeners.add(listener);
        return () => {
          this.listeners.delete(listener);
        };
      },
      getPreferences: this.preferences.getSnapshot,
      subscribePreferences: this.preferences.subscribe,
      reconnect: () => this.connect(),
      discover: () => done({ type: 'discover' }),
      beginPairing: (address: string, expected?: string) =>
        done({ type: 'beginPairing', data: { address, expected } }),
      acceptPairing: (id: string) => done({ type: 'acceptPairing', data: { id } }),
      submitPairingCode: (id: string, code: string) =>
        done({ type: 'pairingCode', data: { id, code } }),
      dismissPairing: (id: string) => done({ type: 'dismissPairing', data: { id } }),
      queryHistory: async (query) => {
        const { data } = await this.command({ type: 'queryHistory', data: { query } }, 'history');
        return {
          hasMore: data.hasMore,
          items: data.items.map((row) => ({
            ...row,
            source: row.source === 'local' ? ('local' as const) : ('remote' as const),
            sourceNoobId: row.source === 'local' ? undefined : row.source,
          })),
        };
      },
      sendCurrent: async () => (await this.command({ type: 'sendCurrent' }, 'sent')).data,
      selectFiles: () => done({ type: 'selectFiles' }),
      selectReceiveDirectory: () => done({ type: 'selectReceiveDirectory' }),
      cancelTransfer: (key: string) => done({ type: 'cancelTransfer', data: { key } }),
      copyReceived: (key: string) => done({ type: 'copyReceived', data: { key } }),
      selectTargets: (targets: string[]) => done({ type: 'selectTargets', data: { targets } }),
      configurePeer: (noobId, patch) =>
        done({
          type: 'configurePeer',
          data: {
            noobId,
            patch: {
              autoSend: patch.autoSend,
              address: Object.hasOwn(patch, 'address')
                ? patch.address
                  ? { type: 'set', data: patch.address }
                  : { type: 'clear' }
                : undefined,
            },
          },
        }),
      updateLocalDevice: async (patch) =>
        (await this.command({ type: 'updateLocalDevice', data: { patch } }, 'saved')).data.revision,
      updateSyncSettings: async (patch) =>
        (await this.command({ type: 'updateSyncSettings', data: { patch } }, 'saved')).data
          .revision,
      updateHostPreferences: async (patch) => {
        this.ensureOpen();
        await this.preferences.updateHost(patch);
      },
      updateAppearance: async (patch) => {
        this.ensureOpen();
        this.preferences.setAppearance(patch);
      },
      setLanguage: (language) => {
        this.ensureOpen();
        return this.preferences.setLanguage(language);
      },
      copyHistory: (id: string) => done({ type: 'copyHistory', data: { id } }),
      deleteHistory: (id: string) => done({ type: 'deleteHistory', data: { id } }),
      clearHistory: () => done({ type: 'clearHistory' }),
      unpair: (noobId: string) => done({ type: 'unpair', data: { noobId } }),
      acknowledgeNavigation: (id: number) => done({ type: 'acknowledgeNavigation', data: { id } }),
    } satisfies Desktop);
  }
  private ensureOpen() {
    if (this.disposed) throw fail('stopped');
  }
  private connection(phase: ConnectionState['phase'], error: Problem | null = null) {
    this.state = { phase, error, hasSnapshot: this.store.hasSnapshot };
    this.listeners.forEach((fn) => fn());
  }
  private acceptHost(host: HostState) {
    if (this.disposed || (this.host && host.revision <= this.host.revision)) return;
    this.host = host;
    this.store.setHost(host);
  }
  async start() {
    this.ensureOpen();
    this.starting ??= this.startOnce();
    await this.starting;
  }
  private async startOnce() {
    this.initialization ??= this.initialize();
    await this.initialization;
    if (!this.disposed) await this.connect();
  }
  private async initialize() {
    void this.bridge
      .setup()
      .then((cleanup) => {
        if (this.disposed) cleanup();
        else this.cleanup = cleanup;
      })
      .catch((error: unknown) => console.error('Desktop menu setup failed', error));
    await this.preferences.start();
  }
  private async command<T extends Reply['type']>(
    request: Request,
    expected: T,
  ): Promise<Extract<Reply, { type: T }>> {
    this.ensureOpen();
    if (this.state.phase !== 'ready') throw fail('backendNotConnected');
    const generation = this.generation;
    const session = this.store.getSnapshot().session;
    try {
      const reply = await this.bridge.request(request);
      if (
        this.disposed ||
        generation !== this.generation ||
        this.store.getSnapshot().session !== session
      )
        throw fail('stopped');
      if (reply.type !== expected) throw fail('generic');
      return reply as Extract<Reply, { type: T }>;
    } catch (error) {
      throw new ProblemError(toProblem(error));
    }
  }
  async connect() {
    this.ensureOpen();
    const generation = ++this.generation;
    this.abort?.abort();
    const abort = new AbortController();
    this.abort = abort;
    const previous = this.session;
    this.session = null;
    this.connection('connecting');
    let connected = false;
    let pending: BackendSnapshot | null = null;
    let stopped: Problem | null = null;
    try {
      await previous?.close();
      if (this.disposed || generation !== this.generation) return;
      const session = await this.bridge.connect((frame) => {
        if (this.disposed || generation !== this.generation) return;
        if (frame.type === 'host') {
          this.acceptHost(frame.data);
          return;
        }
        if (frame.type === 'stopped') {
          stopped = toProblem(frame.data);
          if (connected) {
            this.store.setError(stopped);
            this.connection('offline', stopped);
          }
          return;
        }
        if (stopped) return;
        if (connected && this.host)
          this.store.apply(frame.data, this.host, frame.type === 'recovered');
        else if (
          !pending ||
          pending.session !== frame.data.session ||
          BigInt(frame.data.revision) > BigInt(pending.revision)
        )
          pending = frame.data;
      }, abort.signal);
      if (this.disposed || generation !== this.generation) {
        await session.close();
        return;
      }
      this.session = session;
      this.diagnostic = session.initial.diagnostic;
      this.acceptHost(session.initial.host);
      this.store.apply(session.initial.snapshot, this.host!, true);
      if (pending) this.store.apply(pending, this.host!, true);
      connected = true;
      if (stopped) throw new ProblemError(stopped);
      this.store.setError(undefined);
      this.connection('ready');
    } catch (error) {
      if (this.disposed || generation !== this.generation) return;
      const problem = toProblem(error);
      this.store.setError(problem);
      this.connection('offline', problem);
      throw new ProblemError(problem);
    }
  }
  async probe(action: 'copy' | 'receive') {
    await this.command({ type: 'probe', data: { action } }, 'done');
  }
  async dispose() {
    if (this.disposed) return;
    this.disposed = true;
    this.generation++;
    this.abort?.abort();
    this.preferences.dispose();
    this.preferenceSubscription();
    this.cleanup?.();
    this.connection('disposed');
    this.listeners.clear();
    this.store.dispose();
    this.bridge.dispose();
    const session = this.session;
    this.session = null;
    await session?.close();
  }
}
