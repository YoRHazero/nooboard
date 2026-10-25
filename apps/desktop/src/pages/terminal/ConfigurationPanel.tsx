import { useId, useState } from 'react';
import {
  Cable,
  ChevronDown,
  Command,
  Copy,
  Pencil,
  Play,
  Plus,
  Settings2,
  TerminalSquare,
  Trash2,
} from 'lucide-react';
import type {
  TerminalConfigView,
  TerminalResourceKind,
  TerminalReuse,
  TerminalTarget,
} from '../../client/api';
import { useCommand, useDesktop, useSnapshot } from '../../client/react';
import { useI18n } from '../../shared/i18n/react';
import { errorText } from '../../shared/i18n/errors';
import { Button, IconButton } from '../../shared/ui/controls';
import { Dialog } from '../../shared/ui/Dialog';
import { Select, SelectItem } from '../../shared/ui/Select';
import { ConfigEditor } from './ConfigEditor';
import { RuntimeOptions } from './RuntimeOptions';
import './configuration-panel.css';

const icons = { terminal: TerminalSquare, command: Command, tunnel: Cable, ssh: TerminalSquare };
export function ConfigurationPanel() {
  const { terminal, mode } = useDesktop();
  const configs = useSnapshot((s) => s.terminal.configs);
  const closing = useSnapshot((s) => s.terminal.closing);
  const runCount = useSnapshot((s) => s.terminal.runs.length);
  const { t } = useI18n();
  const { execute, pending, error } = useCommand();
  const panelId = useId();
  const [expanded, setExpanded] = useState(false);
  const [selected, setSelected] = useState<string | null>(null);
  const [reuse, setReuse] = useState<TerminalReuse>('shared');
  const [editor, setEditor] = useState<{
    config?: TerminalConfigView;
    kind?: TerminalResourceKind;
    sshTarget?: string;
  } | null>(null);
  const [deleting, setDeleting] = useState<TerminalConfigView | null>(null);
  const [options, setOptions] = useState(false);
  const config = configs.find((item) => item.id === selected) ?? configs[0];
  const target = (config: TerminalConfigView): TerminalTarget => ({
    kind: config.kind,
    id: config.id,
  });
  const start = (config: TerminalConfigView) =>
    execute(() => terminal.start(target(config), reuse));
  const detail =
    config?.kind === 'ssh'
      ? `${config.data.username}@${config.data.address.value}:${config.data.port}`
      : config?.kind === 'command'
        ? config.data.command
        : (configs.find((item) => item.id === config?.data.sshTarget)?.data.name ??
          t('terminal:local'));

  return (
    <>
      <section className="terminal-config-panel" aria-label={t('terminal:configurations')}>
        <header className="terminal-config-header">
          <button
            type="button"
            className="terminal-config-toggle"
            aria-expanded={expanded}
            aria-controls={panelId}
            aria-label={
              expanded ? t('terminal:collapseConfigurations') : t('terminal:expandConfigurations')
            }
            onClick={() => setExpanded((value) => !value)}
          >
            <ChevronDown size={16} />
            <span>{t('terminal:configurations')}</span>
            <span className="terminal-config-count">{configs.length}</span>
          </button>
          <p className="terminal-config-status muted small">
            {mode === 'preview' ? t('terminal:preview') : `${runCount} · ${t('terminal:active')}`}
          </p>
          <div className="terminal-config-tools">
            <Button onClick={() => setEditor({})}>
              <Plus size={15} />
              {t('terminal:new')}
            </Button>
            <IconButton label={t('terminal:options')} onClick={() => setOptions(true)}>
              <Settings2 size={17} />
            </IconButton>
          </div>
        </header>
        <div id={panelId} className="terminal-config-body" hidden={!expanded}>
          <div className="terminal-config-list">
            {(['terminal', 'command', 'tunnel', 'ssh'] as const).map((kind) => {
              const items = configs.filter((item) => item.kind === kind);
              if (!items.length) return null;
              const Icon = icons[kind];
              return (
                <section key={kind}>
                  <h3>{t(`terminal:${kind}`)}</h3>
                  {items.map((item) => (
                    <button
                      type="button"
                      key={item.id}
                      className={`terminal-config ${config?.id === item.id ? 'is-selected' : ''}`}
                      aria-pressed={config?.id === item.id}
                      onClick={() => setSelected(item.id)}
                      onDoubleClick={() => {
                        if (item.kind !== 'ssh') void start(item);
                      }}
                    >
                      <Icon size={15} />
                      <span>{item.data.name}</span>
                    </button>
                  ))}
                </section>
              );
            })}
            {!configs.length && <p className="muted small">{t('terminal:noConfigs')}</p>}
          </div>
          {config && (
            <div className="terminal-config-detail">
              <div className="terminal-config-summary">
                <strong>{config.data.name}</strong>
                <p className="muted small" title={detail}>
                  {detail}
                </p>
              </div>
              <div className="terminal-config-actions">
                {config.kind === 'command' && (
                  <Select
                    aria-label={t('terminal:reuse')}
                    value={reuse}
                    onValueChange={(value) => setReuse(value as TerminalReuse)}
                  >
                    <SelectItem value="shared">{t('terminal:shared')}</SelectItem>
                    <SelectItem value="alwaysNew">{t('terminal:alwaysNew')}</SelectItem>
                  </Select>
                )}
                {config.kind === 'ssh' ? (
                  <Button onClick={() => setEditor({ kind: 'terminal', sshTarget: config.id })}>
                    <Plus size={14} />
                    {t('terminal:terminal')}
                  </Button>
                ) : (
                  <Button variant="primary" disabled={closing} onClick={() => start(config)}>
                    <Play size={14} />
                    {t('terminal:start')}
                  </Button>
                )}
                <IconButton label={t('terminal:edit')} onClick={() => setEditor({ config })}>
                  <Pencil size={15} />
                </IconButton>
                <IconButton
                  label={t('terminal:copy')}
                  onClick={() =>
                    execute(() =>
                      terminal.request({ type: 'copyConfig', data: { target: target(config) } }),
                    )
                  }
                >
                  <Copy size={15} />
                </IconButton>
                <IconButton label={t('terminal:delete')} onClick={() => setDeleting(config)}>
                  <Trash2 size={15} />
                </IconButton>
              </div>
            </div>
          )}
        </div>
      </section>
      {editor && (
        <ConfigEditor
          config={editor.config}
          initialKind={editor.kind}
          initialSshTarget={editor.sshTarget}
          onClose={() => setEditor(null)}
        />
      )}
      {options && <RuntimeOptions onClose={() => setOptions(false)} />}
      {deleting && (
        <Dialog title={t('terminal:delete')} onClose={() => setDeleting(null)}>
          <p>{t('terminal:deleteQuestion', { name: deleting.data.name })}</p>
          {error && (
            <p className="form-error" role="alert">
              {errorText(error)}
            </p>
          )}
          <div className="dialog-actions">
            <Button onClick={() => setDeleting(null)}>{t('common:cancel')}</Button>
            <Button
              variant="danger"
              disabled={pending}
              onClick={() =>
                execute(async () => {
                  await terminal.request({
                    type: 'deleteConfig',
                    data: { target: target(deleting) },
                  });
                  setDeleting(null);
                })
              }
            >
              {t('terminal:delete')}
            </Button>
          </div>
        </Dialog>
      )}
    </>
  );
}
