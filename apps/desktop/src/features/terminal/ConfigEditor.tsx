import type { TerminalConfigView, TerminalResourceKind } from '../../shared/types';
import { SshTargetEditor } from './SshTargetEditor';
import { SessionConfigEditor } from './SessionConfigEditor';
import { CommandConfigEditor } from './CommandConfigEditor';
import { TunnelConfigEditor } from './TunnelConfigEditor';
export function ConfigEditor({
  config,
  initialKind,
  initialSshTarget,
  onClose,
  onSaved,
}: {
  config?: TerminalConfigView;
  initialKind: TerminalResourceKind;
  initialSshTarget?: string;
  onClose(): void;
  onSaved?(id: string): void;
}) {
  const props = { sshTarget: initialSshTarget, onClose, onSaved };
  switch (config?.kind ?? initialKind) {
    case 'ssh':
      return <SshTargetEditor {...props} config={config?.kind === 'ssh' ? config : undefined} />;
    case 'terminal':
      return (
        <SessionConfigEditor {...props} config={config?.kind === 'terminal' ? config : undefined} />
      );
    case 'command':
      return (
        <CommandConfigEditor {...props} config={config?.kind === 'command' ? config : undefined} />
      );
    case 'tunnel':
      return (
        <TunnelConfigEditor {...props} config={config?.kind === 'tunnel' ? config : undefined} />
      );
  }
}
