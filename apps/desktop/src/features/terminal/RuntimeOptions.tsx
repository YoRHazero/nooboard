import { useSnapshot } from '../../shared/queries/useBackendSnapshotQuery';
import { useActionFeedback } from '../../shared/action-feedback';
import { useOptionsMutation } from './queries/useOptionsMutation';
import { useState } from 'react';
import { useI18n } from '../../shared/i18n/react';
import { Dialog } from '../../shared/ui/Dialog';
import { Button } from '../../shared/ui/controls';

export function RuntimeOptions({ onClose }: { onClose: () => void }) {
  const optionsMutation = useOptionsMutation();

  const initial = useSnapshot((s) => s.terminal.options);
  const [options, setOptions] = useState(initial);
  const { t } = useI18n();
  const { execute, pending } = useActionFeedback([optionsMutation]);
  return (
    <Dialog title={t('terminal:options')} onClose={onClose}>
      <form
        className="terminal-form"
        onSubmit={(e) => {
          e.preventDefault();
          if (!pending)
            void execute(async () => {
              await optionsMutation.mutateAsync({ options });
              onClose();
            });
        }}
      >
        <p className="muted small">{t('terminal:optionsHint')}</p>
        <div className="terminal-form-grid">
          {(Object.keys(options) as (keyof typeof options)[]).map((key) => (
            <label className="form-field" key={key}>
              <span>{t(`terminal:${key}`)}</span>
              <input
                required
                pattern="[0-9]+"
                inputMode="numeric"
                value={options[key]}
                onChange={(e) => setOptions((old) => ({ ...old, [key]: e.target.value }))}
                disabled={pending}
              />
            </label>
          ))}
        </div>
        <div className="dialog-actions">
          <Button onClick={onClose}>{t('common:cancel')}</Button>
          <Button type="submit" variant="primary" disabled={pending}>
            {t('common:save')}
          </Button>
        </div>
      </form>
    </Dialog>
  );
}
