import type { TerminalTransport } from './port';
import type { TerminalAction, TerminalOutputFrame } from './generated';
import { Channel, invoke } from '@tauri-apps/api/core';
import { getCurrentWebview } from '@tauri-apps/api/webview';
import { getCurrentWindow } from '@tauri-apps/api/window';
import type { FileDrop } from './port';
import { locale } from '@tauri-apps/plugin-os';
import type { DesktopBridge, Session } from './port';
import type { Connection, Frame, Request, Reply } from './generated';
export class TauriBridge implements DesktopBridge {
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
    return (await import('./menu')).initializeMenu();
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
