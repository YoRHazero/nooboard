import { useState } from 'react';
import { useI18n } from '../../shared/i18n/react';
import { Select, SelectItem } from '../../shared/ui/Select';
import { ConfigDialog, type ConfigFormProps } from './ConfigDialog';
import type { TerminalDependency } from '../../shared/types';
import { DependencyFields } from './DependencyFields';
export function TunnelConfigEditor({
  config,
  sshTarget = '',
  ...props
}: ConfigFormProps<'tunnel'>) {
  const { t } = useI18n();
  const [dependencies, setDependencies] = useState<TerminalDependency[]>(
    config?.data.dependencies ?? [],
  );
  const [forwardKind, setForwardKind] = useState<'local' | 'remote' | 'dynamic'>(
    config?.kind === 'tunnel' ? config.data.forward.kind : 'local',
  );
  const [listenHost, setListenHost] = useState(
    config?.kind === 'tunnel' ? config.data.forward.listenHost : '127.0.0.1',
  );
  const [listenPort, setListenPort] = useState(
    config?.kind === 'tunnel' ? String(config.data.forward.listenPort) : '8080',
  );
  const [destinationHost, setDestinationHost] = useState(
    config?.kind === 'tunnel' ? (config.data.forward.destinationHost ?? '') : '127.0.0.1',
  );
  const [destinationPort, setDestinationPort] = useState(
    config?.kind === 'tunnel' ? String(config.data.forward.destinationPort ?? '') : '80',
  );
  return (
    <ConfigDialog
      {...props}
      config={config}
      kind="tunnel"
      sshTarget={sshTarget}
      build={(name) => ({
        kind: 'tunnel',
        data: {
          name,
          sshTarget: config?.data.sshTarget ?? sshTarget,
          dependencies,
          forward: {
            kind: forwardKind,
            listenHost: listenHost.trim(),
            listenPort: Number(listenPort),
            ...(forwardKind === 'dynamic'
              ? {}
              : {
                  destinationHost: destinationHost.trim(),
                  destinationPort: Number(destinationPort),
                }),
          },
        },
      })}
    >
      <label className="form-field">
        <span>{t('terminal:forwardKind')}</span>
        <Select
          aria-label={t('terminal:forwardKind')}
          value={forwardKind}
          onValueChange={(e) => setForwardKind(e as typeof forwardKind)}
        >
          <SelectItem value="local">{t('terminal:localForward')}</SelectItem>
          <SelectItem value="remote">{t('terminal:remoteForward')}</SelectItem>
          <SelectItem value="dynamic">{t('terminal:dynamicForward')}</SelectItem>
        </Select>
      </label>
      <div className="terminal-form-grid">
        <label className="form-field">
          <span>{t('terminal:listenHost')}</span>
          <input required value={listenHost} onChange={(e) => setListenHost(e.target.value)} />
        </label>
        <label className="form-field">
          <span>{t('terminal:listenPort')}</span>
          <input
            type="number"
            min="1"
            max="65535"
            required
            value={listenPort}
            onChange={(e) => setListenPort(e.target.value)}
          />
        </label>
        {forwardKind !== 'dynamic' && (
          <>
            <label className="form-field">
              <span>{t('terminal:destinationHost')}</span>
              <input
                required
                value={destinationHost}
                onChange={(e) => setDestinationHost(e.target.value)}
              />
            </label>
            <label className="form-field">
              <span>{t('terminal:destinationPort')}</span>
              <input
                type="number"
                min="1"
                max="65535"
                required
                value={destinationPort}
                onChange={(e) => setDestinationPort(e.target.value)}
              />
            </label>
          </>
        )}
      </div>

      <DependencyFields dependencies={dependencies} setDependencies={setDependencies} />
    </ConfigDialog>
  );
}
