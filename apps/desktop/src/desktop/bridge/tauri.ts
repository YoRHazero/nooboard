import { Channel, invoke } from '@tauri-apps/api/core';
import { getCurrentWebview } from '@tauri-apps/api/webview';
import { getCurrentWindow } from '@tauri-apps/api/window';
import type { FileDrop } from './port';
import { locale } from '@tauri-apps/plugin-os';
import type { DesktopBridge, Session } from './port';
import type { Connection, Frame, Request, Reply } from './generated';
export class TauriBridge implements DesktopBridge {
  readonly mode = 'native' as const;
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
      await invoke('disconnect', { subscription });
    };
    const initial = await invoke<Connection>('connect', { subscription, onFrame: channel });
    if (signal.aborted) await close();
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
