import { useI18n } from '../../i18n/react';
import { ChevronRight } from 'lucide-react';
import type { Peer } from '../../desktop/api';
import { peerState, shortNoobId } from '../../features/devices/selectors';
import { Toggle } from '../../ui/controls';
import { DeviceName, DeviceIcon } from './DeviceName';

export function DeviceRow({
  peer,
  peers,
  selected,
  onSelect,
  onAutoSend,
  onDetails,
}: {
  peer: Peer;
  peers: readonly Peer[];
  selected: boolean;
  onSelect: () => void;
  onAutoSend: (value: boolean) => void;
  onDetails: () => void;
}) {
  const { t } = useI18n();
  const status = peerState(peer);
  const label = `${peer.deviceName} ${shortNoobId(peer.noobId, peers)}`;
  return (
    <li className={`device-row ${selected ? 'is-selected' : ''}`}>
      <label className="device-select">
        <input
          type="checkbox"
          checked={selected}
          onChange={onSelect}
          aria-label={t('devices:sendToLabel', { name: label })}
        />
        <span className="sr-only">{t('common:manualSend')}</span>
      </label>
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
          checked={peer.settings.autoSend}
          onChange={onAutoSend}
        />
      </div>
      <button
        className="device-row__more icon-button"
        aria-label={t('devices:connectionLabel', { name: label })}
        title={t('devices:connectionSettings')}
        onClick={onDetails}
      >
        <ChevronRight size={16} />
      </button>
    </li>
  );
}
