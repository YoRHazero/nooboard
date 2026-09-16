import { type Problem, errorText, toProblem } from '../../i18n/errors';
import { useI18n } from '../../i18n/react';
import { useState } from 'react';
import { Unplug } from 'lucide-react';
import type { Peer } from '../../api/contracts';
import { useClient, useSnapshot } from '../../api/NooboardProvider';
import { peerState } from '../../api/devices';
import { Button } from '../../ui/controls';
import { Dialog } from '../../ui/Dialog';
import { DeviceIcon, DeviceName, IdentityFields } from './DeviceName';

export function PeerDetails({ peer, onClose }: { peer: Peer; onClose: () => void }) {
  const { t } = useI18n();
  const { peers } = useSnapshot();
  const client = useClient();
  const [address, setAddress] = useState(peer.settings.address ?? '');
  const [confirm, setConfirm] = useState(false);
  const [error, setError] = useState<Problem | null>(null);
  const [busy, setBusy] = useState(false);
  const perform = async (operation: () => Promise<void>) => {
    setBusy(true);
    setError(null);
    try {
      await operation();
      onClose();
    } catch (reason) {
      setError(toProblem(reason));
    } finally {
      setBusy(false);
    }
  };
  const status = peerState(peer);
  return (
    <Dialog title={confirm ? t('devices:unpairTitle') : t('devices:details')} onClose={onClose}>
      <div className="peer-detail-heading">
        <span className="device-row__icon">
          <DeviceIcon platform={peer.platform} size={28} />
        </span>
        <DeviceName device={peer} identities={peers} />
        <span className={`home-status is-${status.tone}`}>
          <i />
          {status.label}
        </span>
      </div>
      {confirm ? (
        <p className="dialog-copy">{t('devices:unpairEffect')}</p>
      ) : (
        <>
          <label className="form-field">
            <span>{t('devices:peerAddress')}</span>
            <input
              value={address}
              onChange={(e) => setAddress(e.target.value)}
              placeholder="192.168.1.38:24816"
              autoComplete="off"
              spellCheck={false}
            />
          </label>
          <p className="field-note">{t('devices:emptyAddress')}</p>
          <IdentityFields device={peer} />
          <p className="field-note">
            {peer.platform ?? t('devices:pairedDevices')} · {status.detail} ·{' '}
            {peer.settings.autoSend ? t('devices:participates') : t('devices:notParticipating')}
          </p>
        </>
      )}
      {error && (
        <p className="form-error" role="alert">
          {errorText(error)}
        </p>
      )}
      <div className="dialog-actions peer-detail-actions">
        {confirm ? (
          <>
            <Button disabled={busy} onClick={() => setConfirm(false)}>
              {t('common:back')}
            </Button>
            <Button
              variant="danger"
              disabled={busy}
              onClick={() => void perform(() => client.unpair(peer.noobId))}
            >
              {t('devices:confirmUnpair')}
            </Button>
          </>
        ) : (
          <>
            <Button variant="quiet" className="unpair-button" onClick={() => setConfirm(true)}>
              <Unplug size={14} />
              {t('devices:unpair')}
            </Button>
            <Button
              variant="primary"
              disabled={busy}
              onClick={() =>
                void perform(() =>
                  client.configurePeer(peer.noobId, { address: address.trim() || null }),
                )
              }
            >
              {t('devices:saveAddress')}
            </Button>
          </>
        )}
      </div>
    </Dialog>
  );
}
