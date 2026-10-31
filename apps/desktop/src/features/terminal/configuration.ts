import type { TerminalConfigView, TerminalTarget, TerminalWorkspaceView } from '../../shared/types';

export const LOCAL_TARGET = 'local';
export const LOCAL_SHELL = 'local-shell';
export function configParent(config: TerminalConfigView): string {
  return config.kind === 'ssh' ? config.id : config.data.sshTarget || LOCAL_TARGET;
}

export function references(config: TerminalConfigView): string[] {
  if (config.kind === 'ssh') return [];
  return [
    ...(config.data.sshTarget ? [config.data.sshTarget] : []),
    ...('dependencies' in config.data ? config.data.dependencies.map((dep) => dep.commandId) : []),
  ];
}

/** Saved references and running/starting plans are separate reasons to lock an edit. */
export function configUsage(
  id: string,
  workspace: Pick<TerminalWorkspaceView, 'configs' | 'runs'>,
  pending: readonly { target: TerminalTarget | null }[] = [],
) {
  const referrers = workspace.configs.filter((config) => references(config).includes(id));
  const pendingIds = new Set<string>();
  const visit = (key: string) => {
    if (pendingIds.has(key)) return;
    pendingIds.add(key);
    const config = workspace.configs.find((item) => item.id === key);
    if (config) references(config).forEach(visit);
  };
  pending.forEach((item) => {
    if (item.target) visit(item.target.id);
  });
  const active = workspace.runs.some((run) => run.configId === id || run.sshTarget === id);
  return {
    referrers,
    active: active || pendingIds.has(id),
    locked: active || pendingIds.has(id) || referrers.length > 0,
  };
}
