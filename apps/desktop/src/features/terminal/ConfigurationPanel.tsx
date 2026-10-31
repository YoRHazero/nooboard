import { useState, type ReactNode, type KeyboardEvent } from 'react';
import { toast } from 'sonner';
import {
  Cable,
  ChevronRight,
  Command,
  Copy,
  Laptop,
  MoreHorizontal,
  Pencil,
  Play,
  Plus,
  Server,
  Settings2,
  Square,
  TerminalSquare,
  Trash2,
} from 'lucide-react';
import { useSnapshot } from '../../shared/queries/useBackendSnapshotQuery';
import { useActionFeedback } from '../../shared/action-feedback';
import { useI18n } from '../../shared/i18n/react';
import { Button, IconButton } from '../../shared/ui/controls';
import { Menu, type MenuAction } from '../../shared/ui/Menu';
import { Dialog } from '../../shared/ui/Dialog';
import type { TerminalConfigView, TerminalResourceKind, TerminalRunView } from '../../shared/types';
import { useDeleteConfigMutation } from './queries/useDeleteConfigMutation';
import { useCopyConfigMutation } from './queries/useCopyConfigMutation';
import { useStartMutation } from './queries/useStartMutation';
import { useStartLocalMutation } from './queries/useStartLocalMutation';
import { useStopMutation } from './queries/useStopMutation';
import { useTerminalStore } from './store';
import { configParent, configUsage, LOCAL_SHELL, LOCAL_TARGET } from './configuration';
import { terminalKey } from './identity';
import { ConfigEditor } from './ConfigEditor';
import { RuntimeOptions } from './RuntimeOptions';
import './configuration-panel.css';

const icons = { terminal: TerminalSquare, command: Command, tunnel: Cable, ssh: Server };
type Editor = { id: string } | { kind: TerminalResourceKind; sshTarget?: string };
function TreeRow({
  label,
  icon,
  children,
  detail,
  selected,
  open,
  onToggle,
  onSelect,
  actions,
}: {
  label: string;
  icon: ReactNode;
  children?: ReactNode;
  detail?: string;
  selected?: boolean;
  open?: boolean;
  onToggle?: () => void;
  onSelect?: () => void;
  actions?: ReactNode;
}) {
  const key = (event: KeyboardEvent<HTMLDivElement>) => {
    if (event.target !== event.currentTarget) return;
    if (
      (event.key === 'ArrowRight' && open === false) ||
      (event.key === 'ArrowLeft' && open === true)
    ) {
      event.preventDefault();
      onToggle?.();
    }
    if (event.key === 'Enter' || event.key === ' ') {
      event.preventDefault();
      onSelect?.();
    }
    if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
      event.preventDefault();
      const rows = [
        ...event.currentTarget
          .closest('[role=tree]')!
          .querySelectorAll<HTMLElement>('[role=treeitem]'),
      ];
      rows[rows.indexOf(event.currentTarget) + (event.key === 'ArrowDown' ? 1 : -1)]?.focus();
    }
  };
  return (
    <li role="none">
      <div
        className={`configuration-row ${selected ? 'is-selected' : ''}`}
        role="treeitem"
        tabIndex={0}
        aria-label={label}
        aria-expanded={onToggle ? open : undefined}
        aria-selected={!!selected}
        onKeyDown={key}
        onClick={onSelect}
      >
        {onToggle ? (
          <button
            className="tree-chevron"
            tabIndex={-1}
            aria-label={label}
            aria-expanded={open}
            onClick={(event) => {
              event.stopPropagation();
              onToggle();
            }}
          >
            <ChevronRight size={14} />
          </button>
        ) : (
          <span className="tree-chevron" />
        )}
        <span className="tree-icon">{icon}</span>
        <span className="tree-name">{label}</span>
        <span className="tree-detail">{detail}</span>
        <span className="tree-actions" onClick={(event) => event.stopPropagation()}>
          {actions}
        </span>
      </div>
      {open && children && <ul role="group">{children}</ul>}
    </li>
  );
}
export function ConfigurationPanel() {
  const workspace = useSnapshot((s) => s.terminal);
  const ui = useTerminalStore((s) => s);
  const start = useStartMutation();
  const startLocal = useStartLocalMutation();
  const copy = useCopyConfigMutation();
  const remove = useDeleteConfigMutation();
  const stop = useStopMutation();
  const { execute, pending } = useActionFeedback([start, startLocal, copy, remove, stop]);
  const { t } = useI18n();
  const [editor, setEditor] = useState<Editor | null>(null);
  const [deleting, setDeleting] = useState<TerminalConfigView | null>(null);
  const [options, setOptions] = useState(false);
  const opened = (id: string) => !ui.foldedNodes.includes(id);
  const configs = workspace.configs;
  const configActions = (config: TerminalConfigView): MenuAction[] => {
    const usage = configUsage(config.id, workspace, ui.pending);
    const reason = usage.active
      ? t('terminal:lockedByInstance')
      : usage.referrers.length
        ? t('terminal:lockedByReference', {
            names: usage.referrers.map((c) => c.data.name).join(', '),
          })
        : undefined;
    return [
      {
        label: t('terminal:edit'),
        icon: <Pencil size={14} />,
        disabled: usage.locked,
        reason,
        onSelect: () => setEditor({ id: config.id }),
      },
      {
        label: t('terminal:copy'),
        icon: <Copy size={14} />,
        disabled: copy.isPending,
        onSelect: () => {
          void execute(async () => {
            const reply = await copy.mutateAsync({ target: { kind: config.kind, id: config.id } });
            if (reply.type === 'terminalSaved') {
              ui.revealConfig(reply.data.id, configParent(config));
              setEditor({ id: reply.data.id });
              toast.success(t('terminal:cloned'));
            }
          });
        },
      },
      ...(config.kind === 'command'
        ? [
            {
              label: t('terminal:startNewInstance'),
              icon: <Play size={14} />,
              disabled: workspace.closing,
              onSelect: () => {
                void execute(() =>
                  start.mutateAsync([{ kind: config.kind, id: config.id }, 'alwaysNew']),
                );
              },
            },
          ]
        : []),
      {
        label: t('terminal:delete'),
        icon: <Trash2 size={14} />,
        disabled: usage.locked,
        reason,
        danger: true,
        onSelect: () => setDeleting(config),
      },
    ];
  };
  const more = (config: TerminalConfigView) => (
    <Menu
      label={config.data.name}
      trigger={
        <IconButton label={t('terminal:configActions', { name: config.data.name })}>
          <MoreHorizontal size={16} />
        </IconButton>
      }
      actions={configActions(config)}
    />
  );
  const add = (target: string) => (
    <Menu
      trigger={
        <IconButton label={t('terminal:addConfig')}>
          <Plus size={16} />
        </IconButton>
      }
      actions={(
        ['terminal', 'command', ...(target === LOCAL_TARGET ? [] : ['tunnel'])] as const
      ).map((kind) => ({
        label: t(`terminal:${kind as 'terminal' | 'command' | 'tunnel'}`),
        onSelect: () =>
          setEditor({
            kind: kind as TerminalResourceKind,
            sshTarget: target === LOCAL_TARGET ? undefined : target,
          }),
      }))}
    />
  );
  const runRow = (run: TerminalRunView) => (
    <TreeRow
      key={terminalKey(run.target)}
      label={run.name}
      icon={<span className={`terminal-status terminal-status--${run.phase}`} />}
      selected={
        !ui.selectedConfig && !!ui.selected && terminalKey(ui.selected) === terminalKey(run.target)
      }
      detail={`${t(`terminal:${run.phase}`)}${ui.collapsed.includes(terminalKey(run.target)) ? ` · ${t('terminal:minimized')}` : ''}`}
      onSelect={() => {
        ui.clearConfigSelection();
        ui.select(run.target);
      }}
      actions={
        <IconButton
          label={t('terminal:stopNamed', { name: run.name })}
          disabled={run.phase === 'stopping' || stop.isPending}
          onClick={() => void execute(() => stop.mutateAsync({ target: run.target }))}
        >
          <Square size={12} />
        </IconButton>
      }
    />
  );
  const configRow = (config: TerminalConfigView) => {
    const Icon = icons[config.kind];
    const runs = workspace.runs.filter((r) => r.configId === config.id);
    return (
      <TreeRow
        key={config.id}
        label={config.data.name}
        icon={<Icon size={15} />}
        selected={ui.selectedConfig === config.id}
        detail={`${t(`terminal:${config.kind}`)}${runs.length ? ` · ${runs.length}` : ''}`}
        open={opened(config.id)}
        onToggle={() => ui.toggleNode(config.id)}
        onSelect={() => ui.revealConfig(config.id, configParent(config))}
        actions={
          <>
            <IconButton
              label={t('terminal:startNamed', { name: config.data.name })}
              disabled={workspace.closing}
              onClick={() => {
                ui.revealConfig(config.id, configParent(config));
                void execute(() => start.mutateAsync([{ kind: config.kind, id: config.id }]));
              }}
            >
              <Play size={14} />
            </IconButton>
            {more(config)}
          </>
        }
      >
        {runs.map(runRow)}
      </TreeRow>
    );
  };
  const targets: (TerminalConfigView | null)[] = [null, ...configs.filter((c) => c.kind === 'ssh')];
  const edited = editor && 'id' in editor ? configs.find((c) => c.id === editor.id) : undefined;
  return (
    <>
      <section className="terminal-config-panel" aria-label={t('terminal:configurations')}>
        <header className="terminal-config-header">
          <button
            className="terminal-config-toggle"
            aria-expanded={ui.configurationsOpen}
            aria-controls="configuration-tree"
            onClick={ui.toggleConfigurations}
          >
            <ChevronRight size={16} />
            <span>{t('terminal:configurations')}</span>
            <span className="terminal-config-count">
              {workspace.runs.length} {t('terminal:active')}
            </span>
          </button>
          <div className="terminal-config-tools">
            <IconButton label={t('terminal:newTarget')} onClick={() => setEditor({ kind: 'ssh' })}>
              <Plus size={17} />
            </IconButton>
            <IconButton label={t('terminal:options')} onClick={() => setOptions(true)}>
              <Settings2 size={17} />
            </IconButton>
          </div>
        </header>
        {ui.configurationsOpen && (
          <div className="terminal-config-body" id="configuration-tree">
            <ul
              role="tree"
              aria-label={t('terminal:configurations')}
              className="configuration-tree"
            >
              {targets.map((target) => {
                const id = target?.id ?? LOCAL_TARGET;
                const children = configs.filter((c) => c.kind !== 'ssh' && configParent(c) === id);
                const directRuns = workspace.runs.filter(
                  (r) => !r.configId && (r.sshTarget || LOCAL_TARGET) === id,
                );
                return (
                  <TreeRow
                    key={id}
                    label={target?.data.name ?? t('terminal:local')}
                    icon={target ? <Server size={16} /> : <Laptop size={16} />}
                    detail={
                      target?.kind === 'ssh'
                        ? `${target.data.username}@${target.data.address.value}:${target.data.port}`
                        : t('terminal:builtinTarget')
                    }
                    open={opened(id)}
                    onToggle={() => ui.toggleNode(id)}
                    onSelect={() => ui.toggleNode(id)}
                    actions={
                      <>
                        {add(id)}
                        {target && more(target)}
                      </>
                    }
                  >
                    {!target && (
                      <TreeRow
                        label={t('terminal:localShell')}
                        icon={<TerminalSquare size={15} />}
                        detail={t('terminal:builtinConfig')}
                        open={opened(LOCAL_SHELL)}
                        onToggle={() => ui.toggleNode(LOCAL_SHELL)}
                        onSelect={() => ui.toggleNode(LOCAL_SHELL)}
                        actions={
                          <IconButton
                            label={t('terminal:newLocal')}
                            disabled={workspace.closing}
                            onClick={() => {
                              ui.revealConfig(LOCAL_SHELL, LOCAL_TARGET);
                              void execute(() =>
                                startLocal.mutateAsync([t('terminal:localShell')]),
                              );
                            }}
                          >
                            <Play size={14} />
                          </IconButton>
                        }
                      >
                        {directRuns.map(runRow)}
                      </TreeRow>
                    )}
                    {children.map(configRow)}
                    {target && directRuns.map(runRow)}
                  </TreeRow>
                );
              })}
            </ul>
          </div>
        )}
      </section>
      {editor &&
        ('kind' in editor ? (
          <ConfigEditor
            initialKind={editor.kind}
            initialSshTarget={editor.sshTarget}
            onClose={() => setEditor(null)}
            onSaved={(id) => ui.revealConfig(id, editor.sshTarget ?? LOCAL_TARGET)}
          />
        ) : edited ? (
          <ConfigEditor
            key={edited.id}
            config={edited}
            initialKind={edited.kind}
            onClose={() => setEditor(null)}
          />
        ) : (
          <Dialog title={t('terminal:copy')} onClose={() => setEditor(null)}>
            <p>{t('terminal:connecting')}</p>
          </Dialog>
        ))}
      {options && <RuntimeOptions onClose={() => setOptions(false)} />}
      {deleting && (
        <Dialog title={t('terminal:delete')} onClose={() => setDeleting(null)}>
          <p>{t('terminal:deleteQuestion', { name: deleting.data.name })}</p>
          <div className="dialog-actions">
            <Button onClick={() => setDeleting(null)}>{t('common:cancel')}</Button>
            <Button
              variant="danger"
              disabled={pending || configUsage(deleting.id, workspace, ui.pending).locked}
              onClick={() =>
                void execute(async () => {
                  await remove.mutateAsync({ target: { kind: deleting.kind, id: deleting.id } });
                  setDeleting(null);
                  toast.success(t('terminal:configDeleted'));
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
