import { useState } from 'react';
import type { TerminalDependency } from '../../shared/types';
import { ConfigDialog, type ConfigFormProps } from './ConfigDialog';
import { DependencyFields } from './DependencyFields';
export function SessionConfigEditor({ config, sshTarget, ...props }: ConfigFormProps<'terminal'>) {
  const [dependencies, setDependencies] = useState<TerminalDependency[]>(
    config?.data.dependencies ?? [],
  );
  return (
    <ConfigDialog
      {...props}
      config={config}
      kind="terminal"
      sshTarget={sshTarget}
      build={(name) => ({
        kind: 'terminal',
        data: { name, sshTarget: config?.data.sshTarget ?? sshTarget ?? null, dependencies },
      })}
    >
      <DependencyFields dependencies={dependencies} setDependencies={setDependencies} />
    </ConfigDialog>
  );
}
