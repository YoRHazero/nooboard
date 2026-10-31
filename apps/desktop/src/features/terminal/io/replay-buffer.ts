import type { TerminalOutputFrame } from '../../../shared/types';
export type OutputListener = (frame: TerminalOutputFrame) => void | Promise<void>;
/** A bounded, transient renderer replay window. Ended runs are not kept as history. */
export class TerminalBuffer {
  private chunks: Uint8Array[] = [];
  private length = 0;
  private offset = 0n;
  private end = 0n;
  private listeners = new Set<OutputListener>();
  closed = false;
  constructor(
    readonly subscription: string,
    readonly abort: AbortController,
    private limit: () => number,
  ) {}
  observe(listener: OutputListener) {
    this.listeners.add(listener);
    const bytes = new Uint8Array(this.length);
    let cursor = 0;
    for (const chunk of this.chunks) {
      bytes.set(chunk, cursor);
      cursor += chunk.length;
    }
    return {
      bytes,
      offset: this.offset,
      closed: this.closed,
      unsubscribe: () => {
        this.listeners.delete(listener);
      },
    };
  }
  async accept(frame: TerminalOutputFrame) {
    if (this.abort.signal.aborted) return;
    if (frame.type === 'gap') {
      this.chunks = [];
      this.length = 0;
      this.offset = this.end = BigInt(frame.data.offset);
    }
    if (frame.type === 'data') {
      const offset = BigInt(frame.data.offset);
      if (offset !== this.end) {
        this.chunks = [];
        this.length = 0;
        this.offset = offset;
      }
      const bytes = new Uint8Array(frame.data.bytes);
      this.chunks.push(bytes);
      this.length += bytes.length;
      this.end = offset + BigInt(bytes.length);
      const limit = this.limit();
      while (this.length > limit && this.chunks.length) {
        const first = this.chunks[0];
        const removed = Math.min(first.length, this.length - limit);
        if (removed === first.length) this.chunks.shift();
        else this.chunks[0] = first.slice(removed);
        this.length -= removed;
        this.offset += BigInt(removed);
      }
    }
    if (frame.type === 'closed') this.closed = true;
    // Native delivery waits for this callback: a mounted terminal renderer resolves
    // after parsing the bytes; inactive views retain only the bounded replay window.
    await Promise.all(
      [...this.listeners].map((listener) => Promise.resolve(listener(frame)).catch(() => {})),
    );
  }
  dispose() {
    this.abort.abort();
    this.listeners.clear();
    this.chunks = [];
    this.length = 0;
    this.closed = true;
  }
}
