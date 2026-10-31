import { PreviewTerminalFiles } from './terminal-files';
import { errorText, toProblem } from '../shared/i18n/errors';
import type { TerminalTransport } from '../shared/backend-contract';
import type {
  Frame,
  Reply,
  TerminalAction,
  TerminalConfigView,
  TerminalOutputFrame,
  TerminalTarget,
  TerminalWorkspaceView,
} from '../shared/contracts.generated';
import { fail } from '../shared/i18n/errors';

export const terminalFixture = (): TerminalWorkspaceView => ({
  revision: '1',
  closing: false,
  configs: [
    {
      id: '10000000-0000-4000-8000-000000000002',
      kind: 'command',
      data: { name: 'VPN', command: 'icrrvpn -g', autoInputs: [] },
    },
    {
      id: '10000000-0000-4000-8000-000000000010',
      kind: 'ssh',
      data: {
        name: 'Demo server',
        address: { kind: 'host', value: 'demo.example' },
        port: 22,
        username: 'demo',
        auth: 'password',
        passwordSaved: true,
        trustedKey: { algorithm: 'ssh-ed25519', bytes: [1, 2, 3] },
      },
    },
    {
      id: '10000000-0000-4000-8000-000000000011',
      kind: 'terminal',
      data: {
        name: 'Remote workspace',
        sshTarget: '10000000-0000-4000-8000-000000000010',
        dependencies: [],
      },
    },
    {
      id: '10000000-0000-4000-8000-000000000012',
      kind: 'tunnel',
      data: {
        name: 'Development tunnel',
        sshTarget: '10000000-0000-4000-8000-000000000010',
        dependencies: [],
        forward: {
          kind: 'local',
          listenHost: '127.0.0.1',
          listenPort: 8080,
          destinationHost: '127.0.0.1',
          destinationPort: 3000,
        },
      },
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
  private files = new PreviewTerminalFiles();
  private fileTimers = new Map<string, ReturnType<typeof setInterval>>();
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
        } else if (config.kind === 'command') {
          const previous = this.workspace.configs.find((c) => c.id === id);
          const old = previous?.kind === 'command' ? previous.data.autoInputs : [];
          const edits = config.data.autoInputs ?? [];
          const keys = edits.map((rule) => rule.prefix);
          if (
            new Set(keys).size !== keys.length ||
            edits.some(
              (rule) =>
                !rule.prefix ||
                /[\r\n\0]/.test(rule.prefix) ||
                (rule.value == null ? !old.includes(rule.prefix) : /[\r\n\0]/.test(rule.value)),
            )
          )
            throw fail('generic');
          view = {
            id,
            kind: 'command',
            data: {
              name: config.data.name,
              command: config.data.command,
              sshTarget: config.data.sshTarget,
              autoInputs: [
                ...old.filter((key) => keys.includes(key)),
                ...keys.filter((key) => !old.includes(key)),
              ],
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
        if (ssh?.kind === 'ssh' && (!ssh.data.passwordSaved || !ssh.data.trustedKey))
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
          data: this.files.browse(action.data.path ?? undefined),
        };
      case 'mkdir':
        this.files.add(action.data.path, true);
        break;
      case 'rename':
        this.files.rename(action.data.source, action.data.destination);
        break;
      case 'upload':
      case 'download':
      case 'deleteFiles': {
        const id = crypto.randomUUID();
        const operation = action.type === 'deleteFiles' ? 'delete' : action.type;
        this.workspace.files.push({
          id,
          terminalId: action.data.id,
          operation,
          bytes: '0',
          completed: '0',
          total: '1',
          totalBytes: operation === 'delete' ? '0' : '8192',
          phase: 'running',
        });
        let step = 0;
        const timer = setInterval(() => {
          const file = this.workspace.files.find((file) => file.id === id);
          if (!file) return;
          step++;
          file.bytes = operation === 'delete' ? '0' : String(step * 2048);
          if (step < 4) {
            this.changed();
            return;
          }
          let error: string | undefined;
          try {
            if (action.type === 'upload') {
              const path = `${action.data.directory}/${action.data.folder ? 'uploaded-folder' : 'uploaded-file.txt'}`;
              const entries = this.files.browse(action.data.directory).entries;
              const exists = entries.some((entry) => entry.name === path.split('/').pop());
              if (exists && action.data.conflict === 'overwrite') this.files.remove(path);
              if (!(exists && action.data.conflict === 'skip'))
                this.files.add(path, action.data.folder);
            } else if (action.type === 'deleteFiles')
              action.data.paths.forEach((path) => this.files.remove(path));
          } catch (reason) {
            error = errorText(toProblem(reason));
          }
          this.finishFile(id, error ? 'failed' : 'succeeded', error);
        }, 650);
        this.fileTimers.set(id, timer);
        break;
      }
      case 'stopFiles':
        this.finishFile(action.data.id, 'stopped');
        break;
      case 'options':
        this.workspace.options = action.data.options;
        break;
    }
    this.changed();
    return { type: 'done' };
  }
  private finishFile(id: string, result: string, error?: string) {
    clearInterval(this.fileTimers.get(id));
    this.fileTimers.delete(id);
    this.workspace.files = this.workspace.files.filter((file) => file.id !== id);
    this.changed();
    this.emit({
      type: 'terminalEnded',
      data: { id, kind: 'file', name: '', result, error, cleanup: [] },
    });
  }
  reset() {
    this.fileTimers.forEach(clearInterval);
    this.fileTimers.clear();
    this.files = new PreviewTerminalFiles();
    this.streams.clear();
    this.offsets.clear();
    this.workspace = terminalFixture();
    this.changed();
  }
  dispose() {
    this.fileTimers.forEach(clearInterval);
    this.fileTimers.clear();
    this.streams.clear();
    this.offsets.clear();
  }
}
