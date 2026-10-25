import { fail, ProblemError, toProblem } from '../shared/i18n/errors';
import type { TerminalTransport } from './bridge/port';
import type {
  Reply,
  TerminalAction,
  TerminalEnded,
  TerminalOutputFrame,
  TerminalReuse,
  TerminalRunView,
  TerminalTarget,
  TerminalWorkspaceView,
} from './bridge/generated';
type OpenRequest =
  | { type: 'start'; target: TerminalTarget; reuse: TerminalReuse }
  | { type: 'attach'; target: TerminalTarget }
  | { type: 'startLocal'; name: string };
export type OutputListener = (frame: TerminalOutputFrame) => void | Promise<void>;
export const terminalKey = (target: TerminalTarget) => `${target.kind}:${target.id}`;
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
export interface TerminalClientState {
  selected: TerminalTarget | null;
  collapsed: readonly string[];
  pending: readonly { subscription: string; target: TerminalTarget | null; name?: string }[];
  notice: TerminalEnded | 'lagged' | null;
}
export class TerminalClient {
  private state: TerminalClientState = { selected: null, collapsed: [], pending: [], notice: null };
  private listeners = new Set<() => void>();
  private buffers = new Map<string, TerminalBuffer>();
  private pending = new Map<string, TerminalBuffer>();
  private endedWhileOpening = new Set<string>();
  private knownRuns = new Map<string, TerminalRunView>();
  private dependencies: TerminalWorkspaceView['dependencies'] = [];
  private autoCollapsed = new Set<string>();
  private panelIntent = 0;
  private generation = 0;
  private attaching = new Map<string, Promise<TerminalTarget | null>>();
  constructor(
    private transport: TerminalTransport | undefined,
    private online: () => boolean,
    private limit: () => number,
  ) {}
  getSnapshot = () => this.state;
  subscribe = (listener: () => void) => {
    this.listeners.add(listener);
    return () => {
      this.listeners.delete(listener);
    };
  };
  private publish(patch: Partial<TerminalClientState>) {
    this.state = { ...this.state, ...patch };
    this.listeners.forEach((listener) => listener());
  }
  async request(action: TerminalAction): Promise<Reply> {
    if (!this.online()) throw fail('backendNotConnected');
    if (!this.transport) throw fail('terminalUnavailable');
    const generation = this.generation;
    try {
      const reply = await this.transport.request(action);
      if (generation !== this.generation) throw fail('stopped');
      return reply;
    } catch (error) {
      throw new ProblemError(toProblem(error));
    }
  }
  buffer(target: TerminalTarget) {
    return this.buffers.get(terminalKey(target));
  }
  select(target: TerminalTarget | null) {
    this.panelIntent++;
    this.show(target);
  }
  private show(target: TerminalTarget | null) {
    this.publish({
      selected: target,
      collapsed: target
        ? this.state.collapsed.filter((key) => key !== terminalKey(target))
        : this.state.collapsed,
    });
  }
  collapse(target: TerminalTarget) {
    if (target.kind !== 'command' && target.kind !== 'tunnel') return;
    const key = terminalKey(target);
    if (!this.knownRuns.has(key)) return;
    this.panelIntent++;
    this.updatePanels(new Set([...this.state.collapsed, key]));
  }
  private updatePanels(collapsed: Set<string>, selected = this.state.selected) {
    for (const key of collapsed) {
      // Closing an output subscription never stops its command or tunnel.
      this.buffers.get(key)?.dispose();
      this.buffers.delete(key);
    }
    if (selected && collapsed.has(terminalKey(selected))) {
      const selectedId = selected.id;
      const owner = this.dependencies.find((d) => d.runId === selectedId)?.owner;
      selected =
        owner && this.knownRuns.has(terminalKey(owner)) && !collapsed.has(terminalKey(owner))
          ? owner
          : null;
    }
    if (!selected)
      selected =
        [...this.knownRuns.values()].reverse().find((r) => !collapsed.has(terminalKey(r.target)))
          ?.target ?? null;
    if (
      (selected ? terminalKey(selected) : null) !==
        (this.state.selected ? terminalKey(this.state.selected) : null) ||
      collapsed.size !== this.state.collapsed.length ||
      this.state.collapsed.some((key) => !collapsed.has(key))
    )
      this.publish({ selected, collapsed: [...collapsed] });
  }
  clearNotice() {
    this.publish({ notice: null });
  }
  cancel(subscription: string) {
    this.pending.get(subscription)?.abort.abort();
  }
  async start(target: TerminalTarget, reuse: TerminalReuse = 'shared') {
    return this.open({ type: 'start', target, reuse });
  }
  async startLocal(name: string) {
    return this.open({ type: 'startLocal', name });
  }
  async attach(target: TerminalTarget) {
    if (target.kind === 'tunnel' || this.buffer(target)) {
      return target;
    }
    const key = terminalKey(target);
    const existing = this.attaching.get(key);
    if (existing) return existing;
    const request = this.open({ type: 'attach', target }).finally(() => {
      if (this.attaching.get(key) === request) this.attaching.delete(key);
    });
    this.attaching.set(key, request);
    return request;
  }
  private async open(request: OpenRequest) {
    if (!this.online()) throw fail('backendNotConnected');
    if (!this.transport) throw fail('terminalUnavailable');
    const generation = this.generation;
    const existingRuns = new Set(this.knownRuns.keys());
    const panelIntent = request.type === 'attach' ? this.panelIntent : ++this.panelIntent;
    const subscription = crypto.randomUUID();
    const buffer = new TerminalBuffer(subscription, new AbortController(), this.limit);
    this.pending.set(subscription, buffer);
    const target = request.type === 'startLocal' ? null : request.target;
    this.publish({
      pending: [
        ...this.state.pending,
        { subscription, target, ...(request.type === 'startLocal' ? { name: request.name } : {}) },
      ],
    });
    const action: Extract<TerminalAction, { type: 'start' | 'attach' | 'startLocal' }> =
      request.type === 'startLocal'
        ? { type: 'startLocal', data: { name: request.name, subscription } }
        : request.type === 'start'
          ? { type: 'start', data: { target: request.target, reuse: request.reuse, subscription } }
          : { type: 'attach', data: { target: request.target, subscription } };
    try {
      const reply = await this.transport.open(
        action,
        (frame) => buffer.accept(frame),
        buffer.abort.signal,
      );
      if (generation !== this.generation || buffer.abort.signal.aborted) throw fail('stopped');
      if (reply.type !== 'terminalOpened') throw fail('generic');
      const run = reply.data.target;
      const key = terminalKey(run);
      if (this.endedWhileOpening.has(key)) {
        buffer.dispose();
        return run;
      }
      // A new command may become ready before its start reply arrives. Do not
      // reopen it in that case. Explicitly starting an existing Shared run does
      // reveal it, just like selecting it in the background list.
      const revealExisting =
        request.type === 'start' && existingRuns.has(key) && panelIntent === this.panelIntent;
      const collapsed = this.state.collapsed.includes(key) && !revealExisting;
      // A snapshot may mount a view before the atomic start subscription returns.
      // Keep whichever live buffer the view already uses; replacing it would
      // strand its output listener, including when reusing a Shared command.
      if (run.kind === 'tunnel' || collapsed || this.buffers.has(key)) buffer.dispose();
      else this.buffers.set(key, buffer);
      // Attaching is an I/O operation, not a request to reveal a panel. A late
      // attachment must not undo a user collapse or steal another tab's focus.
      if (request.type !== 'attach' && !collapsed && panelIntent === this.panelIntent)
        this.show(run);
      return run;
    } catch (error) {
      const cancelled = buffer.abort.signal.aborted;
      buffer.dispose();
      if (cancelled) return null;
      throw new ProblemError(toProblem(error));
    } finally {
      this.pending.delete(subscription);
      if (!this.pending.size) this.endedWhileOpening.clear();
      this.publish({
        pending: this.state.pending.filter((request) => request.subscription !== subscription),
      });
    }
  }
  ended(event: TerminalEnded) {
    const key = `${event.kind}:${event.id}`;
    if (this.pending.size) this.endedWhileOpening.add(key);
    this.buffers.get(key)?.dispose();
    this.buffers.delete(key);
    this.knownRuns.delete(key);
    this.autoCollapsed.delete(key);
    this.updatePanels(
      new Set(this.state.collapsed.filter((item) => item !== key)),
      this.state.selected && terminalKey(this.state.selected) === key ? null : this.state.selected,
    );
    this.publish({ notice: event });
  }
  reconcile({ runs, dependencies }: Pick<TerminalWorkspaceView, 'runs' | 'dependencies'>) {
    const current = new Set(runs.map((run) => terminalKey(run.target)));
    let removedSelected = false;
    for (const key of this.knownRuns.keys())
      if (!current.has(key)) {
        this.buffers.get(key)?.dispose();
        this.buffers.delete(key);
        if (this.pending.size) this.endedWhileOpening.add(key);
        if (this.state.selected && terminalKey(this.state.selected) === key) removedSelected = true;
      }
    this.knownRuns = new Map(runs.map((run) => [terminalKey(run.target), run]));
    this.dependencies = dependencies;
    for (const key of this.autoCollapsed) if (!current.has(key)) this.autoCollapsed.delete(key);
    const collapsed = new Set(this.state.collapsed.filter((key) => current.has(key)));
    const released = new Set(
      dependencies.filter((d) => d.gate === 'released' && d.runId).map((d) => d.runId),
    );
    for (const run of runs) {
      const key = terminalKey(run.target);
      if (
        run.target.kind === 'command' &&
        run.phase === 'running' &&
        (run.readyMarkerReceived || released.has(run.target.id)) &&
        !this.autoCollapsed.has(key)
      ) {
        this.autoCollapsed.add(key);
        collapsed.add(key);
      }
    }
    this.updatePanels(collapsed, removedSelected ? null : this.state.selected);
  }
  lagged() {
    this.publish({ notice: 'lagged' });
  }
  reset() {
    this.generation++;
    this.attaching.clear();
    this.knownRuns.clear();
    this.dependencies = [];
    this.autoCollapsed.clear();
    this.buffers.forEach((buffer) => buffer.dispose());
    this.buffers.clear();
    this.pending.forEach((buffer) => buffer.dispose());
    this.pending.clear();
    this.endedWhileOpening.clear();
    this.publish({ selected: null, collapsed: [], pending: [], notice: null });
  }
  dispose() {
    this.reset();
    this.listeners.clear();
  }
}
