import { Channel, invoke } from '@tauri-apps/api/core';
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
  dispose() {}
}
