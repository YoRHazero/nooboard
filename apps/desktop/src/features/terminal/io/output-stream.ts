import { getBackendSnapshot } from '../../../shared/backend';
import { TerminalBuffer } from './replay-buffer';
import { terminalKey } from '../identity';
import { terminalStore, type TerminalStore } from '../store';
import type { TerminalSessionState } from '../store/session.slice';
import { fail, ProblemError, toProblem } from '../../../shared/i18n/errors';
import type { TerminalTransport } from '../../../shared/backend-contract';
import type {
  Reply,
  TerminalAction,
  TerminalEnded,
  TerminalReuse,
  TerminalTarget,
  TerminalWorkspaceView,
} from '../../../shared/contracts.generated';
type OpenRequest =
  | { type: 'start'; target: TerminalTarget; reuse: TerminalReuse }
  | { type: 'attach'; target: TerminalTarget }
  | { type: 'startLocal'; name: string };
export class TerminalStreams {
  private get state() {
    return this.ui.getState();
  }
  private buffers = new Map<string, TerminalBuffer>();
  private pending = new Map<string, TerminalBuffer>();
  private endedWhileOpening = new Set<string>();
  private knownRunKeys = new Set<string>();
  private stopUi: () => void;
  private get panelIntent() {
    return this.ui.getState().selectionIntent;
  }
  private set panelIntent(value: number) {
    this.ui.setState({ selectionIntent: value });
  }
  private generation = 0;
  private attaching = new Map<string, Promise<TerminalTarget | null>>();
  constructor(
    private transport: TerminalTransport | undefined,
    private online: () => boolean,
    private limit: () => number,
    readonly ui: TerminalStore = terminalStore,
    private workspace: () => Pick<TerminalWorkspaceView, 'runs' | 'dependencies'> = () =>
      getBackendSnapshot().terminal,
  ) {
    this.stopUi = ui.subscribe((next, previous) => {
      if (next.collapsed === previous.collapsed) return;
      for (const key of next.collapsed) {
        this.buffers.get(key)?.dispose();
        this.buffers.delete(key);
      }
    });
  }
  private publish(patch: Partial<TerminalSessionState>) {
    this.ui.setState(patch);
  }
  async request(action: Extract<TerminalAction, { type: 'input' | 'resize' }>): Promise<Reply> {
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
  private show(target: TerminalTarget | null) {
    this.publish({
      selected: target,
      collapsed: target
        ? this.state.collapsed.filter((key) => key !== terminalKey(target))
        : this.state.collapsed,
    });
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
    const existingRuns = new Set(this.knownRunKeys);
    const panelIntent = request.type === 'attach' ? this.panelIntent : ++this.panelIntent;
    const subscription = crypto.randomUUID();
    const buffer = new TerminalBuffer(subscription, new AbortController(), this.limit);
    this.pending.set(subscription, buffer);
    const target = request.type === 'startLocal' ? null : request.target;
    this.publish({
      pending: [
        ...this.state.pending,
        {
          subscription,
          target,
          attaching: request.type === 'attach',
          ...(request.type === 'startLocal' ? { name: request.name } : {}),
        },
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
      // A user may minimize a new instance before its start reply arrives. Do not
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
    if (event.kind === 'terminal') this.ui.getState().forgetFileView(event.id);
    const key = `${event.kind}:${event.id}`;
    if (this.pending.size) this.endedWhileOpening.add(key);
    this.buffers.get(key)?.dispose();
    this.buffers.delete(key);
    this.knownRunKeys.delete(key);
    if (
      event.kind === 'terminal' ||
      event.kind === 'command' ||
      event.kind === 'tunnel' ||
      event.kind === 'ssh'
    )
      this.ui.getState().reconcile(this.workspace(), { kind: event.kind, id: event.id });
    this.publish({ notice: event });
  }
  reconcile(workspace: TerminalWorkspaceView) {
    const current = new Set(workspace.runs.map((run) => terminalKey(run.target)));
    for (const key of this.knownRunKeys)
      if (!current.has(key)) {
        this.buffers.get(key)?.dispose();
        this.buffers.delete(key);
        if (key.startsWith('terminal:')) this.ui.getState().forgetFileView(key.slice(9));
        if (this.pending.size) this.endedWhileOpening.add(key);
      }
    this.knownRunKeys = current;
    this.ui.getState().reconcile(workspace);
  }
  lagged() {
    this.publish({ notice: 'lagged' });
  }
  reset() {
    this.generation++;
    this.attaching.clear();
    this.knownRunKeys.clear();
    this.buffers.forEach((buffer) => buffer.dispose());
    this.buffers.clear();
    this.pending.forEach((buffer) => buffer.dispose());
    this.pending.clear();
    this.endedWhileOpening.clear();
    this.ui.getState().resetSession();
    this.ui.getState().forgetFileView();
    this.ui.getState().closeFiles();
  }
  dispose() {
    this.reset();
    this.stopUi();
  }
}

let active: TerminalStreams | undefined;
export function installTerminalStreams(streams: TerminalStreams) {
  active = streams;
}
export function getTerminalStreams() {
  if (!active) throw fail('terminalUnavailable');
  return active;
}
