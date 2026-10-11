import { type Problem, errorText, toProblem } from '../../i18n/errors';
import { useI18n } from '../../i18n/react';
import { useState } from 'react';
import { Unplug } from 'lucide-react';
import type { Peer } from '../../desktop/api';
import { useDesktop, useSnapshot } from '../../desktop/api';
import { peerState } from '../../features/devices/selectors';
import { Button } from '../../ui/controls';
import { Dialog } from '../../ui/Dialog';
import { DeviceIcon, DeviceName, IdentityFields } from './DeviceName';

export function PeerDetails({ peer, onClose }: { peer: Peer; onClose: () => void }) {
  const { t } = useI18n();
  const { peers, settings, onboarding } = useSnapshot();
  const client = useDesktop();
  const [confirm, setConfirm] = useState(false);
  const [error, setError] = useState<Problem | null>(null);
  const [busy, setBusy] = useState(false);
  const perform = async (operation: () => Promise<unknown>) => {
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
  const status = peerState(peer, settings.paused);
  const unpaired = peer.connectionState === 'unpaired';
  const nearby = onboarding.nearby.find((device) => device.noobId === peer.noobId);
  return (
    <Dialog
      title={
        confirm ? t(unpaired ? 'devices:removeTitle' : 'devices:unpairTitle') : t('devices:details')
      }
      onClose={onClose}
    >
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
        <p className="dialog-copy">
          {t(unpaired ? 'devices:removeEffect' : 'devices:unpairEffect')}
        </p>
      ) : (
        <>
          <IdentityFields device={peer} />
          <p className="field-note">
            {peer.platform && <>{peer.platform} · </>}
            {status.detail}
            {!unpaired && (
              <>
                {' '}
                ·{' '}
                {peer.settings.autoSend ? t('devices:participates') : t('devices:notParticipating')}
              </>
            )}
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
              {t(unpaired ? 'devices:removeDevice' : 'devices:confirmUnpair')}
            </Button>
          </>
        ) : (
          <>
            <Button variant="quiet" className="unpair-button" onClick={() => setConfirm(true)}>
              <Unplug size={14} />
              {t(unpaired ? 'devices:removeDevice' : 'devices:unpair')}
            </Button>
            {unpaired && nearby?.addresses[0] && (
              <Button
                variant="primary"
                disabled={busy}
                onClick={() =>
                  void perform(() => client.beginPairing(nearby.addresses[0], peer.noobId))
                }
              >
                {t('devices:pairAgain')}
              </Button>
            )}
            {peer.connectionState === 'paused' && settings.paused && (
              <Button
                disabled={busy}
                onClick={() => void perform(() => client.updateSyncSettings({ paused: false }))}
              >
                {t('devices:resumeLocal')}
              </Button>
            )}
            <Button onClick={onClose}>{t('common:close')}</Button>
          </>
        )}
      </div>
    </Dialog>
  );
}
