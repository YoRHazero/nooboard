import { useState } from 'react';
import { useI18n } from '../../shared/i18n/react';
import { Select, SelectItem } from '../../shared/ui/Select';
import { ConfigDialog, type ConfigFormProps } from './ConfigDialog';
import { SshAddressFields, type SshAddressDraft } from './SshAddressFields';
import { handleAction as execute } from '../../shared/action-feedback';
export function SshTargetEditor({ config, ...props }: ConfigFormProps<'ssh'>) {
  const { t } = useI18n();
  const [address, setAddress] = useState<SshAddressDraft>({
    kind: config?.kind === 'ssh' ? config.data.address.kind : 'host',
    host:
      config?.kind === 'ssh' && config.data.address.kind === 'host'
        ? config.data.address.value
        : '',
    deviceId:
      config?.kind === 'ssh' && config.data.address.kind === 'device'
        ? config.data.address.value
        : '',
    port: config?.kind === 'ssh' ? String(config.data.port) : '22',
  });
  const [username, setUsername] = useState(config?.kind === 'ssh' ? config.data.username : '');
  const [auth, setAuth] = useState<'keep' | 'password' | 'privateKey'>(
    config?.kind === 'ssh' ? 'keep' : 'password',
  );
  const [privateKey, setPrivateKey] = useState('');
  const [passphrase, setPassphrase] = useState('');
  return (
    <ConfigDialog
      {...props}
      config={config}
      kind="ssh"
      build={(name) => ({
        kind: 'ssh',
        data: {
          name,
          address: {
            kind: address.kind,
            value: address.kind === 'host' ? address.host.trim() : address.deviceId,
          },
          port: Number(address.port),
          username: username.trim(),
          auth,
          ...(auth === 'privateKey' ? { privateKey, passphrase: passphrase || null } : {}),
        },
      })}
    >
      <SshAddressFields value={address} onChange={setAddress} />
      <div className="terminal-form-grid">
        <label className="form-field">
          <span>{t('terminal:username')}</span>
          <input
            required
            value={username}
            autoComplete="off"
            onChange={(e) => setUsername(e.target.value)}
          />
        </label>
        <label className="form-field">
          <span>{t('terminal:authentication')}</span>
          <Select
            aria-label={t('terminal:authentication')}
            value={auth}
            onValueChange={(e) => setAuth(e as typeof auth)}
          >
            {config && <SelectItem value="keep">{t('terminal:keep')}</SelectItem>}
            <SelectItem value="password">{t('terminal:password')}</SelectItem>
            <SelectItem value="privateKey">{t('terminal:privateKey')}</SelectItem>
          </Select>
        </label>
      </div>
      {auth === 'password' && <p className="muted small">{t('terminal:passwordHint')}</p>}
      {auth === 'privateKey' && (
        <>
          <label className="form-field">
            <span>{t('terminal:keyFile')}</span>
            <input
              type="file"
              onChange={(e) => {
                const file = e.target.files?.[0];
                if (file) void execute(async () => setPrivateKey(await file.text()));
              }}
            />
          </label>
          <label className="form-field">
            <span>{t('terminal:keyContent')}</span>
            <textarea
              required
              className="mono"
              rows={4}
              autoComplete="off"
              autoCorrect="off"
              spellCheck={false}
              value={privateKey}
              onChange={(e) => setPrivateKey(e.target.value)}
            />
          </label>
          <label className="form-field">
            <span>{t('terminal:passphrase')}</span>
            <input
              type="password"
              autoComplete="new-password"
              value={passphrase}
              onChange={(e) => setPassphrase(e.target.value)}
            />
          </label>
          <p className="muted small">{t('terminal:keyHint')}</p>
        </>
      )}
    </ConfigDialog>
  );
}
