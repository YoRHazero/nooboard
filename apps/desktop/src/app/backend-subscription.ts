import { publishTerminalEnded } from '../shared/events';
import { QueryClient } from '@tanstack/react-query';
import { TerminalStreams, installTerminalStreams } from '../features/terminal/io/output-stream';
import { terminalStore } from '../features/terminal/store';
import { ProblemError, toProblem, fail, type Problem } from '../shared/i18n/errors';
import type { BackendTransport, Session } from '../shared/backend-contract';
import type { BackendSnapshot } from '../shared/contracts.generated';
import { SnapshotProjection } from './snapshot-projection';
import {
  PreferenceController,
  installPreferences,
  type PreferenceStorage,
} from '../features/settings/preferences';
import { usePreferenceStore } from '../features/settings/store';
import type { ConnectionState, HostState } from '../shared/types';
import { useApplicationStore } from '../shared/store';
import { installBackend, advanceBackendGeneration, backendRequest } from '../shared/backend';
import { publishBackendEvent } from '../shared/events';
import { createQueryClient } from './query-client';

/** Owns a WebView subscription, not Core or a second business-state store. */
export class BackendSubscription {
  diagnostic = false;
  private disposed = false;
  private terminal: TerminalStreams;
  private generation = 0;
  private session: Session | null = null;
  private abort: AbortController | null = null;
  private get state() {
    return useApplicationStore.getState().connection;
  }
  private host: HostState | undefined;
  readonly projection: SnapshotProjection;
  private preferences: PreferenceController;
  private cleanup: (() => void) | undefined;
  private initialization: Promise<void> | undefined;
  private starting: Promise<void> | undefined;
  constructor(
    private bridge: BackendTransport,
    readonly queryClient: QueryClient = createQueryClient(),
    storage?: PreferenceStorage,
  ) {
    useApplicationStore.setState({
      connection: { phase: 'idle', error: null, hasSnapshot: false },
      error: null,
    });
    this.preferences = new PreferenceController(
      bridge,
      storage,
      (host) => this.acceptHost(host),
      usePreferenceStore,
    );
    installPreferences(this.preferences);
    this.projection = new SnapshotProjection(queryClient);
    this.projection.onEvent(publishBackendEvent);
    this.terminal = new TerminalStreams(
      bridge.terminal,
      () => !this.disposed && this.state.phase === 'ready',
      () => Number(this.projection.getSnapshot().terminal.options.replayBytes),
      terminalStore,
      () =>
        this.projection.hasSnapshot
          ? this.projection.getSnapshot().terminal
          : { runs: [], dependencies: [] },
    );
    installTerminalStreams(this.terminal);
    this.terminal.reset();
    installBackend(bridge, queryClient, () => this.connect());
  }
  private ensureOpen() {
    if (this.disposed) throw fail('stopped');
  }
  private connection(phase: ConnectionState['phase'], error: Problem | null = null) {
    useApplicationStore
      .getState()
      .setConnection({ phase, error, hasSnapshot: this.projection.hasSnapshot });
  }
  private acceptHost(host: HostState) {
    if (this.disposed || (this.host && host.revision <= this.host.revision)) return;
    this.host = host;
    this.projection.setHost(host);
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
  async connect() {
    this.ensureOpen();
    const generation = ++this.generation;
    advanceBackendGeneration();
    void this.queryClient.cancelQueries({ predicate: (query) => query.queryKey[0] !== 'backend' });
    this.terminal.reset();
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
        if (frame.type === 'terminalEnded') {
          this.terminal.ended(frame.data);
          publishTerminalEnded(frame.data);
          return;
        }
        if (frame.type === 'terminalLagged') {
          this.terminal.lagged();
          return;
        }
        if (frame.type === 'host') {
          this.acceptHost(frame.data);
          return;
        }
        if (frame.type === 'stopped') {
          stopped = toProblem(frame.data);
          if (connected) {
            this.connection('offline', stopped);
          }
          return;
        }
        if (stopped) return;
        if (connected && this.host) {
          const previousSession = this.projection.getSnapshot().session;
          if (this.projection.apply(frame.data, this.host, frame.type === 'recovered')) {
            if (previousSession !== frame.data.session) this.terminal.reset();
            this.terminal.reconcile(frame.data.terminal);
          }
        } else if (
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
      this.projection.apply(session.initial.snapshot, this.host!, true);
      if (pending) this.projection.apply(pending, this.host!, true);
      this.terminal.reconcile(this.projection.getSnapshot().terminal);
      connected = true;
      if (stopped) throw new ProblemError(stopped);
      this.connection('ready');
      void this.queryClient.invalidateQueries({
        predicate: (query) => query.queryKey[0] !== 'backend',
      });
    } catch (error) {
      if (this.disposed || generation !== this.generation) return;
      const problem = toProblem(error);
      this.connection('offline', problem);
      throw new ProblemError(problem);
    }
  }
  async probe(action: 'copy' | 'receive') {
    await backendRequest({ type: 'probe', data: { action } }, 'done');
  }
  async dispose() {
    if (this.disposed) return;
    this.disposed = true;
    this.terminal.dispose();
    this.generation++;
    advanceBackendGeneration();
    this.abort?.abort();
    this.preferences.dispose();
    this.cleanup?.();
    this.connection('disposed');
    this.projection.dispose();
    await this.queryClient.cancelQueries();
    this.queryClient.clear();
    this.bridge.dispose();
    const session = this.session;
    this.session = null;
    await session?.close();
  }
}
