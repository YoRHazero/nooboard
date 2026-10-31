import { useSnapshot } from '../../shared/queries/useBackendSnapshotQuery';
import { useI18n } from '../../shared/i18n/react';
import { ArrowUpRight } from 'lucide-react';
import { type Peer } from '../../shared/types';
import { peerState } from '../devices/index';
import { DeviceIcon, DeviceName } from '../devices/index';
export function PairedDevices({
  devices,
  onManage,
}: {
  devices: readonly Peer[];
  onManage: () => void;
}) {
  const { t } = useI18n();
  const { settings } = useSnapshot();
  return (
    <section className="mailbox-devices" aria-label={t('devices:pairedDevices')}>
      <header className="mailbox-section-heading">
        <h3>{t('devices:pairedDevices')}</h3>
        <span>
          {t('home:onlineCount', {
            online: devices.filter((p) => p.online).length,
            total: devices.length,
          })}
        </span>
      </header>
      {devices.length ? (
        <ul className="mailbox-device-list" tabIndex={0} aria-label={t('home:pairedList')}>
          {devices.map((peer) => {
            const status = peerState(peer, settings.paused);
            return (
              <li key={peer.noobId} className="mailbox-device">
                <span
                  className="mailbox-device__platform"
                  role="img"
                  aria-label={peer.platform ?? t('common:devices')}
                >
                  <DeviceIcon platform={peer.platform} size={17} />
                </span>
                <DeviceName device={peer} identities={devices} />
                <span className={`home-status is-${status.tone}`} title={status.detail}>
                  <i />
                  {status.label}
                </span>
              </li>
            );
          })}
        </ul>
      ) : (
        <p className="field-note">{t('common:unpaired')}</p>
      )}
      <button className="text-button panel-link" onClick={onManage}>
        {devices.length ? t('home:manageDevices') : t('devices:add')} <ArrowUpRight size={13} />
      </button>
    </section>
  );
}
