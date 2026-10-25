import { type Problem, errorText, toProblem } from '../../shared/i18n/errors';
import { useI18n } from '../../shared/i18n/react';
import { useState } from 'react';
import { Unplug } from 'lucide-react';
import type { Peer } from '../../client/api';
import { useDesktop, useSnapshot } from '../../client/react';
import { peerAddresses, peerState } from '../../features/devices/api';
import { Button } from '../../shared/ui/controls';
import { Dialog } from '../../shared/ui/Dialog';
import { DeviceIcon, DeviceName, IdentityFields } from '../../features/devices/api';

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
  const addresses = peerAddresses(peer, onboarding.nearby);
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
          <dl className="identity-fields peer-addresses">
            <div>
              <dt>{t('devices:currentConnectionAddress')}</dt>
              <dd>
                {addresses.current ? (
                  <>
                    <code>{addresses.current}</code>
                    <p className="field-note">{t('devices:currentConnectionAddressHelp')}</p>
                  </>
                ) : (
                  <span className="field-note">{t('devices:noCurrentConnection')}</span>
                )}
              </dd>
            </div>
            {addresses.discovered.length > 0 && (
              <div>
                <dt>{t('devices:discoveredSyncAddresses')}</dt>
                <dd>
                  {addresses.discovered.map((address) => (
                    <code key={address}>{address}</code>
                  ))}
                </dd>
              </div>
            )}
            {addresses.saved && (
              <div>
                <dt>{t('devices:savedAddress')}</dt>
                <dd>
                  <code>{addresses.saved}</code>
                  <p className="field-note">{t('devices:savedAddressHelp')}</p>
                </dd>
              </div>
            )}
          </dl>
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
