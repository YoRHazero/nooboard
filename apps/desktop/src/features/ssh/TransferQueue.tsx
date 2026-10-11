import { useState } from 'react';
import {
  ArrowDownToLine,
  ArrowUpFromLine,
  ChevronDown,
  ChevronUp,
  FolderOpen,
  RotateCw,
  X,
} from 'lucide-react';
import { useRemote, useDesktop, useCommand } from '../../desktop/api';
import { useI18n } from '../../i18n/react';
import { IconButton } from '../../ui/controls';
import { errorText, toProblem } from '../../i18n/errors';
const finished = new Set(['Succeeded', 'Failed', 'Cancelled', 'Unconfirmed']);
const bytes = (n: number) =>
  n < 1024
    ? `${n} B`
    : n < 1048576
      ? `${(n / 1024).toFixed(1)} KB`
      : `${(n / 1048576).toFixed(1)} MB`;
export function TransferQueue() {
  const { transfers } = useRemote();
  const desktop = useDesktop();
  const { execute } = useCommand();
  const { t } = useI18n();
  const [open, setOpen] = useState(false);
  const active = transfers.filter((task) => !finished.has(task.state)).length;
  return (
    <section className="ssh-queue">
      <button className="ssh-queue-toggle" aria-expanded={open} onClick={() => setOpen(!open)}>
        {open ? <ChevronDown size={15} /> : <ChevronUp size={15} />}
        <strong>{t('ssh:transfers')}</strong>
        <span>{t('ssh:taskCount', { active, total: transfers.length })}</span>
        <span className="ssh-queue-hint">{t('ssh:independentTasks')}</span>
      </button>
      {open && (
        <div className="ssh-queue-list">
          {!transfers.length && <p>{t('ssh:noTransfers')}</p>}
          {[...transfers].reverse().map((task) => (
            <div className="ssh-task" key={task.id}>
              {task.direction === 'Upload' ? (
                <ArrowUpFromLine size={16} />
              ) : (
                <ArrowDownToLine size={16} />
              )}
              <div className="ssh-task-body">
                <strong title={task.source}>{task.source.split(/[\\/]/).at(-1)}</strong>
                <p title={task.target ?? task.destination}>{task.target ?? task.destination}</p>
                <progress
                  max={task.total ?? Math.max(1, task.transferred)}
                  value={task.transferred}
                  aria-label={t('ssh:progress')}
                />
                {task.error && <p className="ssh-error">{errorText(toProblem(task.error))}</p>}
                {task.state === 'Unconfirmed' && (
                  <p className="ssh-error">
                    {t('ssh:checkTarget')} {task.temporaryPath}
                  </p>
                )}
              </div>
              <div className="ssh-task-state">
                <span>{t(`ssh:transferState${task.state}`)}</span>
                <small>
                  {bytes(task.transferred)}
                  {task.total != null ? ` / ${bytes(task.total)}` : ''}
                </small>
              </div>
              {(task.state === 'Failed' || task.state === 'Cancelled') && (
                <IconButton
                  label={t('ssh:retry')}
                  onClick={() =>
                    void execute(() => desktop.remote({ type: 'retry', data: { id: task.id } }))
                  }
                >
                  <RotateCw size={14} />
                </IconButton>
              )}
              {task.state === 'Succeeded' && task.direction === 'Download' && (
                <IconButton
                  label={t('ssh:reveal')}
                  onClick={() =>
                    void execute(() => desktop.remote({ type: 'reveal', data: { id: task.id } }))
                  }
                >
                  <FolderOpen size={15} />
                </IconButton>
              )}
              <IconButton
                label={t(finished.has(task.state) ? 'ssh:removeTask' : 'ssh:cancelTask')}
                onClick={() =>
                  void execute(() =>
                    desktop.remote({
                      type: finished.has(task.state) ? 'forgetTransfer' : 'cancel',
                      data: { id: task.id },
                    }),
                  )
                }
              >
                <X size={14} />
              </IconButton>
            </div>
          ))}
        </div>
      )}
    </section>
  );
}
