import { useState } from 'react';
import { useDesktop, useSnapshot, type Peer } from '../../desktop/api';
import { type Problem, errorText, toProblem } from '../../i18n/errors';
import { useI18n } from '../../i18n/react';
import { Dialog } from '../../ui/Dialog';
import { Button } from '../../ui/controls';

export function UnpairedPrompt() {
  const { peers, session, onboarding } = useSnapshot();
  const [dismissed, setDismissed] = useState<ReadonlySet<string>>(new Set());
  const key = (peer: Peer) => `${session}:${peer.noobId}:${peer.unpairedNotice}`;
  const peer = peers.find((peer) => peer.unpairedNotice != null && !dismissed.has(key(peer)));
  if (!peer || onboarding.session) return null;
  const id = key(peer);
  return (
    <Notice key={id} peer={peer} onClose={() => setDismissed((old) => new Set([...old, id]))} />
  );
}

function Notice({ peer, onClose }: { peer: Peer; onClose: () => void }) {
  const { t } = useI18n();
  const client = useDesktop();
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<Problem | null>(null);
  const remove = async () => {
    setBusy(true);
    setError(null);
    try {
      await client.unpair(peer.noobId);
      onClose();
    } catch (reason) {
      setError(toProblem(reason));
    } finally {
      setBusy(false);
    }
  };
  return (
    <Dialog
      title={t('devices:unpairedNoticeTitle', { name: peer.deviceName })}
      onClose={() => {
        if (!busy) onClose();
      }}
    >
      <p className="dialog-copy">{t('devices:unpairedNoticeBody')}</p>
      {error && (
        <p className="form-error" role="alert">
          {errorText(error)}
        </p>
      )}
      <div className="dialog-actions">
        <Button disabled={busy} onClick={onClose}>
          {t('devices:keepDevice')}
        </Button>
        <Button variant="danger" disabled={busy} onClick={() => void remove()}>
          {t('devices:removeDevice')}
        </Button>
      </div>
    </Dialog>
  );
}
