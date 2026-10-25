import { type Problem, errorText, toProblem } from '../../shared/i18n/errors';
import { useI18n } from '../../shared/i18n/react';
import { useState } from 'react';
import { useDesktop, useSnapshot } from '../../client/react';
import { canSend, peerState, shortNoobId } from '../devices/api';
import { DeviceIcon, DeviceName } from '../devices/api';
import { Button } from '../../shared/ui/controls';
import { Dialog } from '../../shared/ui/Dialog';

export function TargetPicker({ onClose }: { onClose: () => void }) {
  const { t } = useI18n();
  const { peers, manualTargets, settings } = useSnapshot();
  const client = useDesktop();
  const [selected, setSelected] = useState([...manualTargets]);
  const [error, setError] = useState<Problem | null>(null);
  const save = async () => {
    try {
      await client.selectTargets(selected.filter((id) => peers.some((p) => p.noobId === id)));
      onClose();
    } catch (reason) {
      setError(toProblem(reason));
    }
  };
  return (
    <Dialog title={t('transfers:targetsTitle')} onClose={onClose}>
      <ul className="target-picker">
        {peers.map((peer) => (
          <li key={peer.noobId}>
            <label>
              <input
                type="checkbox"
                checked={selected.includes(peer.noobId)}
                onChange={() =>
                  setSelected((ids) =>
                    ids.includes(peer.noobId)
                      ? ids.filter((id) => id !== peer.noobId)
                      : [...ids, peer.noobId],
                  )
                }
                aria-label={t('transfers:selectLabel', {
                  name: peer.deviceName,
                  id: shortNoobId(peer.noobId, peers),
                })}
              />
              <DeviceIcon platform={peer.platform} size={20} />
              <DeviceName device={peer} identities={peers} />
              <small>
                {canSend(peer)
                  ? t('common:canReceive')
                  : peer.connectionState === 'connected'
                    ? t('devices:notReceiving')
                    : peerState(peer, settings.paused).label}
              </small>
            </label>
          </li>
        ))}
      </ul>
      {error && (
        <p className="form-error" role="alert">
          {errorText(error)}
        </p>
      )}
      <div className="dialog-actions">
        <Button onClick={onClose}>{t('common:cancel')}</Button>
        <Button variant="primary" onClick={() => void save()}>
          {t('transfers:saveSelection')}
        </Button>
      </div>
    </Dialog>
  );
}
