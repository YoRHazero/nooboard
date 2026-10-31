import { useState } from 'react';
import { useI18n } from '../../shared/i18n/react';
import { ConfigDialog, type ConfigFormProps } from './ConfigDialog';
import { AutoInputFields, type AutoInputDraft } from './AutoInputFields';
export function CommandConfigEditor({ config, sshTarget, ...props }: ConfigFormProps<'command'>) {
  const { t } = useI18n();
  const [command, setCommand] = useState(config?.data.command ?? '');
  const [rules, setRules] = useState<AutoInputDraft[]>(() =>
    (config?.data.autoInputs ?? []).map((prefix) => ({
      id: crypto.randomUUID(),
      prefix,
      saved: true,
    })),
  );
  return (
    <ConfigDialog
      {...props}
      config={config}
      kind="command"
      sshTarget={sshTarget}
      build={(name) => ({
        kind: 'command',
        data: {
          name,
          sshTarget: config?.data.sshTarget ?? sshTarget ?? null,
          command,
          autoInputs: rules.map((rule) => ({ prefix: rule.prefix, value: rule.value ?? null })),
        },
      })}
    >
      <label className="form-field">
        <span>{t('terminal:runCommand')}</span>
        <textarea
          required
          spellCheck={false}
          className="mono"
          rows={4}
          value={command}
          onChange={(event) => setCommand(event.target.value)}
        />
      </label>
      <AutoInputFields rules={rules} setRules={setRules} />
    </ConfigDialog>
  );
}
