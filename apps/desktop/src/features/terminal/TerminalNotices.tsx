import { useEffect, useRef, useState } from 'react';
import { toast } from 'sonner';
import { useQueryClient } from '@tanstack/react-query';
import { useSnapshot } from '../../shared/queries/useBackendSnapshotQuery';
import { useI18n } from '../../shared/i18n/react';
import { onTerminalEnded } from '../../shared/events';
import { handleAction } from '../../shared/action-feedback';
import { Button } from '../../shared/ui/controls';
import { useTerminalStore } from './store';
import { getTerminalStreams } from './io/output-stream';
import { HostKeyChange } from './HostKeyChange';
import { useStopFilesMutation } from './queries/useStopFilesMutation';
import { terminalKeys } from './queries/keys';

export function TerminalNotices() {
  const pending = useTerminalStore((state) => state.pending);
  const notice = useTerminalStore((state) => state.notice);
  const clearNotice = useTerminalStore((state) => state.clearNotice);
  const workspace = useSnapshot((snapshot) => snapshot.terminal);
  const session = useSnapshot((snapshot) => snapshot.session);
  const queryClient = useQueryClient();
  const stop = useStopFilesMutation();
  const { t } = useI18n();
  const [hostChange, setHostChange] = useState(false);
  const files = useRef(new Set<string>());
  const ended = useRef(new Set<string>());
  const missing = useRef(new Map<string, ReturnType<typeof setTimeout>>());
  const inputFailures = useRef(new Map<string, string>());
  useEffect(() => {
    missing.current.forEach(clearTimeout);
    missing.current.clear();
    for (const id of files.current) toast.dismiss(`file:${id}`);
    files.current.clear();
    ended.current.clear();
    inputFailures.current.clear();
  }, [session]);
  useEffect(() => {
    const active = new Set(workspace.runs.map((run) => run.target.id));
    for (const id of inputFailures.current.keys()) {
      if (!active.has(id)) inputFailures.current.delete(id);
    }
    for (const run of workspace.runs) {
      if (!run.autoInputError || inputFailures.current.get(run.target.id) === run.autoInputError)
        continue;
      inputFailures.current.set(run.target.id, run.autoInputError);
      toast.error(t('terminal:autoInputFailed', { name: run.name }), {
        id: `auto-input:${run.target.id}`,
        description: run.autoInputError,
        duration: Infinity,
      });
    }
  }, [workspace.runs, t]);
  useEffect(
    () =>
      onTerminalEnded((event) => {
        const failed = event.result === 'failed' || !!event.error || event.cleanup.length > 0;
        const title =
          event.kind === 'file'
            ? t(
                event.result === 'succeeded'
                  ? 'terminal:transferSucceeded'
                  : event.result === 'stopped'
                    ? 'terminal:transferStopped'
                    : 'terminal:transferFailed',
              )
            : t('terminal:instanceEnded', { name: event.name });
        const id = event.kind === 'file' ? `file:${event.id}` : `ended:${event.kind}:${event.id}`;
        const description = [
          event.error,
          event.cleanup.length ? `${t('terminal:cleanup')}: ${event.cleanup.join(', ')}` : null,
        ]
          .filter(Boolean)
          .join('\n');
        (failed ? toast.error : toast.success)(title, {
          id,
          position: event.kind === 'file' ? 'bottom-left' : 'bottom-right',
          description,
          duration: failed ? Infinity : 4500,
          dismissible: true,
          closeButton: true,
          action: null,
        });
        if (event.kind === 'file') {
          ended.current.add(event.id);
          if (ended.current.size > 256) ended.current.delete(ended.current.values().next().value!);
          clearTimeout(missing.current.get(event.id));
          missing.current.delete(event.id);
          files.current.delete(event.id);
          void queryClient.invalidateQueries({ queryKey: terminalKeys.directories });
        }
      }),
    [t, queryClient],
  );
  useEffect(() => {
    for (const file of workspace.files) {
      if (ended.current.has(file.id)) continue;
      clearTimeout(missing.current.get(file.id));
      missing.current.delete(file.id);
      files.current.add(file.id);
      const total = Number(file.totalBytes);
      const value = total > 0 ? Math.min(1, Number(file.bytes) / total) : undefined;
      const operation =
        file.operation === 'upload'
          ? t('terminal:upload')
          : file.operation === 'download'
            ? t('terminal:download')
            : t('terminal:deleteFiles');
      toast.loading(`${operation} · ${t(`terminal:${file.phase}`)}`, {
        id: `file:${file.id}`,
        position: 'bottom-left',
        duration: Infinity,
        closeButton: false,
        dismissible: false,
        description: (
          <div className="file-toast-progress">
            <progress aria-label={operation} max={1} value={value} />
            <span>
              {t('terminal:fileProgress', {
                completed: file.completed,
                total: file.total ?? '—',
                bytes: file.bytes,
              })}
            </span>
          </div>
        ),
        action: (
          <Button
            variant="quiet"
            disabled={file.phase === 'stopping'}
            onClick={() => void handleAction(() => stop.mutateAsync({ id: file.id }))}
          >
            {t('common:cancel')}
          </Button>
        ),
      });
    }
    for (const id of files.current) {
      if (workspace.files.some((file) => file.id === id) || missing.current.has(id)) continue;
      missing.current.set(
        id,
        setTimeout(() => {
          // A removed snapshot is not evidence of success: completion events can be lost on reconnect.
          toast.warning(t('terminal:transferUnknown'), {
            id: `file:${id}`,
            position: 'bottom-left',
            duration: Infinity,
            closeButton: true,
            dismissible: true,
            description: undefined,
            action: null,
          });
          files.current.delete(id);
          missing.current.delete(id);
        }, 1500),
      );
    }
  }, [workspace.files, t]);
  useEffect(
    () => () => {
      missing.current.forEach(clearTimeout);
    },
    [],
  );
  useEffect(() => {
    const starts = pending.filter((item) => !item.attaching);
    const next = new Set(starts.map((item) => item.subscription));
    toast.getToasts().forEach((notification) => {
      if (
        typeof notification.id === 'string' &&
        notification.id.startsWith('start:') &&
        !next.has(notification.id.slice(6))
      )
        toast.dismiss(notification.id);
    });
    const timer = setTimeout(
      () =>
        starts.forEach((item) =>
          toast.loading(
            item.name ??
              workspace.configs.find((c) => c.id === item.target?.id)?.data.name ??
              t('terminal:connecting'),
            {
              id: `start:${item.subscription}`,
              duration: Infinity,
              closeButton: false,
              action: (
                <Button
                  variant="quiet"
                  onClick={() => getTerminalStreams().cancel(item.subscription)}
                >
                  {t('terminal:cancelStart')}
                </Button>
              ),
            },
          ),
        ),
      180,
    );
    return () => clearTimeout(timer);
  }, [pending, workspace.configs, t]);
  useEffect(() => {
    if (notice === 'lagged')
      toast.warning(t('terminal:lagged'), { id: 'terminal-lagged', duration: Infinity });
    if (notice) clearNotice();
  }, [notice, clearNotice, t]);
  useEffect(() => {
    const issue = workspace.issue;
    if (!issue) {
      toast.dismiss('terminal-issue');
      return;
    }
    toast.error(issue.kind === 'hostKeyChanged' ? t('terminal:hostChanged') : issue.message, {
      id: 'terminal-issue',
      duration: Infinity,
      action:
        issue.targetId && issue.received
          ? {
              label: t('terminal:replaceKey'),
              onClick: (event) => {
                event.preventDefault();
                setHostChange(true);
              },
            }
          : undefined,
    });
  }, [workspace.issue, t]);
  return hostChange && workspace.issue?.targetId && workspace.issue.received ? (
    <HostKeyChange
      targetId={workspace.issue.targetId}
      previous={workspace.issue.previous}
      received={workspace.issue.received}
      onClose={() => setHostChange(false)}
    />
  ) : null;
}
