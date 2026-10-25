import { Select, SelectItem } from '../../shared/ui/Select';
import { useState, useSyncExternalStore } from 'react';
import { Cable, FolderOpen, Layers, Minus, Plus, Square, TerminalSquare, X } from 'lucide-react';
import { terminalKey } from '../../client/api';
import { useCommand, useDesktop, useSnapshot } from '../../client/react';
import { useI18n } from '../../shared/i18n/react';
import { Button, IconButton } from '../../shared/ui/controls';
import { ConfigurationPanel } from './ConfigurationPanel';
import { BackgroundRuns } from './BackgroundRuns';
import { TerminalView } from './TerminalView';
import { RemoteFiles } from './RemoteFiles';
import { HostKeyChange } from './HostKeyChange';
import './terminal.css';

export function TerminalPage() {
  const { terminal } = useDesktop();
  const workspace = useSnapshot((s) => s.terminal);
  const state = useSyncExternalStore(terminal.subscribe, terminal.getSnapshot);
  const { t } = useI18n();
  const { execute } = useCommand();
  const [hostChange, setHostChange] = useState(false);
  const [files, setFiles] = useState(false);
  const releaseReasons: Record<string, string> = {
    SuccessfulExit: t('terminal:successfulExit'),
    ReadyMarker: t('terminal:readyMarker'),
    ManualContinue: t('terminal:manualContinue'),
  };
  const collapsed = new Set(state.collapsed);
  const panels = workspace.runs.filter((r) => !collapsed.has(terminalKey(r.target)));
  const background = workspace.runs.filter((r) => collapsed.has(terminalKey(r.target)));
  const run = panels.find(
    (r) => state.selected && terminalKey(r.target) === terminalKey(state.selected),
  );
  const local = () => execute(() => terminal.startLocal(t('terminal:localShell')));
  return (
    <div className="terminal-page">
      <ConfigurationPanel />
      <BackgroundRuns runs={background} />
      {state.notice && (
        <div className="terminal-notice" role="status">
          <span>
            {state.notice === 'lagged' ? (
              t('terminal:lagged')
            ) : (
              <>
                <strong>{state.notice.name}</strong> · {state.notice.error ?? state.notice.result}
                {state.notice.cleanup.length > 0 && (
                  <>
                    <br />
                    {t('terminal:cleanup')}: {state.notice.cleanup.join(', ')}
                  </>
                )}
              </>
            )}
          </span>
          <IconButton label={t('common:dismiss')} onClick={() => terminal.clearNotice()}>
            <X size={14} />
          </IconButton>
        </div>
      )}
      {workspace.issue && (
        <div className="terminal-notice" role="alert">
          <span>
            {workspace.issue.kind === 'hostKeyChanged'
              ? t('terminal:hostChanged')
              : workspace.issue.message}
          </span>
          {workspace.issue.targetId && workspace.issue.received && (
            <Button onClick={() => setHostChange(true)}>{t('terminal:replaceKey')}</Button>
          )}
        </div>
      )}
      {state.pending.map((pending) => (
        <div className="terminal-notice" role="status" key={pending.subscription}>
          <span>
            {pending.name ??
              workspace.configs.find((c) => c.id === pending.target?.id)?.data.name ??
              t('terminal:connecting')}
          </span>
          <Button onClick={() => terminal.cancel(pending.subscription)}>
            {t('terminal:cancelStart')}
          </Button>
        </div>
      ))}
      <div
        className={`terminal-workbench ${files && run?.target.kind === 'terminal' && run.sshTarget ? 'has-files' : ''}`}
      >
        <section className="terminal-stage" aria-label={t('terminal:active')}>
          <div className="terminal-tabs" role="tablist" aria-label={t('terminal:active')}>
            {panels.map((item) => (
              <div
                className={`terminal-tab ${run?.target.id === item.target.id ? 'is-selected' : ''}`}
                key={terminalKey(item.target)}
              >
                <button
                  role="tab"
                  aria-selected={run?.target.id === item.target.id}
                  onClick={() => terminal.select(item.target)}
                >
                  <span className={`terminal-status terminal-status--${item.phase}`} />
                  {item.name}
                </button>
                <IconButton
                  label={
                    item.target.kind === 'terminal'
                      ? t('terminal:closeSession')
                      : t('terminal:collapsePanel')
                  }
                  disabled={item.target.kind === 'terminal' && item.phase === 'stopping'}
                  onClick={() =>
                    item.target.kind === 'terminal'
                      ? execute(() =>
                          terminal.request({ type: 'stop', data: { target: item.target } }),
                        )
                      : terminal.collapse(item.target)
                  }
                >
                  {item.target.kind === 'terminal' ? <X size={13} /> : <Minus size={14} />}
                </IconButton>
              </div>
            ))}
            <IconButton label={t('terminal:newLocal')} disabled={workspace.closing} onClick={local}>
              <Plus size={16} />
            </IconButton>
          </div>
          {run ? (
            <>
              <div className="terminal-run-toolbar">
                <span className="small muted">
                  {t(`terminal:${run.phase}`)}
                  {run.address ? ` · ${run.address}` : ''}
                </span>
                <div>
                  {run.target.kind === 'command' && (
                    <>
                      <Select
                        aria-label={t('terminal:lifetime')}
                        value={run.lifetime ?? 'independent'}
                        disabled={run.phase === 'stopping'}
                        onValueChange={(e) =>
                          execute(() =>
                            terminal.request({
                              type: 'lifetime',
                              data: {
                                id: run.target.id,
                                policy: e as 'independent' | 'dependent',
                              },
                            }),
                          )
                        }
                      >
                        <SelectItem value="independent">{t('terminal:independent')}</SelectItem>
                        <SelectItem value="dependent">{t('terminal:dependent')}</SelectItem>
                      </Select>
                      <IconButton
                        label={t('terminal:stopNamed', { name: run.name })}
                        disabled={run.phase === 'stopping'}
                        onClick={() =>
                          execute(() =>
                            terminal.request({ type: 'stop', data: { target: run.target } }),
                          )
                        }
                      >
                        <Square size={12} />
                      </IconButton>
                    </>
                  )}
                  {run.target.kind === 'terminal' && run.sshTarget && (
                    <IconButton
                      label={t('terminal:files')}
                      onClick={() => setFiles((value) => !value)}
                    >
                      <FolderOpen size={16} />
                    </IconButton>
                  )}
                </div>
              </div>
              {workspace.dependencies
                .filter((dep) => dep.owner.id === run.target.id)
                .map((dep) => (
                  <div className="terminal-dependency" key={dep.id}>
                    <span>
                      {workspace.configs.find((c) => c.id === dep.commandId)?.data.name} ·{' '}
                      {t(`terminal:${dep.gate}`)}
                      {dep.reason && ` · ${releaseReasons[dep.reason] ?? dep.reason}`}
                    </span>
                    <div>
                      {dep.runId && (
                        <Button
                          variant="quiet"
                          onClick={() => terminal.select({ kind: 'command', id: dep.runId! })}
                        >
                          {t('terminal:viewOutput')}
                        </Button>
                      )}
                      {dep.gate !== 'released' &&
                        dep.gate !== 'failed' &&
                        dep.gate !== 'cancelled' && (
                          <Button
                            disabled={!dep.runId}
                            onClick={() =>
                              execute(() =>
                                terminal.request({ type: 'continue', data: { id: dep.id } }),
                              )
                            }
                          >
                            {t('terminal:continue')}
                          </Button>
                        )}
                    </div>
                  </div>
                ))}
              {run.target.kind === 'tunnel' ? (
                <div className="terminal-tunnel">
                  <Cable size={32} />
                  <h2>{run.name}</h2>
                  <code>
                    {run.forward?.listenHost}:{run.forward?.listenPort}
                    {run.forward?.destinationHost
                      ? ` → ${run.forward.destinationHost}:${run.forward.destinationPort}`
                      : ' · SOCKS'}
                  </code>
                  <p>{t('terminal:tunnelHint')}</p>
                  <Button
                    disabled={run.phase === 'stopping'}
                    onClick={() =>
                      execute(() =>
                        terminal.request({ type: 'stop', data: { target: run.target } }),
                      )
                    }
                  >
                    <Square size={13} />
                    {t('terminal:stop')}
                  </Button>
                </div>
              ) : (
                <TerminalView key={terminalKey(run.target)} target={run.target} />
              )}
            </>
          ) : (
            <div className="terminal-empty">
              {background.length ? (
                <Layers size={38} strokeWidth={1.2} />
              ) : (
                <TerminalSquare size={38} strokeWidth={1.2} />
              )}
              <h2>{t(background.length ? 'terminal:backgroundOnly' : 'terminal:noRuns')}</h2>
              <p>{t(background.length ? 'terminal:backgroundHint' : 'terminal:emptyHint')}</p>
              <Button variant="primary" onClick={local}>
                <Plus size={15} />
                {t('terminal:newLocal')}
              </Button>
            </div>
          )}
        </section>
        {files && run?.target.kind === 'terminal' && run.sshTarget && (
          <RemoteFiles key={run.target.id} id={run.target.id} onClose={() => setFiles(false)} />
        )}
      </div>
      {hostChange && workspace.issue?.targetId && workspace.issue.received && (
        <HostKeyChange
          targetId={workspace.issue.targetId}
          previous={workspace.issue.previous}
          received={workspace.issue.received}
          onClose={() => setHostChange(false)}
        />
      )}
    </div>
  );
}
