import { useState, type ReactNode } from 'react';
import { toast } from 'sonner';
import type {
  TerminalConfigInput,
  TerminalConfigView,
  TerminalResourceKind,
} from '../../shared/types';
import { useSnapshot } from '../../shared/queries/useBackendSnapshotQuery';
import { useI18n } from '../../shared/i18n/react';
import { useActionFeedback } from '../../shared/action-feedback';
import { Dialog } from '../../shared/ui/Dialog';
import { Button } from '../../shared/ui/controls';
import { useSaveConfigMutation } from './queries/useSaveConfigMutation';
import { useTerminalStore } from './store';
import { configUsage } from './configuration';
export interface ConfigFormProps<K extends TerminalResourceKind> {
  config?: Extract<TerminalConfigView, { kind: K }>;
  sshTarget?: string;
  onClose(): void;
  onSaved?(id: string): void;
}
export function ConfigDialog({
  config,
  kind,
  sshTarget,
  onClose,
  onSaved,
  children,
  build,
}: {
  config?: TerminalConfigView;
  kind: TerminalResourceKind;
  sshTarget?: string;
  onClose(): void;
  onSaved?(id: string): void;
  children: ReactNode;
  build(name: string): TerminalConfigInput;
}) {
  const mutation = useSaveConfigMutation();
  const workspace = useSnapshot((s) => s.terminal);
  const starts = useTerminalStore((s) => s.pending);
  const { t } = useI18n();
  const { execute, pending } = useActionFeedback([mutation]);
  const [name, setName] = useState(config?.data.name ?? '');
  const locked = !!config && configUsage(config.id, workspace, starts).locked;
  const target = config && config.kind !== 'ssh' ? config.data.sshTarget : sshTarget;
  return (
    <Dialog
      className={kind === 'command' ? 'terminal-command-dialog' : undefined}
      title={t(config ? 'terminal:editConfigTitle' : 'terminal:newConfigTitle', {
        kind: t(`terminal:${kind}`),
      })}
      onClose={onClose}
    >
      <form
        className="terminal-form"
        onSubmit={(event) => {
          event.preventDefault();
          if (pending || locked) return;
          void execute(async () => {
            const reply = await mutation.mutateAsync({
              id: config?.id,
              config: build(name.trim()),
            });
            if (reply.type === 'terminalSaved') onSaved?.(reply.data.id);
            toast.success(t('terminal:configSaved'));
            onClose();
          });
        }}
      >
        {kind !== 'ssh' && (
          <p className="muted small">
            {t('terminal:targetLabel')}:{' '}
            {workspace.configs.find((item) => item.id === target)?.data.name ?? t('terminal:local')}
          </p>
        )}
        {locked && <p className="muted small">{t('terminal:configLocked')}</p>}
        <fieldset disabled={pending || locked}>
          <label className="form-field">
            <span>{t('terminal:name')}</span>
            <input
              required
              pattern=".*\S.*"
              autoFocus
              data-autofocus
              value={name}
              onChange={(event) => setName(event.target.value)}
            />
          </label>
          {children}
        </fieldset>
        <div className="dialog-actions">
          <Button onClick={onClose}>{t('common:cancel')}</Button>
          <Button type="submit" variant="primary" disabled={pending || locked}>
            {pending ? t('common:saving') : t('common:save')}
          </Button>
        </div>
      </form>
    </Dialog>
  );
}
