import { SshAddressFields, type SshAddressDraft } from './SshAddressFields';
import { Select, SelectItem } from '../../shared/ui/Select';
import { useState } from 'react';
import { ArrowUp, Plus, X } from 'lucide-react';
import type {
  TerminalConfigInput,
  TerminalConfigView,
  TerminalDependency,
  TerminalResourceKind,
} from '../../client/api';
import { useCommand, useDesktop, useSnapshot } from '../../client/react';
import { useI18n } from '../../shared/i18n/react';
import { errorText } from '../../shared/i18n/errors';
import { Dialog } from '../../shared/ui/Dialog';
import { Button, IconButton } from '../../shared/ui/controls';

export function ConfigEditor({
  config,
  initialKind = 'terminal',
  initialSshTarget = '',
  onClose,
}: {
  config?: TerminalConfigView;
  initialKind?: TerminalResourceKind;
  initialSshTarget?: string;
  onClose: () => void;
}) {
  const { t } = useI18n();
  const { terminal } = useDesktop();
  const workspace = useSnapshot((s) => s.terminal);
  const { execute, pending, error } = useCommand();
  const [kind, setKind] = useState(config?.kind ?? initialKind);
  const [name, setName] = useState(config?.data.name ?? '');
  const [sshTarget, setSshTarget] = useState(
    config && 'sshTarget' in config.data ? (config.data.sshTarget ?? '') : initialSshTarget,
  );
  const [command, setCommand] = useState(config?.kind === 'command' ? config.data.command : '');
  const [dependencies, setDependencies] = useState<TerminalDependency[]>(
    config && 'dependencies' in config.data ? config.data.dependencies : [],
  );
  const [address, setAddress] = useState<SshAddressDraft>({
    kind: config?.kind === 'ssh' ? config.data.address.kind : 'host',
    host:
      config?.kind === 'ssh' && config.data.address.kind === 'host'
        ? config.data.address.value
        : '',
    deviceId:
      config?.kind === 'ssh' && config.data.address.kind === 'device'
        ? config.data.address.value
        : '',
    port: config?.kind === 'ssh' ? String(config.data.port) : '22',
  });
  const [username, setUsername] = useState(config?.kind === 'ssh' ? config.data.username : '');
  const [auth, setAuth] = useState<'keep' | 'password' | 'privateKey'>(
    config?.kind === 'ssh' ? 'keep' : 'password',
  );
  const [privateKey, setPrivateKey] = useState('');
  const [passphrase, setPassphrase] = useState('');
  const [forwardKind, setForwardKind] = useState<'local' | 'remote' | 'dynamic'>(
    config?.kind === 'tunnel' ? config.data.forward.kind : 'local',
  );
  const [listenHost, setListenHost] = useState(
    config?.kind === 'tunnel' ? config.data.forward.listenHost : '127.0.0.1',
  );
  const [listenPort, setListenPort] = useState(
    config?.kind === 'tunnel' ? String(config.data.forward.listenPort) : '8080',
  );
  const [destinationHost, setDestinationHost] = useState(
    config?.kind === 'tunnel' ? (config.data.forward.destinationHost ?? '') : '127.0.0.1',
  );
  const [destinationPort, setDestinationPort] = useState(
    config?.kind === 'tunnel' ? String(config.data.forward.destinationPort ?? '') : '80',
  );
  const commands = workspace.configs.filter((c) => c.kind === 'command');
  const targets = workspace.configs.filter((c) => c.kind === 'ssh');
  const save = () =>
    execute(async () => {
      let input: TerminalConfigInput;
      switch (kind) {
        case 'terminal':
          input = { kind, data: { name: name.trim(), sshTarget: sshTarget || null, dependencies } };
          break;
        case 'command':
          input = { kind, data: { name: name.trim(), sshTarget: sshTarget || null, command } };
          break;
        case 'ssh':
          input = {
            kind,
            data: {
              name: name.trim(),
              address: {
                kind: address.kind,
                value: address.kind === 'host' ? address.host.trim() : address.deviceId,
              },
              port: Number(address.port),
              username: username.trim(),
              auth,
              ...(auth === 'privateKey' ? { privateKey, passphrase: passphrase || null } : {}),
            },
          };
          break;
        case 'tunnel':
          input = {
            kind,
            data: {
              name: name.trim(),
              sshTarget,
              dependencies,
              forward: {
                kind: forwardKind,
                listenHost: listenHost.trim(),
                listenPort: Number(listenPort),
                ...(forwardKind === 'dynamic'
                  ? {}
                  : {
                      destinationHost: destinationHost.trim(),
                      destinationPort: Number(destinationPort),
                    }),
              },
            },
          };
          break;
      }
      await terminal.request({ type: 'saveConfig', data: { id: config?.id, config: input } });
      setPrivateKey('');
      setPassphrase('');
      onClose();
    });
  return (
    <Dialog title={config ? t('terminal:edit') : t('terminal:new')} onClose={onClose}>
      <form
        className="terminal-form"
        onSubmit={(event) => {
          event.preventDefault();
          if (!pending) void save();
        }}
      >
        <fieldset disabled={pending}>
          <div className="terminal-form-grid">
            <label className="form-field">
              <span>{t('terminal:kind')}</span>
              <Select
                aria-label={t('terminal:kind')}
                value={kind}
                disabled={!!config}
                onValueChange={(e) => setKind(e as TerminalResourceKind)}
              >
                {(['terminal', 'command', 'tunnel', 'ssh'] as const).map((kind) => (
                  <SelectItem key={kind} value={kind}>
                    {t(`terminal:${kind}`)}
                  </SelectItem>
                ))}
              </Select>
            </label>
            <label className="form-field">
              <span>{t('terminal:name')}</span>
              <input
                required
                autoFocus
                data-autofocus
                value={name}
                onChange={(e) => setName(e.target.value)}
              />
            </label>
          </div>
          {kind !== 'ssh' && (
            <label className="form-field">
              <span>{t('terminal:sshTarget')}</span>
              <Select
                required={kind === 'tunnel'}
                value={sshTarget}
                onValueChange={(e) => setSshTarget(e)}
              >
                <SelectItem value="">
                  {kind === 'tunnel' ? t('terminal:selectTarget') : t('terminal:local')}
                </SelectItem>
                {targets.map((c) => (
                  <SelectItem key={c.id} value={c.id}>
                    {c.data.name}
                  </SelectItem>
                ))}
              </Select>
            </label>
          )}
          {kind === 'command' && (
            <label className="form-field">
              <span>{t('terminal:runCommand')}</span>
              <textarea
                required
                spellCheck={false}
                className="mono"
                rows={3}
                value={command}
                onChange={(e) => setCommand(e.target.value)}
              />
            </label>
          )}
          {kind === 'ssh' && (
            <>
              <SshAddressFields value={address} onChange={setAddress} />
              <div className="terminal-form-grid">
                <label className="form-field">
                  <span>{t('terminal:username')}</span>
                  <input
                    required
                    value={username}
                    autoComplete="off"
                    onChange={(e) => setUsername(e.target.value)}
                  />
                </label>
                <label className="form-field">
                  <span>{t('terminal:authentication')}</span>
                  <Select
                    aria-label={t('terminal:authentication')}
                    value={auth}
                    onValueChange={(e) => setAuth(e as typeof auth)}
                  >
                    {config && <SelectItem value="keep">{t('terminal:keep')}</SelectItem>}
                    <SelectItem value="password">{t('terminal:password')}</SelectItem>
                    <SelectItem value="privateKey">{t('terminal:privateKey')}</SelectItem>
                  </Select>
                </label>
              </div>
              {auth === 'password' && <p className="muted small">{t('terminal:passwordHint')}</p>}
              {auth === 'privateKey' && (
                <>
                  <label className="form-field">
                    <span>{t('terminal:keyFile')}</span>
                    <input
                      type="file"
                      onChange={(e) => {
                        const file = e.target.files?.[0];
                        if (file) void execute(async () => setPrivateKey(await file.text()));
                      }}
                    />
                  </label>
                  <label className="form-field">
                    <span>{t('terminal:keyContent')}</span>
                    <textarea
                      required
                      className="mono"
                      rows={4}
                      autoComplete="off"
                      autoCorrect="off"
                      spellCheck={false}
                      value={privateKey}
                      onChange={(e) => setPrivateKey(e.target.value)}
                    />
                  </label>
                  <label className="form-field">
                    <span>{t('terminal:passphrase')}</span>
                    <input
                      type="password"
                      autoComplete="new-password"
                      value={passphrase}
                      onChange={(e) => setPassphrase(e.target.value)}
                    />
                  </label>
                  <p className="muted small">{t('terminal:keyHint')}</p>
                </>
              )}
            </>
          )}
          {kind === 'tunnel' && (
            <>
              <label className="form-field">
                <span>{t('terminal:forwardKind')}</span>
                <Select
                  aria-label={t('terminal:forwardKind')}
                  value={forwardKind}
                  onValueChange={(e) => setForwardKind(e as typeof forwardKind)}
                >
                  <SelectItem value="local">{t('terminal:localForward')}</SelectItem>
                  <SelectItem value="remote">{t('terminal:remoteForward')}</SelectItem>
                  <SelectItem value="dynamic">{t('terminal:dynamicForward')}</SelectItem>
                </Select>
              </label>
              <div className="terminal-form-grid">
                <label className="form-field">
                  <span>{t('terminal:listenHost')}</span>
                  <input
                    required
                    value={listenHost}
                    onChange={(e) => setListenHost(e.target.value)}
                  />
                </label>
                <label className="form-field">
                  <span>{t('terminal:listenPort')}</span>
                  <input
                    type="number"
                    min="1"
                    max="65535"
                    required
                    value={listenPort}
                    onChange={(e) => setListenPort(e.target.value)}
                  />
                </label>
                {forwardKind !== 'dynamic' && (
                  <>
                    <label className="form-field">
                      <span>{t('terminal:destinationHost')}</span>
                      <input
                        required
                        value={destinationHost}
                        onChange={(e) => setDestinationHost(e.target.value)}
                      />
                    </label>
                    <label className="form-field">
                      <span>{t('terminal:destinationPort')}</span>
                      <input
                        type="number"
                        min="1"
                        max="65535"
                        required
                        value={destinationPort}
                        onChange={(e) => setDestinationPort(e.target.value)}
                      />
                    </label>
                  </>
                )}
              </div>
            </>
          )}
          {(kind === 'terminal' || kind === 'tunnel') && (
            <section className="terminal-prerequisites">
              <h3>{t('terminal:dependencies')}</h3>
              <p className="muted small">{t('terminal:dependencyHint')}</p>
              {dependencies.map((dep, index) => (
                <div className="terminal-dependency-edit" key={index}>
                  <span>{index + 1}.</span>
                  <Select
                    aria-label={t('terminal:command')}
                    value={dep.commandId}
                    onValueChange={(e) =>
                      setDependencies((items) =>
                        items.map((item, i) => (i === index ? { ...item, commandId: e } : item)),
                      )
                    }
                  >
                    {commands.map((c) => (
                      <SelectItem key={c.id} value={c.id}>
                        {c.data.name}
                      </SelectItem>
                    ))}
                  </Select>
                  <Select
                    aria-label={t('terminal:reuse')}
                    value={dep.reuse}
                    onValueChange={(e) =>
                      setDependencies((items) =>
                        items.map((item, i) =>
                          i === index ? { ...item, reuse: e as TerminalDependency['reuse'] } : item,
                        ),
                      )
                    }
                  >
                    <SelectItem value="shared">{t('terminal:shared')}</SelectItem>
                    <SelectItem value="alwaysNew">{t('terminal:alwaysNew')}</SelectItem>
                  </Select>
                  <IconButton
                    label={t('terminal:moveUp')}
                    disabled={!index}
                    onClick={() =>
                      setDependencies((items) => {
                        const next = [...items];
                        [next[index - 1], next[index]] = [next[index], next[index - 1]];
                        return next;
                      })
                    }
                  >
                    <ArrowUp size={14} />
                  </IconButton>
                  <IconButton
                    label={t('terminal:remove')}
                    onClick={() => setDependencies((items) => items.filter((_, i) => i !== index))}
                  >
                    <X size={14} />
                  </IconButton>
                </div>
              ))}
              <Button
                disabled={!commands.length}
                onClick={() =>
                  setDependencies((items) => [
                    ...items,
                    { commandId: commands[0].id, reuse: 'shared' },
                  ])
                }
              >
                <Plus size={14} />
                {t('terminal:addDependency')}
              </Button>
              {!commands.length && <p className="muted small">{t('terminal:noCommands')}</p>}
            </section>
          )}
        </fieldset>
        {config && <p className="muted small">{t('terminal:configHint')}</p>}
        {error && (
          <p className="form-error" role="alert">
            {errorText(error)}
          </p>
        )}
        <div className="dialog-actions">
          <Button onClick={onClose}>{t('common:cancel')}</Button>
          <Button type="submit" variant="primary" disabled={pending}>
            {pending ? t('common:saving') : t('common:save')}
          </Button>
        </div>
      </form>
    </Dialog>
  );
}
