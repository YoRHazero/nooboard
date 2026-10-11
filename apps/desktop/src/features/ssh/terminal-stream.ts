/** Byte offsets avoid corrupting UTF-8 / escape sequences across IPC chunks. */
export class TerminalStream {
  offset = 0n;
  constructor(
    private write: (bytes: Uint8Array) => void,
    private gap: () => void,
  ) {}
  accept(offset: string, bytes: number[]): boolean {
    const start = BigInt(offset);
    if (start > this.offset) return false;
    const skip = Number(this.offset - start);
    if (skip < bytes.length) {
      this.write(Uint8Array.from(bytes.slice(skip)));
      this.offset = start + BigInt(bytes.length);
    }
    return true;
  }
  replay(replay: { start: string; end: string; gap: boolean; bytes: number[] }) {
    if (replay.gap && BigInt(replay.start) > this.offset) {
      this.gap();
      this.offset = BigInt(replay.start);
    }
    this.accept(replay.start, replay.bytes);
  }
}
