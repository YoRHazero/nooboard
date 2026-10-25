import type { TerminalTransport } from '../client/bridge/port';
import type {
  Frame,
  Reply,
  TerminalAction,
  TerminalConfigView,
  TerminalOutputFrame,
  TerminalTarget,
  TerminalWorkspaceView,
} from '../client/bridge/generated';
import { fail } from '../shared/i18n/errors';

export const terminalFixture = (): TerminalWorkspaceView => ({
  revision: '1',
  closing: false,
  configs: [
    {
      id: '10000000-0000-4000-8000-000000000002',
      kind: 'command',
      data: { name: 'VPN', command: 'icrrvpn -g' },
    },
  ],
  runs: [],
  dependencies: [],
  prompts: [],
  files: [],
  options: {
    gracefulStopMs: '5000',
    forceStopMs: '3000',
    shutdownMs: '15000',
    connectMs: '15000',
    requestMs: '30000',
    replayBytes: '262144',
  },
});
type Receive = (frame: TerminalOutputFrame) => void | Promise<void>;
/** UI-only fixtures. Never executes a shell or connects to a host. */
export class PreviewTerminal implements TerminalTransport {
  private workspace = terminalFixture();
  private streams = new Map<string, { target: TerminalTarget; receive: Receive }>();
  private offsets = new Map<string, number>();
  private sequence = 0;
  constructor(
    private publish: (workspace: TerminalWorkspaceView) => void,
    private emit: (frame: Frame) => void,
  ) {}
  private changed() {
    this.workspace.revision = String(BigInt(this.workspace.revision) + 1n);
    this.publish(structuredClone(this.workspace));
  }
  private async output(target: TerminalTarget, text: string) {
    const bytes = [...new TextEncoder().encode(text)];
    const offset = this.offsets.get(target.id) ?? 0;
    this.offsets.set(target.id, offset + bytes.length);
    for (const stream of this.streams.values())
      if (stream.target.id === target.id)
        await stream.receive({
          type: 'data',
          data: { sequence: String(++this.sequence), offset: String(offset), bytes },
        });
  }
  async open(
    action: Extract<TerminalAction, { type: 'start' | 'attach' | 'startLocal' }>,
    receive: Receive,
    signal: AbortSignal,
  ): Promise<Reply> {
    if (signal.aborted) throw fail('stopped');
    const reply = await this.request(action);
    if (reply.type === 'terminalOpened' && reply.data.target.kind !== 'tunnel' && !signal.aborted) {
      const target = reply.data.target;
      this.streams.set(action.data.subscription, { target, receive });
      signal.addEventListener(
        'abort',
        () => {
          this.streams.delete(action.data.subscription);
        },
        { once: true },
      );
      await this.output(target, '\r\nPreview terminal. Commands are not executed.\r\n$ ');
    }
    return reply;
  }
  async request(action: TerminalAction): Promise<Reply> {
    switch (action.type) {
      case 'saveConfig': {
        const { config } = action.data;
        const id = action.data.id ?? crypto.randomUUID();
        let view: TerminalConfigView;
        if (config.kind === 'ssh') {
          const previous = this.workspace.configs.find((c) => c.id === id);
          view = {
            id,
            kind: 'ssh',
            data: {
              name: config.data.name,
              address: config.data.address,
              port: config.data.port,
              username: config.data.username,
              auth:
                config.data.auth === 'keep' && previous?.kind === 'ssh'
                  ? previous.data.auth
                  : config.data.auth === 'privateKey'
                    ? 'privateKey'
                    : 'password',
              passwordSaved: previous?.kind === 'ssh' && previous.data.passwordSaved,
              trustedKey: previous?.kind === 'ssh' ? previous.data.trustedKey : null,
            },
          };
        } else view = { ...config, id };
        this.workspace.configs = [...this.workspace.configs.filter((c) => c.id !== id), view];
        this.changed();
        return { type: 'terminalSaved', data: { id } };
      }
      case 'copyConfig': {
        const config = this.workspace.configs.find((c) => c.id === action.data.target.id);
        if (!config) throw fail('generic');
        const copy = structuredClone(config);
        copy.id = crypto.randomUUID();
        copy.data.name += ' (copy)';
        if (copy.kind === 'ssh') copy.data.trustedKey = null;
        this.workspace.configs.push(copy);
        this.changed();
        return { type: 'terminalSaved', data: { id: copy.id } };
      }
      case 'deleteConfig': {
        const id = action.data.target.id;
        if (
          this.workspace.runs.some((r) => r.configId === id) ||
          this.workspace.configs.some(
            (c) =>
              ('sshTarget' in c.data && c.data.sshTarget === id) ||
              ('dependencies' in c.data && c.data.dependencies.some((d) => d.commandId === id)),
          )
        )
          throw fail('terminalConfigInUse');
        this.workspace.configs = this.workspace.configs.filter((c) => c.id !== id);
        break;
      }
      case 'startLocal': {
        const target: TerminalTarget = { kind: 'terminal', id: crypto.randomUUID() };
        this.workspace.runs.push({
          target,
          configId: null,
          name: action.data.name,
          phase: 'running',
          readyMarkerReceived: false,
        });
        this.changed();
        return { type: 'terminalOpened', data: { target } };
      }
      case 'start': {
        const config = this.workspace.configs.find((c) => c.id === action.data.target.id);
        if (!config || config.kind === 'ssh') throw fail('generic');
        const existing =
          config.kind === 'command' && action.data.reuse === 'shared'
            ? [...this.workspace.runs].reverse().find((r) => r.configId === config.id)
            : undefined;
        if (existing) return { type: 'terminalOpened', data: { target: existing.target } };
        const target = { kind: config.kind, id: crypto.randomUUID() };
        this.workspace.runs.push({
          target,
          configId: config.id,
          name: config.data.name,
          phase: 'running',
          sshTarget: config.data.sshTarget,
          command: config.kind === 'command' ? config.data.command : null,
          lifetime: config.kind === 'command' ? 'independent' : null,
          readyMarkerReceived: false,
          forward: config.kind === 'tunnel' ? config.data.forward : null,
        });
        if ('dependencies' in config.data)
          for (const dependency of config.data.dependencies) {
            const command = this.workspace.configs.find((c) => c.id === dependency.commandId);
            if (command?.kind !== 'command') continue;
            const started = await this.request({
              type: 'start',
              data: {
                target: { kind: 'command', id: command.id },
                reuse: dependency.reuse,
                subscription: crypto.randomUUID(),
              },
            });
            if (started.type === 'terminalOpened')
              this.workspace.dependencies.push({
                id: crypto.randomUUID(),
                owner: target,
                commandId: command.id,
                runId: started.data.target.id,
                gate: 'waitingForRelease',
              });
          }
        const ssh = this.workspace.configs.find((c) => c.id === config.data.sshTarget);
        if (ssh?.kind === 'ssh')
          this.workspace.prompts.push({
            id: crypto.randomUUID(),
            targetId: ssh.id,
            owner: target,
            prompt: ssh.data.trustedKey
              ? { kind: 'password', data: { username: ssh.data.username } }
              : {
                  kind: 'host',
                  data: {
                    address: ssh.data.address.value,
                    fingerprint: 'SHA256:preview-only',
                    key: { algorithm: 'ssh-ed25519', bytes: [1, 2, 3] },
                  },
                },
          });
        this.changed();
        return { type: 'terminalOpened', data: { target } };
      }
      case 'attach':
        return { type: 'terminalOpened', data: { target: action.data.target } };
      case 'ack':
        break;
      case 'detach':
        this.streams.delete(action.data.subscription);
        break;
      case 'input':
        await this.output(
          action.data.target,
          new TextDecoder()
            .decode(new Uint8Array(action.data.bytes))
            .replace(/\r/g, '\r\nPreview command not executed.\r\n$ '),
        );
        break;
      case 'resize':
        break;
      case 'stop': {
        const run = this.workspace.runs.find((r) => r.target.id === action.data.target.id);
        this.workspace.runs = this.workspace.runs.filter(
          (r) => r.target.id !== action.data.target.id,
        );
        this.workspace.dependencies = this.workspace.dependencies.filter(
          (d) => d.owner.id !== action.data.target.id,
        );
        this.workspace.prompts = this.workspace.prompts.filter(
          (p) => p.owner.id !== action.data.target.id,
        );
        if (run)
          this.emit({
            type: 'terminalEnded',
            data: {
              kind: run.target.kind,
              id: run.target.id,
              name: run.name,
              result: 'stopped',
              cleanup: [],
            },
          });
        break;
      }
      case 'continue': {
        const d = this.workspace.dependencies.find((d) => d.id === action.data.id);
        if (d) d.gate = 'released';
        break;
      }
      case 'lifetime': {
        const run = this.workspace.runs.find((r) => r.target.id === action.data.id);
        if (run) run.lifetime = action.data.policy;
        break;
      }
      case 'confirmHost': {
        const prompt = this.workspace.prompts.find((p) => p.id === action.data.id);
        const ssh = this.workspace.configs.find((c) => c.id === prompt?.targetId);
        this.workspace.prompts = this.workspace.prompts.filter((p) => p.id !== action.data.id);
        if (action.data.accept && prompt?.prompt.kind === 'host' && ssh?.kind === 'ssh') {
          ssh.data.trustedKey = prompt.prompt.data.key;
          this.workspace.prompts.push({
            ...prompt,
            id: crypto.randomUUID(),
            prompt: { kind: 'password', data: { username: ssh.data.username } },
          });
        }
        break;
      }
      case 'password': {
        const prompt = this.workspace.prompts.find((p) => p.id === action.data.id);
        const ssh = this.workspace.configs.find((c) => c.id === prompt?.targetId);
        if (ssh?.kind === 'ssh' && action.data.remember) ssh.data.passwordSaved = true;
        this.workspace.prompts = this.workspace.prompts.filter((p) => p.id !== action.data.id);
        break;
      }
      case 'cancelPrompt':
        this.workspace.prompts = this.workspace.prompts.filter((p) => p.id !== action.data.id);
        break;
      case 'replaceHost': {
        const ssh = this.workspace.configs.find((c) => c.id === action.data.targetId);
        if (ssh?.kind === 'ssh') ssh.data.trustedKey = action.data.replacement;
        this.workspace.issue = null;
        break;
      }
      case 'browse':
        return {
          type: 'terminalDirectory',
          data: {
            path: action.data.path ?? '/home/demo',
            entries: [
              { name: 'projects', kind: 'directory' },
              { name: 'README.md', kind: 'file', size: '2048' },
              { name: 'notes.txt', kind: 'file', size: '256' },
            ],
          },
        };
      case 'mkdir':
      case 'rename':
      case 'upload':
      case 'download':
      case 'deleteFiles':
      case 'stopFiles':
        break;
      case 'options':
        this.workspace.options = action.data.options;
        break;
    }
    this.changed();
    return { type: 'done' };
  }
  reset() {
    this.streams.clear();
    this.offsets.clear();
    this.workspace = terminalFixture();
    this.changed();
  }
  dispose() {
    this.streams.clear();
    this.offsets.clear();
  }
}
