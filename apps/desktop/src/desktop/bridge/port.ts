import type { Connection, Frame, Reply, Request } from './generated';
export interface Session {
  initial: Connection;
  close(): Promise<void>;
}
export interface DesktopBridge {
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
