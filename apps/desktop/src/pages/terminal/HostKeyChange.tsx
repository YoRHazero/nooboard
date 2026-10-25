import { useEffect, useState } from 'react';
import type { TerminalHostKey } from '../../client/api';
import { useCommand, useDesktop } from '../../client/react';
import { useI18n } from '../../shared/i18n/react';
import { errorText } from '../../shared/i18n/errors';
import { Dialog } from '../../shared/ui/Dialog';
import { Button } from '../../shared/ui/controls';
function Fingerprint({ value }: { value: TerminalHostKey }) {
  const [fingerprint, setFingerprint] = useState('');
  useEffect(() => {
    let active = true;
    void crypto.subtle.digest('SHA-256', new Uint8Array(value.bytes)).then((bytes) => {
      if (active)
        setFingerprint(
          `SHA256:${btoa(String.fromCharCode(...new Uint8Array(bytes))).replace(/=+$/, '')}`,
        );
    });
    return () => {
      active = false;
    };
  }, [value]);
  return (
    <p className="mono terminal-break">
      {value.algorithm}
      <br />
      {fingerprint}
    </p>
  );
}
export function HostKeyChange({
  targetId,
  previous,
  received,
  onClose,
}: {
  targetId: string;
  previous?: TerminalHostKey | null;
  received: TerminalHostKey;
  onClose: () => void;
}) {
  const { t } = useI18n();
  const { terminal } = useDesktop();
  const { execute, pending, error } = useCommand();
  return (
    <Dialog title={t('terminal:hostChanged')} onClose={onClose}>
      <div className="terminal-form">
        <p>{t('terminal:replaceHint')}</p>
        {previous && (
          <>
            <h3>{t('terminal:previousKey')}</h3>
            <Fingerprint value={previous} />
          </>
        )}
        <h3>{t('terminal:receivedKey')}</h3>
        <Fingerprint value={received} />
        {error && (
          <p className="form-error" role="alert">
            {errorText(error)}
          </p>
        )}
        <div className="dialog-actions">
          <Button onClick={onClose}>{t('common:cancel')}</Button>
          <Button
            variant="danger"
            disabled={pending}
            onClick={() =>
              execute(async () => {
                await terminal.request({
                  type: 'replaceHost',
                  data: { targetId, previous, replacement: received },
                });
                onClose();
              })
            }
          >
            {t('terminal:replaceKey')}
          </Button>
        </div>
      </div>
    </Dialog>
  );
}
