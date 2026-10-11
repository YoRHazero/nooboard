import { useEffect, useState, type CSSProperties } from 'react';
import {
  PanelRightClose,
  PanelRightOpen,
  Plus,
  ShieldCheck,
  TerminalSquare,
  X,
} from 'lucide-react';
import { useDesktop, useRemote, useSnapshot, useCommand } from '../../desktop/api';
import { useI18n } from '../../i18n/react';
import { Button, IconButton } from '../../ui/controls';
import { Dialog } from '../../ui/Dialog';
import { Login } from './Login';
import { TerminalPane } from './TerminalPane';
import { FileTree } from './FileTree';
import { TransferQueue } from './TransferQueue';
import { errorText, toProblem } from '../../i18n/errors';
import './ssh.css';
export function SshPage() {
  const desktop = useDesktop();
  const remote = useRemote();
  const { peers } = useSnapshot();
  const { execute, pending } = useCommand();
  const { t } = useI18n();
  const [active, setActive] = useState<string | undefined>(() => remote.workspaces[0]?.id);
  const [device, setDevice] = useState(() => peers[0]?.noobId ?? '');
  const [files, setFiles] = useState(true);
  const [width, setWidth] = useState(330);
  const [selection, setSelection] = useState('');
  const [login, setLogin] = useState(false);
  const workspace = remote.workspaces.find((w) => w.id === active);
  const challenge = remote.challenges[0];
  useEffect(() => {
    if (active && !remote.workspaces.some((w) => w.id === active)) setActive(undefined);
  }, [active, remote.workspaces]);
  useEffect(() => {
    setSelection('');
    setLogin(false);
  }, [active]);
  useEffect(() => {
    if (!peers.some((p) => p.noobId === device)) setDevice(peers[0]?.noobId ?? '');
  }, [peers, device]);
  const connected = (id: string) => {
    setActive(id);
    setLogin(false);
  };
  return (
    <div className="ssh-workspace" style={{ '--ssh-tree-width': `${width}px` } as CSSProperties}>
      <div className="ssh-top">
        <div className="ssh-tabs" role="tablist" aria-label={t('ssh:sessions')}>
          {remote.workspaces.map((w) => (
            <div className={`ssh-tab ${active === w.id ? 'is-active' : ''}`} key={w.id}>
              <button
                role="tab"
                title={`${w.username}@${w.deviceId}:${w.port}`}
                aria-selected={active === w.id}
                onClick={() => setActive(w.id)}
              >
                <span className={`ssh-dot ${w.terminal.state === 'Ready' ? 'is-ready' : ''}`} />
                {peers.find((p) => p.noobId === w.deviceId)?.deviceName ?? w.deviceId}
                <small>{w.username}</small>
              </button>
              <IconButton
                label={t('ssh:closeSession')}
                onClick={() =>
                  void execute(() => desktop.remote({ type: 'close', data: { workspace: w.id } }))
                }
              >
                <X size={12} />
              </IconButton>
            </div>
          ))}
          <IconButton label={t('ssh:newSession')} onClick={() => setActive(undefined)}>
            <Plus size={17} />
          </IconButton>
        </div>
        <IconButton
          label={t(files ? 'ssh:hideFiles' : 'ssh:showFiles')}
          disabled={!workspace}
          onClick={() => setFiles(!files)}
        >
          {files ? <PanelRightClose size={17} /> : <PanelRightOpen size={17} />}
        </IconButton>
      </div>
      <div className="ssh-session-bar">
        {workspace ? (
          <>
            <span className="mono">
              {workspace.username}@
              {peers.find((p) => p.noobId === workspace.deviceId)?.deviceName ?? workspace.deviceId}
              :{workspace.port}
            </span>
            <span
              className={`ssh-status ${workspace.terminal.state === 'Ready' ? 'is-ready' : ''}`}
            >
              {t(`ssh:phase${workspace.terminal.state}`)}
            </span>
            {workspace.address && <span className="muted mono">{workspace.address}</span>}
            {workspace.terminal.state === 'Ready' && workspace.files.state === 'Failed' && (
              <Button variant="quiet" onClick={() => setLogin(true)}>
                {t('ssh:reconnectFiles')}
              </Button>
            )}{' '}
          </>
        ) : (
          <>
            <label htmlFor="ssh-device">{t('ssh:device')}</label>
            <select id="ssh-device" value={device} onChange={(e) => setDevice(e.target.value)}>
              {peers.map((p) => (
                <option key={p.noobId} value={p.noobId}>
                  {p.deviceName} · {p.noobId.slice(0, 8)}
                </option>
              ))}
            </select>
            <span className="muted">{t('ssh:pairedOnly')}</span>
          </>
        )}
      </div>
      {workspace?.error && (
        <div className="ssh-inline-error" role="alert">
          {errorText(toProblem(workspace.error))}
        </div>
      )}
      <div className={`ssh-body ${files && workspace ? 'has-files' : ''}`}>
        <div className="ssh-terminal-area">
          {workspace ? (
            <>
              {(workspace.terminal.state === 'Ready' || workspace.terminal.state === 'Ended') && (
                <TerminalPane
                  key={workspace.terminalId}
                  workspace={workspace.id}
                  session={workspace.terminalId}
                  onSelection={setSelection}
                />
              )}
              {workspace.terminal.state === 'Connecting' && (
                <div className="ssh-connecting">
                  <TerminalSquare size={28} />
                  <p>{t(challenge ? 'ssh:waitingTrust' : 'ssh:connecting')}</p>
                </div>
              )}
              {(workspace.terminal.state === 'Failed' || workspace.terminal.state === 'Closed') && (
                <Login
                  key={`${workspace.terminalId}:${workspace.terminal.state}`}
                  device={workspace.deviceId}
                  workspace={workspace}
                  onConnected={connected}
                />
              )}{' '}
              {workspace.terminal.state === 'Ended' && (
                <div className="ssh-ended">
                  <span>{t('ssh:shellEnded')}</span>
                  <Button onClick={() => setLogin(true)}>{t('ssh:reconnect')}</Button>
                </div>
              )}
            </>
          ) : peers.length ? (
            <Login key={device} device={device} onConnected={connected} />
          ) : (
            <div className="ssh-connecting">
              <TerminalSquare size={32} />
              <h2>{t('ssh:noDevices')}</h2>
              <p>{t('ssh:pairFirst')}</p>
            </div>
          )}
        </div>
        {workspace && files && (
          <>
            <div
              className="ssh-divider"
              role="separator"
              aria-label={t('ssh:resizeFiles')}
              aria-orientation="vertical"
              aria-valuenow={width}
              aria-valuemin={260}
              aria-valuemax={560}
              tabIndex={0}
              onKeyDown={(e) => {
                if (e.key === 'ArrowLeft') {
                  e.preventDefault();
                  setWidth((v) => Math.min(560, v + 20));
                }
                if (e.key === 'ArrowRight') {
                  e.preventDefault();
                  setWidth((v) => Math.max(260, v - 20));
                }
              }}
              onPointerDown={(e) => {
                e.currentTarget.setPointerCapture(e.pointerId);
              }}
              onPointerMove={(e) => {
                if (e.currentTarget.hasPointerCapture(e.pointerId))
                  setWidth(
                    Math.min(
                      560,
                      Math.max(
                        260,
                        e.currentTarget.parentElement!.getBoundingClientRect().right - e.clientX,
                      ),
                    ),
                  );
              }}
            />
            <FileTree key={workspace.filesId} workspace={workspace} selectedText={selection} />
          </>
        )}
      </div>
      <TransferQueue />
      {login && workspace && (
        <Dialog title={t('ssh:reconnect')} onClose={() => setLogin(false)}>
          <Login workspace={workspace} device={workspace.deviceId} onConnected={connected} />
        </Dialog>
      )}
      {challenge && (
        <Dialog
          title={t('ssh:hostKeyTitle')}
          onClose={() =>
            void execute(() =>
              desktop.remote({ type: 'hostKey', data: { id: challenge.id, accept: false } }),
            )
          }
        >
          <div className="ssh-trust">
            <ShieldCheck size={28} />
            <p>{t('ssh:hostKeyHint')}</p>
            <dl>
              <dt>{t('ssh:address')}</dt>
              <dd className="mono">{challenge.address}</dd>
              <dt>{t('ssh:algorithm')}</dt>
              <dd>{challenge.algorithm}</dd>
              <dt>{t('ssh:fingerprint')}</dt>
              <dd className="mono">{challenge.fingerprint}</dd>
            </dl>
            <div>
              <Button
                disabled={pending}
                onClick={() =>
                  void execute(() =>
                    desktop.remote({ type: 'hostKey', data: { id: challenge.id, accept: false } }),
                  )
                }
              >
                {t('ssh:reject')}
              </Button>
              <Button
                variant="primary"
                disabled={pending}
                onClick={() =>
                  void execute(() =>
                    desktop.remote({ type: 'hostKey', data: { id: challenge.id, accept: true } }),
                  )
                }
              >
                {t('ssh:trust')}
              </Button>
            </div>
          </div>
        </Dialog>
      )}
    </div>
  );
}
