import { useConfirmHostMutation } from './queries/useConfirmHostMutation';
import { usePasswordMutation } from './queries/usePasswordMutation';
import { useSnapshot } from '../../shared/queries/useBackendSnapshotQuery';
import { useActionFeedback } from '../../shared/action-feedback';
import { useCancelPromptMutation } from './queries/useCancelPromptMutation';
import { useState } from 'react';
import type { TerminalPromptView } from '../../shared/types';
import { useI18n } from '../../shared/i18n/react';
import { Button } from '../../shared/ui/controls';
import { Dialog } from '../../shared/ui/Dialog';

export function AuthenticationPrompt() {
  const prompts = useSnapshot((s) => s.terminal.prompts);
  return prompts[0] ? <Prompt key={prompts[0].id} prompt={prompts[0]} /> : null;
}
function Prompt({ prompt }: { prompt: TerminalPromptView }) {
  const cancelPromptMutation = useCancelPromptMutation();

  const confirmHostMutation = useConfirmHostMutation();
  const passwordMutation = usePasswordMutation();
  const { t } = useI18n();
  const configs = useSnapshot((s) => s.terminal.configs);
  const { execute, pending } = useActionFeedback([
    cancelPromptMutation,
    confirmHostMutation,
    passwordMutation,
  ]);
  const [password, setPassword] = useState('');
  const [remember, setRemember] = useState(false);
  const close = () => {
    if (!pending) void execute(() => cancelPromptMutation.mutateAsync({ id: prompt.id }));
  };
  const host = prompt.prompt.kind === 'host' ? prompt.prompt.data : null;
  return (
    <Dialog title={host ? t('terminal:hostTrust') : t('terminal:authenticate')} onClose={close}>
      <form
        className="terminal-form"
        onSubmit={(e) => {
          e.preventDefault();
          if (!pending)
            void execute(async () => {
              await (host
                ? confirmHostMutation.mutateAsync({ id: prompt.id, accept: true })
                : passwordMutation.mutateAsync({ id: prompt.id, password, remember }));
              setPassword('');
            });
        }}
      >
        <strong>{configs.find((c) => c.id === prompt.targetId)?.data.name}</strong>
        {host ? (
          <>
            <p className="muted small">{t('terminal:hostTrustHint')}</p>
            <p className="mono terminal-break">{host.address}</p>
            <p className="mono terminal-break">
              {host.key.algorithm}
              <br />
              {host.fingerprint}
            </p>
          </>
        ) : (
          <>
            <p className="muted">
              {prompt.prompt.kind === 'password' && prompt.prompt.data.username}
            </p>
            <label className="form-field">
              <span>{t('terminal:password')}</span>
              <input
                type="password"
                autoFocus
                data-autofocus
                autoComplete="current-password"
                value={password}
                disabled={pending}
                onChange={(e) => setPassword(e.target.value)}
              />
            </label>
            <label className="terminal-checkbox">
              <input
                type="checkbox"
                checked={remember}
                disabled={pending}
                onChange={(e) => setRemember(e.target.checked)}
              />
              {t('terminal:rememberPassword')}
            </label>
          </>
        )}
        <div className="dialog-actions">
          <Button onClick={close} disabled={pending}>
            {t('common:cancel')}
          </Button>
          <Button variant="primary" type="submit" disabled={pending}>
            {host ? t('terminal:trust') : t('terminal:authenticate')}
          </Button>
        </div>
      </form>
    </Dialog>
  );
}
