import { useId } from 'react';
import { RefreshCw } from 'lucide-react';
import { useCommand, useDesktop, useSnapshot } from '../../client/react';
import { shortNoobId } from '../../features/devices/api';
import { useI18n } from '../../shared/i18n/react';
import { errorText } from '../../shared/i18n/errors';
import { Button } from '../../shared/ui/controls';
import { Select, SelectItem } from '../../shared/ui/Select';

export interface SshAddressDraft {
  kind: 'host' | 'device';
  host: string;
  deviceId: string;
  port: string;
}
interface DeviceOption {
  noobId: string;
  deviceName: string;
  detected: boolean;
}

export function SshAddressFields({
  value,
  onChange,
}: {
  value: SshAddressDraft;
  onChange: (value: SshAddressDraft) => void;
}) {
  const { t } = useI18n();
  const client = useDesktop();
  const peers = useSnapshot((s) => s.peers);
  const nearby = useSnapshot((s) => s.onboarding.nearby);
  const localId = useSnapshot((s) => s.localDevice.noobId);
  const { execute, pending, error } = useCommand();
  const deviceFieldId = useId();
  const devicesById = new Map<string, DeviceOption>();
  for (const peer of peers)
    if (peer.noobId && peer.noobId !== localId) {
      devicesById.set(peer.noobId, {
        noobId: peer.noobId,
        deviceName: peer.deviceName,
        detected: peer.online && !!peer.connectionAddress,
      });
    }
  // Discovery includes unpaired devices and may contain several interfaces per ID.
  for (const device of nearby)
    if (device.noobId && device.noobId !== localId) {
      const known = devicesById.get(device.noobId);
      devicesById.set(device.noobId, {
        noobId: device.noobId,
        deviceName: known?.deviceName ?? device.deviceName,
        detected: !!known?.detected || device.addresses.length > 0,
      });
    }
  // Preserve an existing selection even if its device has disappeared from discovery.
  if (value.deviceId && !devicesById.has(value.deviceId)) {
    devicesById.set(value.deviceId, {
      noobId: value.deviceId,
      deviceName: t('terminal:unavailableDevice'),
      detected: false,
    });
  }
  const devices = [...devicesById.values()].sort(
    (a, b) => Number(b.detected) - Number(a.detected) || a.deviceName.localeCompare(b.deviceName),
  );
  const refresh = () => execute(() => client.discover());
  const chooseSource = (kind: SshAddressDraft['kind']) => {
    if (kind === value.kind) return;
    onChange({ ...value, kind });
    if (kind === 'device' && !pending) void refresh();
  };

  return (
    <>
      <div className="terminal-form-grid">
        <div className="form-field">
          <span>{t('terminal:addressKind')}</span>
          <div
            className="segmented terminal-address-source"
            role="group"
            aria-label={t('terminal:addressKind')}
          >
            <button
              type="button"
              aria-pressed={value.kind === 'host'}
              onClick={() => chooseSource('host')}
            >
              {t('terminal:manual')}
            </button>
            <button
              type="button"
              aria-pressed={value.kind === 'device'}
              onClick={() => chooseSource('device')}
            >
              {t('terminal:detect')}
            </button>
          </div>
        </div>
        <label className="form-field">
          <span>{t('terminal:port')}</span>
          <input
            type="number"
            min="1"
            max="65535"
            required
            value={value.port}
            onChange={(event) => onChange({ ...value, port: event.target.value })}
          />
        </label>
      </div>
      {value.kind === 'host' ? (
        <label className="form-field">
          <span>{t('terminal:host')}</span>
          <input
            required
            value={value.host}
            autoComplete="off"
            spellCheck={false}
            onChange={(event) => onChange({ ...value, host: event.target.value })}
          />
        </label>
      ) : (
        <>
          <div className="form-field">
            <div className="terminal-device-heading">
              <label htmlFor={deviceFieldId}>{t('terminal:device')}</label>
              <Button
                variant="quiet"
                disabled={pending}
                onClick={refresh}
                aria-label={t('terminal:refreshDevices')}
              >
                <RefreshCw size={13} />
                {pending ? t('devices:searching') : t('terminal:refresh')}
              </Button>
            </div>
            <Select
              id={deviceFieldId}
              aria-label={t('terminal:device')}
              required
              value={value.deviceId}
              onValueChange={(deviceId) => onChange({ ...value, deviceId })}
            >
              <SelectItem value="" disabled>
                {t('terminal:selectDevice')}
              </SelectItem>
              {devices.map((device) => (
                <SelectItem key={device.noobId} value={device.noobId}>
                  {device.deviceName} · {shortNoobId(device.noobId, devices)}
                  {device.detected ? '' : ` · ${t('terminal:notDetected')}`}
                </SelectItem>
              ))}
            </Select>
          </div>
          <p className="muted small">
            {devices.length ? t('terminal:deviceHint') : t('terminal:noDevices')}
          </p>
          {error && (
            <p className="form-error" role="alert">
              {errorText(error)}
            </p>
          )}
        </>
      )}
    </>
  );
}
