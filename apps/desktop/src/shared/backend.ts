import type { TerminalTransport } from './backend-contract';
import type { TerminalAction, TerminalOutputFrame } from './contracts.generated';
import { Channel, invoke } from '@tauri-apps/api/core';
import { getCurrentWebview } from '@tauri-apps/api/webview';
import { getCurrentWindow } from '@tauri-apps/api/window';
import type { FileDrop } from './backend-contract';
import { locale } from '@tauri-apps/plugin-os';
import type { BackendTransport, Session } from './backend-contract';
import type { Connection, Frame, Request, Reply } from './contracts.generated';
export class NativeBackend implements BackendTransport {
  readonly mode = 'native' as const;
  private owner: string | null = null;
  readonly terminal: TerminalTransport = {
    request: (action) => this.terminalRequest(this.owner ?? '', action),
    open: async (action, receive, signal) => {
      const owner = this.owner ?? '';
      const subscription = action.data.subscription;
      const detach = () => {
        void this.terminalRequest(owner, { type: 'detach', data: { subscription } }).catch(
          () => {},
        );
      };
      signal.addEventListener('abort', detach, { once: true });
      const channel = new Channel<TerminalOutputFrame>();
      channel.onmessage = (frame) => {
        if (signal.aborted) return;
        void Promise.resolve(receive(frame))
          .then(async () => {
            if (!signal.aborted && (frame.type === 'data' || frame.type === 'gap'))
              await this.terminalRequest(owner, {
                type: 'ack',
                data: { subscription, sequence: frame.data.sequence },
              });
          })
          .catch(detach);
      };
      try {
        const reply = await invoke<Reply>('request', {
          request: { type: 'terminal', data: { owner, action } } satisfies Request,
          onOutput: channel,
        });
        if (signal.aborted) detach();
        return reply;
      } catch (error) {
        signal.removeEventListener('abort', detach);
        detach();
        throw error;
      }
    },
  };
  private terminalRequest(owner: string, action: TerminalAction) {
    return invoke<Reply>('request', {
      request: { type: 'terminal', data: { owner, action } } satisfies Request,
    });
  }
  async connect(receive: (frame: Frame) => void, signal: AbortSignal): Promise<Session> {
    const subscription = crypto.randomUUID();
    const channel = new Channel<Frame>();
    channel.onmessage = (frame) => {
      if (!signal.aborted) receive(frame);
    };
    let closed = false;
    const close = async () => {
      if (closed) return;
      closed = true;
      if (this.owner === subscription) this.owner = null;
      await invoke('disconnect', { subscription });
    };
    const initial = await invoke<Connection>('connect', { subscription, onFrame: channel });
    if (signal.aborted) await close();
    else this.owner = subscription;
    return { initial, close };
  }
  request(request: Request) {
    return invoke<Reply>('request', { request });
  }
  locale() {
    return locale();
  }
  async setup() {
    return (await import('./native-menu')).initializeMenu();
  }
  async onFileDrop(listener: (event: FileDrop) => void) {
    let paths: string[] = [];
    const scale = await getCurrentWindow().scaleFactor();
    return getCurrentWebview().onDragDropEvent(({ payload }) => {
      if (payload.type === 'leave') {
        listener({ type: 'leave', paths: [], x: 0, y: 0 });
        paths = [];
        return;
      }
      if (payload.type === 'enter' || payload.type === 'drop') paths = payload.paths;
      listener({
        type: payload.type === 'drop' ? 'drop' : 'over',
        paths,
        x: payload.position.x / scale,
        y: payload.position.y / scale,
      });
    });
  }
  dispose() {}
}

// Installed once by application bootstrap. Queries call this boundary directly.
import type { QueryClient } from '@tanstack/react-query';
import type { DesktopSnapshot } from './types';
import { backendSnapshotKey } from './queries/useBackendSnapshotQuery';
import { fail, ProblemError, toProblem } from './i18n/errors';
import { useApplicationStore } from './store';
let installed:
  | { transport: BackendTransport; queryClient: QueryClient; reconnect: () => Promise<void> }
  | undefined;
let generation = 0;
export function installBackend(
  transport: BackendTransport,
  queryClient: QueryClient,
  reconnect: () => Promise<void>,
) {
  generation++;
  installed = { transport, queryClient, reconnect };
}
export function advanceBackendGeneration() {
  return ++generation;
}
export function getBackend() {
  if (!installed) throw fail('stopped');
  return installed.transport;
}
export function getBackendQueryClient() {
  if (!installed) throw fail('stopped');
  return installed.queryClient;
}
export function getBackendSnapshot() {
  const snapshot = getBackendQueryClient().getQueryData<DesktopSnapshot>(backendSnapshotKey);
  if (!snapshot) throw fail('backendNotConnected');
  return snapshot;
}
export function subscribeBackendSnapshot(listener: () => void) {
  return getBackendQueryClient()
    .getQueryCache()
    .subscribe((event) => {
      if (
        event.type === 'updated' &&
        event.query.queryKey[0] === 'backend' &&
        event.query.queryKey[1] === 'snapshot'
      )
        listener();
    });
}
export function reconnectBackend() {
  if (!installed) return Promise.reject(fail('stopped'));
  return installed.reconnect();
}
export async function backendRequest<T extends Reply['type']>(
  request: Request,
  expected: T,
): Promise<Extract<Reply, { type: T }>> {
  const state = useApplicationStore.getState().connection;
  if (state.phase === 'disposed') throw fail('stopped');
  if (state.phase !== 'ready') throw fail('backendNotConnected');
  const epoch = generation;
  const session = getBackendSnapshot().session;
  try {
    const reply = await getBackend().request(request);
    if (epoch !== generation || getBackendSnapshot().session !== session) throw fail('stopped');
    if (reply.type !== expected) throw fail('generic');
    return reply as Extract<Reply, { type: T }>;
  } catch (error) {
    throw new ProblemError(toProblem(error));
  }
}
export async function terminalRequest(action: TerminalAction): Promise<Reply> {
  const state = useApplicationStore.getState().connection;
  if (state.phase === 'disposed') throw fail('stopped');
  if (state.phase !== 'ready') throw fail('backendNotConnected');
  const transport = getBackend().terminal;
  if (!transport) throw fail('terminalUnavailable');
  const epoch = generation;
  const session = getBackendSnapshot().session;
  try {
    const reply = await transport.request(action);
    if (epoch !== generation || getBackendSnapshot().session !== session) throw fail('stopped');
    return reply;
  } catch (error) {
    throw new ProblemError(toProblem(error));
  }
}
