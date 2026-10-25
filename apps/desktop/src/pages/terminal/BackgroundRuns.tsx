import { Cable, ChevronDown, Layers, Square, TerminalSquare } from 'lucide-react';
import { terminalKey, type TerminalRunView } from '../../client/api';
import { useCommand, useDesktop } from '../../client/react';
import { useI18n } from '../../shared/i18n/react';
import { Button, IconButton } from '../../shared/ui/controls';
import './background-runs.css';

export function BackgroundRuns({ runs }: { runs: readonly TerminalRunView[] }) {
  const { terminal } = useDesktop();
  const { execute } = useCommand();
  const { t } = useI18n();
  if (!runs.length) return null;
  return (
    <details className="terminal-background">
      <summary>
        <ChevronDown size={15} className="terminal-background-chevron" />
        <Layers size={15} />
        <strong>{t('terminal:backgroundRuns')}</strong>
        <span className="terminal-background-count">{runs.length}</span>
        <span className="terminal-background-names">{runs.map((run) => run.name).join(' · ')}</span>
      </summary>
      <div className="terminal-background-list">
        {runs.map((run) => {
          const Icon = run.target.kind === 'tunnel' ? Cable : TerminalSquare;
          return (
            <div className="terminal-background-run" key={terminalKey(run.target)}>
              <Icon size={16} />
              <div className="terminal-background-info">
                <strong>{run.name}</strong>
                <span className="small muted">
                  <span className={`terminal-status terminal-status--${run.phase}`} />
                  {t(`terminal:${run.phase}`)}
                </span>
                {run.error && <p className="form-error">{run.error}</p>}
              </div>
              <Button variant="quiet" onClick={() => terminal.select(run.target)}>
                {t('terminal:reopenPanel')}
              </Button>
              <IconButton
                label={t('terminal:stopNamed', { name: run.name })}
                disabled={run.phase === 'stopping'}
                onClick={() =>
                  execute(() => terminal.request({ type: 'stop', data: { target: run.target } }))
                }
              >
                <Square size={12} />
              </IconButton>
            </div>
          );
        })}
      </div>
    </details>
  );
}
