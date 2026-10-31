import { Plus, X } from 'lucide-react';
import { useRef, type Dispatch, type SetStateAction } from 'react';
import { useI18n } from '../../shared/i18n/react';
import { Button, IconButton } from '../../shared/ui/controls';
import { ToggleTip } from '../../shared/ui/ToggleTip';

export interface AutoInputDraft {
  id: string;
  prefix: string;
  saved: boolean;
  value?: string;
}

export function AutoInputFields({
  rules,
  setRules,
}: {
  rules: AutoInputDraft[];
  setRules: Dispatch<SetStateAction<AutoInputDraft[]>>;
}) {
  const { t } = useI18n();
  const newRule = useRef<string | null>(null);
  const update = (id: string, patch: Partial<AutoInputDraft>) =>
    setRules((items) => items.map((item) => (item.id === id ? { ...item, ...patch } : item)));
  return (
    <section className="terminal-prerequisites terminal-auto-inputs">
      <div className="terminal-auto-input-heading">
        <h3>{t('terminal:autoInputs')}</h3>
        <ToggleTip label={t('terminal:autoInputsHelp')}>{t('terminal:autoInputsHint')}</ToggleTip>
      </div>
      <div className="terminal-auto-input-list" role="region" aria-label={t('terminal:autoInputs')}>
        {rules.map((rule) => (
          <div className="terminal-auto-input" key={rule.id}>
            <label className="form-field">
              <span>{t('terminal:autoInputPrefix')}</span>
              <input
                className="mono"
                required
                readOnly={rule.saved}
                spellCheck={false}
                autoComplete="off"
                value={rule.prefix}
                ref={(element) => {
                  element?.setCustomValidity(
                    rules.some((other) => other.id !== rule.id && other.prefix === rule.prefix)
                      ? t('terminal:autoInputDuplicate')
                      : '',
                  );
                  if (element && newRule.current === rule.id) {
                    newRule.current = null;
                    element.focus();
                  }
                }}
                onChange={(event) => update(rule.id, { prefix: event.target.value })}
              />
            </label>
            <label className="form-field">
              <span>{t('terminal:autoInputValue')}</span>
              {rule.value === undefined ? (
                <span className="muted small">{t('terminal:autoInputSaved')}</span>
              ) : (
                <input
                  type="password"
                  autoComplete="new-password"
                  spellCheck={false}
                  value={rule.value}
                  placeholder={t('terminal:autoInputEmpty')}
                  onChange={(event) => update(rule.id, { value: event.target.value })}
                />
              )}
            </label>
            <div className="terminal-auto-input-actions">
              {rule.saved && (
                <Button
                  variant="quiet"
                  onClick={() =>
                    update(rule.id, {
                      value: rule.value === undefined ? '' : undefined,
                    })
                  }
                >
                  {t(
                    rule.value === undefined
                      ? 'terminal:autoInputReplace'
                      : 'terminal:autoInputKeep',
                  )}
                </Button>
              )}
              <IconButton
                label={t('terminal:remove')}
                onClick={() => setRules((items) => items.filter((item) => item.id !== rule.id))}
              >
                <X size={14} />
              </IconButton>
            </div>
          </div>
        ))}
      </div>
      <Button
        onClick={() => {
          const id = crypto.randomUUID();
          newRule.current = id;
          setRules((items) => [...items, { id, prefix: '', saved: false, value: '' }]);
        }}
      >
        <Plus size={14} />
        {t('terminal:addAutoInput')}
      </Button>
    </section>
  );
}
