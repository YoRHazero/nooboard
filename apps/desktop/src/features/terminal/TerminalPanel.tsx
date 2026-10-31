import { useTerminalStore } from './store/index';
import { useSnapshot } from '../../shared/queries/useBackendSnapshotQuery';
import { useActionFeedback } from '../../shared/action-feedback';
import { useContinueMutation } from './queries/useContinueMutation';
import { useLifetimeMutation } from './queries/useLifetimeMutation';
import { useStartLocalMutation } from './queries/useStartLocalMutation';
import { Select, SelectItem } from '../../shared/ui/Select';
import { Cable, FolderOpen, Layers, Minus, Plus, TerminalSquare } from 'lucide-react';
import { terminalKey } from './identity';
import { useI18n } from '../../shared/i18n/react';
import { Button, IconButton } from '../../shared/ui/controls';
import { TerminalView } from './TerminalView';
import './terminal.css';

export function TerminalPanel() {
  const startLocalMutation = useStartLocalMutation();
  const lifetimeMutation = useLifetimeMutation();
  const continueMutation = useContinueMutation();

  const workspace = useSnapshot((s) => s.terminal);
  const state = useTerminalStore((state) => state);
  const { t } = useI18n();
  const { execute } = useActionFeedback([startLocalMutation, lifetimeMutation, continueMutation]);
  const toggleFiles = useTerminalStore((state) => state.toggleFiles);
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
  const local = () => execute(() => startLocalMutation.mutateAsync([t('terminal:localShell')]));
  return (
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
              onClick={() => {
                state.clearConfigSelection();
                state.select(item.target);
              }}
            >
              <span className={`terminal-status terminal-status--${item.phase}`} />
              {item.name}
            </button>
            <IconButton
              label={t('terminal:collapsePanel')}
              onClick={() => state.collapse(item.target, workspace)}
            >
              <Minus size={14} />
            </IconButton>
          </div>
        ))}
        <IconButton label={t('terminal:newLocal')} disabled={workspace.closing} onClick={local}>
          <Plus size={16} />
        </IconButton>
      </div>
      {!run && (
        <div className="terminal-run-toolbar">
          <span className="small muted">{t('terminal:noSelection')}</span>
          <IconButton label={t('terminal:files')} disabled>
            <FolderOpen size={16} />
          </IconButton>
        </div>
      )}
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
                        lifetimeMutation.mutateAsync({
                          id: run.target.id,
                          policy: e as 'independent' | 'dependent',
                        }),
                      )
                    }
                  >
                    <SelectItem value="independent">{t('terminal:independent')}</SelectItem>
                    <SelectItem value="dependent">{t('terminal:dependent')}</SelectItem>
                  </Select>
                </>
              )}
              <IconButton
                label={t('terminal:files')}
                disabled={run.target.kind !== 'terminal' || !run.sshTarget}
                aria-pressed={state.filesOpen && !!run.sshTarget && run.target.kind === 'terminal'}
                onClick={toggleFiles}
              >
                <FolderOpen size={16} />
              </IconButton>
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
                      onClick={() => state.select({ kind: 'command', id: dep.runId! })}
                    >
                      {t('terminal:viewOutput')}
                    </Button>
                  )}
                  {dep.gate !== 'released' && dep.gate !== 'failed' && dep.gate !== 'cancelled' && (
                    <Button
                      disabled={!dep.runId}
                      onClick={() => execute(() => continueMutation.mutateAsync({ id: dep.id }))}
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
  );
}
