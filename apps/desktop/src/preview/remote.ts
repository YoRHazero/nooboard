/** Scripted examples for the SSH UI. No network or credential implementation. */
import type { Frame, RemoteReply, RemoteRequest } from '../desktop/bridge/generated';
import { emptyRemote, type RemoteSnapshot } from '../desktop/remote/model';
import { fail } from '../i18n/errors';
export class RemotePreview {
  state: RemoteSnapshot = { ...emptyRemote(), downloadDirectory: '/Users/you/Downloads' };
  private output = new Map<string, number[]>();
  private remembered = new Map<string, boolean>();
  private timers = new Set<ReturnType<typeof setTimeout>>();
  constructor(private emit: (frame: Frame) => void) {}
  private publish() {
    this.state.revision = String(BigInt(this.state.revision) + 1n);
    this.emit({ type: 'remote', data: structuredClone(this.state) });
  }
  private ready(id: string) {
    const w = this.state.workspaces.find((w) => w.id === id);
    if (!w) return;
    w.address = `192.168.1.42:${w.port}`;
    w.terminal = { state: 'Ready' };
    w.files = { state: 'Ready' };
    w.root = w.initialDirectory = '/home/you';
    const text = `\x1b[32mConnected to ${w.username}@remote\x1b[0m\r\n\r\nWelcome to the nooboard SSH preview.\r\nFile browsing and transfers use an independent SFTP connection.\r\n\r\n${w.username}@remote:~ $ `;
    this.output.set(w.terminalId, [...new TextEncoder().encode(text)]);
    this.state.profiles = [
      {
        id: w.profileId,
        deviceId: w.deviceId,
        username: w.username,
        port: w.port,
        remembered: this.remembered.get(w.id) ?? false,
      },
    ];
  }
  async request(request: RemoteRequest): Promise<RemoteReply> {
    switch (request.type) {
      case 'connect': {
        const d = request.data;
        const id = d.workspace ?? crypto.randomUUID();
        const old = this.state.workspaces.find((w) => w.id === id);
        if (old) {
          old.terminalId = crypto.randomUUID();
          old.terminal = { state: 'Connecting' };
        } else
          this.state.workspaces.push({
            id,
            profileId: `${d.device}:${d.username}:${d.port}`,
            deviceId: d.device,
            username: d.username,
            port: d.port,
            terminalId: id,
            filesId: id,
            terminal: { state: 'Connecting' },
            files: { state: 'Connecting' },
            generation: '0',
            directories: [],
          });
        this.remembered.set(id, d.remember);
        this.state.challenges = [
          {
            id: 'preview-host-key',
            sessionId: old?.terminalId ?? id,
            address: '192.168.1.42:22',
            algorithm: 'ssh-ed25519',
            fingerprint: 'SHA256:f9gNprT7Yz/9aDtEu+5MqfHUQH98PKXxSWJ6YybXysc',
          },
        ];
        this.publish();
        return { type: 'connected', data: { id } };
      }
      case 'hostKey': {
        const w = this.state.workspaces.find(
          (w) => w.terminalId === this.state.challenges[0]?.sessionId,
        );
        this.state.challenges = [];
        if (w) {
          if (request.data.accept) this.ready(w.id);
          else {
            w.terminal = { state: 'Failed', error: { code: 'sshHostKeyRejected' } };
            w.files = { state: 'Failed', error: { code: 'sshHostKeyRejected' } };
          }
        }
        break;
      }
      case 'close':
        this.state.workspaces = this.state.workspaces.filter(
          (w) => w.id !== request.data.workspace,
        );
        break;
      case 'replay': {
        const w = this.state.workspaces.find((w) => w.id === request.data.workspace);
        const output = this.output.get(w?.terminalId ?? '') ?? [];
        const start = Math.min(output.length, Number(request.data.after));
        return {
          type: 'replay',
          data: {
            start: String(start),
            end: String(output.length),
            gap: false,
            bytes: output.slice(start),
          },
        };
      }
      case 'input': {
        const w = this.state.workspaces.find((w) => w.id === request.data.workspace);
        if (w) {
          let bytes = request.data.bytes;
          if (bytes.includes(13))
            bytes = [...new TextEncoder().encode(`\r\n${w.username}@remote:~ $ `)];
          const output = this.output.get(w.terminalId) ?? [];
          this.emit({
            type: 'terminal',
            data: { sessionId: w.terminalId, offset: String(output.length), bytes },
          });
          output.push(...bytes);
          this.output.set(w.terminalId, output);
        }
        return { type: 'done' };
      }
      case 'navigate': {
        const w = this.state.workspaces.find((w) => w.id === request.data.target.workspace);
        if (w) {
          const p = request.data.path;
          if (!['/home/you', '/home/you/projects', 'projects', '/tmp'].includes(p))
            throw fail('sshNotFound');
          w.root = p === 'projects' ? '/home/you/projects' : p;
          w.directories = [];
          w.generation = String(BigInt(w.generation) + 1n);
        }
        break;
      }
      case 'list': {
        const w = this.state.workspaces.find((w) => w.id === request.data.target.workspace);
        if (w) {
          const path = request.data.path;
          const entries =
            path === '/home/you'
              ? [
                  { name: 'projects', kind: 'Directory' as const },
                  { name: 'notes.txt', kind: 'File' as const },
                  { name: '.config', kind: 'Directory' as const },
                  { name: 'archive.zip', kind: 'File' as const },
                ]
              : path === '/home/you/projects'
                ? [
                    { name: 'README.md', kind: 'File' as const },
                    { name: 'src', kind: 'Directory' as const },
                  ]
                : [];
          w.directories = w.directories.filter((d) => d.path !== path);
          w.directories.push({
            path,
            state: 'Complete',
            entries: entries.map((e) => ({
              ...e,
              path: `${path}/${e.name}`,
              size: e.kind === 'File' ? 2048 : undefined,
            })),
          });
        }
        break;
      }
      case 'download':
      case 'selectUpload':
      case 'upload': {
        const w = this.state.workspaces.find((w) => w.id === request.data.target.workspace);
        if (!w) throw fail('offline');
        const paths =
          request.type === 'selectUpload'
            ? ['/Users/you/Documents/report.pdf']
            : request.data.paths;
        for (const source of paths) {
          const id = crypto.randomUUID();
          const direction = request.type === 'download' ? 'Download' : 'Upload';
          const destination =
            request.type === 'download' ? this.state.downloadDirectory! : request.data.directory;
          this.state.transfers.push({
            id,
            sessionId: w.filesId,
            deviceId: w.deviceId,
            direction,
            source,
            destination,
            state: 'Running',
            transferred: 2048,
            total: 8192,
          });
          const timer = setTimeout(() => {
            this.timers.delete(timer);
            const task = this.state.transfers.find((t) => t.id === id);
            if (task && task.state === 'Running') {
              task.state = 'Succeeded';
              task.transferred = 8192;
              task.target = `${destination}/${source.split('/').at(-1)}`;
              this.publish();
            }
          }, 900);
          this.timers.add(timer);
        }
        break;
      }
      case 'cancel': {
        const task = this.state.transfers.find((t) => t.id === request.data.id);
        if (task) task.state = 'Cancelled';
        break;
      }
      case 'forgetTransfer':
        this.state.transfers = this.state.transfers.filter((t) => t.id !== request.data.id);
        break;
      case 'selectDownloadDirectory':
        this.state.downloadDirectory = '/Users/you/Downloads';
        break;
      case 'retry':
      case 'collapse':
      case 'resize':
      case 'reveal':
      case 'forgetHostKey':
        break;
      default: {
        const exhaustive: never = request;
        throw new Error(String(exhaustive));
      }
    }
    this.publish();
    return { type: 'done' };
  }
  dispose() {
    this.timers.forEach(clearTimeout);
    this.timers.clear();
  }
}
