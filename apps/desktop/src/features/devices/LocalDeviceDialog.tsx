import { type Problem, problem, errorText, toProblem } from '../../i18n/errors';
import { useI18n } from '../../i18n/react';
import { useState } from 'react';
import { useDesktop, useSnapshot } from '../../desktop/api';
import { parsePort, validName } from '../../features/devices/selectors';
import { Button } from '../../ui/controls';
import { Dialog } from '../../ui/Dialog';
import { IdentityFields } from './DeviceName';
import { LocalAddresses } from './LocalAddresses';

export function LocalDeviceDialog({ onClose }: { onClose: () => void }) {
  const { t } = useI18n();
  const { localDevice } = useSnapshot();
  const client = useDesktop();
  const [initial] = useState(localDevice);
  const [deviceName, setDeviceName] = useState(initial.deviceName);
  const [pairingPort, setPairingPort] = useState(String(initial.pairingPort));
  const [error, setError] = useState<Problem | null>(null);
  const [saving, setSaving] = useState(false);
  const save = async () => {
    const pairing = parsePort(pairingPort);
    if (!validName(deviceName.trim())) {
      setError(problem('invalidName'));
      return;
    }
    if (pairing === null) {
      setError(problem('invalidPort'));
      return;
    }
    if (pairing === localDevice.syncPort) {
      setError(problem('portInUse'));
      return;
    }
    setSaving(true);
    setError(null);
    try {
      await client.updateLocalDevice({
        deviceName: deviceName.trim(),
        ...(pairing !== initial.pairingPort ? { pairingPort: pairing } : {}),
      });
      onClose();
    } catch (reason) {
      setError(toProblem(reason));
    } finally {
      setSaving(false);
    }
  };
  return (
    <Dialog title={t('devices:localInfo')} onClose={onClose}>
      <form
        onSubmit={(event) => {
          event.preventDefault();
          if (!saving) void save();
        }}
      >
        <LocalAddresses device={localDevice} />
        <div className="local-device-fields">
          <label className="form-field">
            <span>{t('devices:deviceName')}</span>
            <input
              value={deviceName}
              onChange={(event) => setDeviceName(event.target.value)}
              maxLength={160}
              autoComplete="off"
              disabled={saving}
            />
          </label>
          <label className="form-field">
            <span>{t('devices:pairingPort')}</span>
            <input
              value={pairingPort}
              onChange={(event) => setPairingPort(event.target.value)}
              inputMode="numeric"
              spellCheck={false}
              autoComplete="off"
              disabled={saving}
            />
          </label>
        </div>
        <details className="local-device-identity">
          <summary>{t('devices:identity')}</summary>
          <IdentityFields device={localDevice} />
        </details>
        {error && (
          <p className="form-error" role="alert">
            {errorText(error)}
          </p>
        )}
        <div className="dialog-actions">
          <Button onClick={onClose}>{t('common:cancel')}</Button>
          <Button type="submit" variant="primary" disabled={saving}>
            {saving ? t('common:saving') : t('common:save')}
          </Button>
        </div>
      </form>
    </Dialog>
  );
}
