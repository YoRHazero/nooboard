import type { TerminalAction, TerminalOutputFrame } from './generated';
import type { Connection, Frame, Reply, Request } from './generated';
export interface Session {
  initial: Connection;
  close(): Promise<void>;
}
export interface TerminalTransport {
  request(action: TerminalAction): Promise<Reply>;
  open(
    action: Extract<TerminalAction, { type: 'start' | 'attach' | 'startLocal' }>,
    receive: (frame: TerminalOutputFrame) => void | Promise<void>,
    signal: AbortSignal,
  ): Promise<Reply>;
}
export interface DesktopBridge {
  readonly terminal?: TerminalTransport;
  onFileDrop?(listener: (event: FileDrop) => void): Promise<() => void>;
  readonly mode: 'native' | 'preview';
  connect(receive: (frame: Frame) => void, signal: AbortSignal): Promise<Session>;
  request(request: Request): Promise<Reply>;
  locale(): Promise<string | null>;
  setup(): Promise<() => void>;
  dispose(): void;
}

export interface FileDrop {
  type: 'enter' | 'over' | 'drop' | 'leave';
  paths: string[];
  x: number;
  y: number;
}
