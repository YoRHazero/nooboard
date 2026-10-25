import { useI18n } from '../../shared/i18n/react';
import { ChevronRight } from 'lucide-react';
import { type Peer } from '../../client/api';
import { useSnapshot } from '../../client/react';
import { peerState, shortNoobId } from '../../features/devices/api';
import { Toggle } from '../../shared/ui/controls';
import { DeviceName, DeviceIcon } from '../../features/devices/api';

export function DeviceRow({
  peer,
  peers,
  onAutoSend,
  onDetails,
}: {
  peer: Peer;
  peers: readonly Peer[];
  onAutoSend: (value: boolean) => void;
  onDetails: () => void;
}) {
  const { t } = useI18n();
  const { settings } = useSnapshot();
  const status = peerState(peer, settings.paused);
  const label = `${peer.deviceName} ${shortNoobId(peer.noobId, peers)}`;
  return (
    <li className="device-row">
      <button
        className="device-row__identity"
        onClick={onDetails}
        aria-label={t('devices:detailsLabel', { name: label })}
      >
        <span className="device-row__icon">
          <DeviceIcon platform={peer.platform} />
        </span>
        <DeviceName device={peer} identities={peers} />
      </button>
      <div className="device-row__connection">
        <span className={`home-status is-${status.tone}`}>
          <i />
          {status.label}
        </span>
        <small>{status.detail}</small>
      </div>
      <div className="device-row__automatic">
        <span>{t('common:autoSend')}</span>
        <Toggle
          label={t('devices:autoSendLabel', { name: label })}
          disabled={peer.unpairedNotice != null}
          checked={peer.settings.autoSend}
          onChange={onAutoSend}
        />
      </div>
      <button
        className="device-row__more icon-button"
        aria-label={t('devices:detailsLabel', { name: label })}
        title={t('devices:details')}
        onClick={onDetails}
      >
        <ChevronRight size={16} />
      </button>
    </li>
  );
}
