import { useI18n } from '../../i18n/react';
import { Laptop, Monitor, Terminal } from 'lucide-react';
import type { DeviceIdentity, Platform } from '../../desktop/api';
import { shortNoobId } from '../../features/devices/selectors';
export function DeviceIcon({ platform, size = 24 }: { platform?: Platform; size?: number }) {
  const Icon = platform === 'macOS' ? Laptop : platform === 'Ubuntu' ? Terminal : Monitor;
  return <Icon size={size} strokeWidth={1.5} aria-hidden="true" />;
}
export function DeviceName({
  device,
  identities = [],
}: {
  device: Pick<DeviceIdentity, 'noobId' | 'deviceName'>;
  identities?: readonly Pick<DeviceIdentity, 'noobId'>[];
}) {
  const { t } = useI18n();
  return (
    <span className="device-name">
      <strong title={device.deviceName}>{device.deviceName}</strong>
      <code
        title={`noob_id · ${device.noobId}`}
        aria-label={t('common:deviceIdLabel', { id: device.noobId })}
      >
        {shortNoobId(device.noobId, identities)}
      </code>
    </span>
  );
}
export function IdentityFields({ device }: { device: DeviceIdentity }) {
  const { t } = useI18n();
  return (
    <dl className="identity-fields">
      <div>
        <dt>{t('common:deviceId')}</dt>
        <dd>
          <code>{device.noobId}</code>
        </dd>
      </div>
      <div>
        <dt>{t('common:fingerprint')}</dt>
        <dd>
          <code>{device.fingerprint.match(/.{1,4}/g)?.join(' ')}</code>
        </dd>
      </div>
    </dl>
  );
}
