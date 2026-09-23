import { useI18n } from '../../i18n/react';
import { useState } from 'react';
import { Plus, Radio } from 'lucide-react';
import { useDesktop, useCommand, useSnapshot } from '../../desktop/api';
import { Button, EmptyState } from '../../ui/controls';
import { DiscoverDialog } from './DiscoverDialog';
import { LocalDevicePanel } from './LocalDevicePanel';
import { DeviceRow } from './DeviceRow';
import { PeerDetails } from './PeerDetails';
import { SendSelection } from './SendSelection';

export function DevicesPage() {
  const { t } = useI18n();
  const { peers, manualTargets, settings, configuration } = useSnapshot();
  const client = useDesktop();
  const { execute } = useCommand();
  const [pairing, setPairing] = useState(false);
  const [detailId, setDetailId] = useState<string | null>(null);
  const detail = peers.find((p) => p.noobId === detailId);
  const online = peers.filter((p) => p.online).length;
  const automatic = peers.filter((p) => p.settings.autoSend).length;
  return (
    <section className="devices-page">
      <LocalDevicePanel />
      {configuration.restartRequired && <p role="status">{t('settings:restartRequired')}</p>}
      <section className="device-directory" aria-label={t('devices:directory')}>
        <header className="device-directory__heading">
          <div>
            <h2>
              {t('devices:pairedDevices')}
              <span>{peers.length}</span>
            </h2>
            {peers.length > 0 && (
              <p>{t('devices:connectionCounts', { online, offline: peers.length - online })}</p>
            )}
          </div>
          <Button onClick={() => setPairing(true)}>
            <Plus size={15} />
            {t('devices:add')}
          </Button>
        </header>
        {peers.length ? (
          <>
            <div className="device-columns" aria-hidden="true">
              <span>{t('common:manual')}</span>
              <span>{t('common:devices')}</span>
              <span>{t('devices:connectionStatus')}</span>
              <span>{t('common:autoSend')}</span>
              <span />
            </div>
            <ul className="device-roster" aria-label={t('devices:roster')}>
              {peers.map((peer) => (
                <DeviceRow
                  key={peer.noobId}
                  peer={peer}
                  peers={peers}
                  selected={manualTargets.includes(peer.noobId)}
                  onSelect={() =>
                    execute(() =>
                      client.selectTargets(
                        manualTargets.includes(peer.noobId)
                          ? manualTargets.filter((id) => id !== peer.noobId)
                          : [...manualTargets, peer.noobId],
                      ),
                    )
                  }
                  onAutoSend={(autoSend) =>
                    execute(() => client.configurePeer(peer.noobId, { autoSend }))
                  }
                  onDetails={() => setDetailId(peer.noobId)}
                />
              ))}
            </ul>
            <div className="device-routing-note">
              <Radio size={14} />
              <p>
                {settings.paused
                  ? t('common:paused')
                  : settings.mode === 'automatic'
                    ? t('devices:automaticMode')
                    : t('devices:manualMode')}
                {' · '}
                {automatic
                  ? t('devices:autoEnabled', { count: automatic })
                  : t('devices:noneAutoEnabled')}
              </p>
            </div>
            <SendSelection />
          </>
        ) : (
          <EmptyState icon={<Plus size={24} />} title={t('devices:empty')} />
        )}
      </section>
      {pairing && <DiscoverDialog onClose={() => setPairing(false)} />}
      {detail && (
        <PeerDetails key={detail.noobId} peer={detail} onClose={() => setDetailId(null)} />
      )}
    </section>
  );
}
