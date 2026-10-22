import { useRef, useState } from 'react';
import { KeyRound, TerminalSquare } from 'lucide-react';
import {
  useDesktop,
  useRemote,
  useSnapshot,
  useCommand,
  type RemoteWorkspace,
} from '../../desktop/api';
import { useI18n } from '../../i18n/react';
import { Button } from '../../ui/controls';
import { errorText, toProblem } from '../../i18n/errors';

export function Login({
  device,
  workspace,
  onConnected,
}: {
  device: string;
  workspace?: RemoteWorkspace;
  onConnected: (id: string) => void;
}) {
  const desktop = useDesktop();
  const remote = useRemote();
  const { peers } = useSnapshot();
  const { t } = useI18n();
  const profile = remote.profiles.find(
    (p) => p.deviceId === device && (!workspace || p.id === workspace.profileId),
  );
  const [username, setUsername] = useState(workspace?.username ?? profile?.username ?? '');
  const [port, setPort] = useState(String(workspace?.port ?? profile?.port ?? 22));
  const [password, setPassword] = useState('');
  const [remember, setRemember] = useState(false);
  const [error, setError] = useState<string>();
  const { execute, pending } = useCommand();
  const submitting = useRef(false);
  const saved = remote.profiles.find(
    (p) =>
      p.deviceId === device && p.username === username && p.port === Number(port) && p.remembered,
  );
  const connect = async (stored = false) => {
    if (submitting.current) return;
    submitting.current = true;
    setError(undefined);
    const secret = stored ? undefined : password;
    setPassword('');
    setRemember(false);
    try {
      const result = await desktop.remote({
        type: 'connect',
        data: {
          workspace:
            workspace?.username === username && workspace.port === Number(port)
              ? workspace.id
              : undefined,
          device,
          username,
          port: Number(port),
          password: secret,
          remember: stored || remember,
        },
      });
      if (result.type === 'connected') onConnected(result.data.id);
    } catch (reason) {
      setError(errorText(toProblem(reason)));
    } finally {
      submitting.current = false;
    }
  };
  const failure = workspace?.terminal.error ?? workspace?.files.error;
  return (
    <div className="ssh-login">
      <div className="ssh-login__heading">
        <TerminalSquare size={26} />
        <h2>
          {t(workspace ? 'ssh:reconnect' : 'ssh:connectTo', {
            device: peers.find((p) => p.noobId === device)?.deviceName ?? device,
          })}
        </h2>
        <p>{t('ssh:loginHint')}</p>
      </div>
      <form
        aria-busy={pending}
        onSubmit={(event) => {
          event.preventDefault();
          void execute(() => connect());
        }}
      >
        <label>
          {t('ssh:username')}
          <input
            autoComplete="username"
            autoFocus
            required
            value={username}

            maxLength={256}
            onChange={(e) => setUsername(e.target.value)}
          />
        </label>
        <label>
          {t('ssh:port')}
          <input
            type="number"
            min={1}
            max={65535}
            required
            value={port}

            onChange={(e) => setPort(e.target.value)}
          />
        </label>
        <label className="ssh-login__wide">
          {t('ssh:password')}
          <input
            type="password"
            autoComplete="current-password"
            required
            maxLength={4096}
            value={password}
            onChange={(e) => setPassword(e.target.value)}
          />
        </label>
        <label className="ssh-check ssh-login__wide">
          <input
            type="checkbox"
            checked={remember}
            onChange={(e) => setRemember(e.target.checked)}
          />
          {t('ssh:remember')}
        </label>
        {(error || failure) && (
          <p className="ssh-error ssh-login__wide" role="alert">
            {error ?? errorText(toProblem(failure))}
          </p>
        )}
        <div className="ssh-login__actions ssh-login__wide">
          <Button type="submit" variant="primary" disabled={pending || !device}>
            {t(pending ? 'ssh:connecting' : 'ssh:connect')}
          </Button>
          {saved && (
            <Button disabled={pending} onClick={() => void execute(() => connect(true))}>
              <KeyRound size={14} />
              {t('ssh:oneClick')}
            </Button>
          )}
        </div>
        {failure?.code === 'sshHostKeyChanged' && (
          <Button
            className="ssh-login__wide"
            variant="danger"
            onClick={() => {
              if (window.confirm(t('ssh:resetTrustConfirm')))
                void execute(() =>
                  desktop.remote({ type: 'forgetHostKey', data: { device, port: Number(port) } }),
                );
            }}
          >
            {t('ssh:resetTrust')}
          </Button>
        )}
      </form>
    </div>
  );
}
