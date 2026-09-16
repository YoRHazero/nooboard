import { useI18n } from '../../i18n/react';
import { useState } from 'react';
import { Pencil } from 'lucide-react';
import { useSnapshot } from '../../api/NooboardProvider';
import { Button } from '../../ui/controls';
import { DeviceName } from './DeviceName';
import { LocalDeviceDialog } from './LocalDeviceDialog';

export function LocalDevicePanel() {
  const { t } = useI18n();
  const { localDevice, peers, settings } = useSnapshot();
  const [editing, setEditing] = useState(false);
  return (
    <>
      <section className="local-device-strip" aria-label={t('common:thisDevice')}>
        <img className="local-device-bird" src="/mascot/icon.png" alt={t('devices:localBird')} />
        <div className="local-device-heading">
          <span className="eyebrow">
            {t('devices:localPlatform', {
              platform: localDevice.platform ?? t('devices:thisPlatform'),
            })}
          </span>
          <DeviceName device={localDevice} identities={peers} />
        </div>
        <div className="local-device-meta">
          <span>
            {settings.paused
              ? t('common:paused')
              : settings.receive
                ? t('common:receiveOn')
                : t('common:receiveOff')}
          </span>
          <span>
            {localDevice.addresses.length === 1 ? (
              <>
                {t('devices:localIp')}
                {' · '}
                <code>{localDevice.addresses[0].ip}</code>
              </>
            ) : localDevice.addresses.length > 1 ? (
              t('devices:localIps', { count: localDevice.addresses.length })
            ) : (
              t('devices:noIp')
            )}
          </span>
        </div>
        <Button variant="quiet" onClick={() => setEditing(true)}>
          <Pencil size={14} />
          {t('devices:localInfo')}
        </Button>
      </section>
      {editing && <LocalDeviceDialog onClose={() => setEditing(false)} />}
    </>
  );
}
