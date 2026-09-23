import { type Problem, errorText, toProblem } from '../../i18n/errors';
import { useI18n } from '../../i18n/react';
import { useEffect, useState } from 'react';
import { Check, KeyRound, LoaderCircle } from 'lucide-react';
import { useDesktop, useSnapshot } from '../../desktop/api';
import type { PairingSession } from '../../desktop/api';
import { Dialog } from '../../ui/Dialog';
import { Button } from '../../ui/controls';
export function PairingPrompt() {
  const { onboarding } = useSnapshot();
  return onboarding?.session ? (
    <SessionPrompt key={onboarding.session.id} session={onboarding.session} />
  ) : null;
}
function SessionPrompt({ session }: { session: PairingSession }) {
  const { t } = useI18n();
  const client = useDesktop();
  const [code, setCode] = useState('');
  const [error, setError] = useState<Problem | null>(null);
  const [busy, setBusy] = useState(false);
  const [now, setNow] = useState(Date.now());
  useEffect(() => {
    const timer = setInterval(() => setNow(Date.now()), 1000);
    return () => clearInterval(timer);
  }, []);
  useEffect(() => {
    if (session.stage === 'EnteringCode') setCode('');
  }, [session.stage, session.attemptsLeft]);
  const run = async (action: () => Promise<void>) => {
    setBusy(true);
    setError(null);
    try {
      await action();
    } catch (e) {
      setError(toProblem(e));
    } finally {
      setBusy(false);
    }
  };
  const close = () => void run(() => client.dismissPairing(session.id));
  const incoming = session.incoming && session.stage === 'AwaitingApproval';
  const done = session.stage === 'Completed';
  const failed = session.stage === 'Failed';
  const titles = {
    Requesting: t('devices:requesting'),
    AwaitingApproval: incoming ? t('devices:incomingRequest') : t('devices:awaitingApproval'),
    ShowingCode: t('devices:showCode'),
    EnteringCode: t('devices:enterCode'),
    Verifying: t('devices:verifying'),
    Saving: t('devices:savingPair'),
    Completed: t('devices:pairCompleted'),
    Failed: t('devices:pairFailed'),
  };
  const left = Math.min(120, Math.max(0, Math.ceil((session.expiresAt - now) / 1000)));
  return (
    <Dialog title={titles[session.stage]} onClose={close}>
      <div className="pair-session-heading">
        <img src="/mascot/icon.png" alt={t('common:bird')} />
        <div>
          <strong>{session.deviceName}</strong>
          {session.noobId && <code title={session.noobId}>{session.noobId.slice(0, 8)}</code>}
        </div>
        {done ? <Check size={25} /> : <KeyRound size={22} />}
      </div>
      {incoming && <p className="dialog-copy">{t('devices:pairConsent')}</p>}
      {session.stage === 'ShowingCode' && (
        <>
          <output className="pairing-code" aria-label={t('devices:codeLabel')}>
            {session.code?.replace(/(.{4})(.{4})/, '$1 $2')}
          </output>
          <p className="field-note">
            {t('devices:codeStatus', { seconds: left, count: session.attemptsLeft })}
          </p>
        </>
      )}
      {session.stage === 'EnteringCode' && (
        <form
          id="pair-code-form"
          onSubmit={(e) => {
            e.preventDefault();
            void run(() => client.submitPairingCode(session.id, code));
          }}
        >
          <label className="form-field">
            <span>{t('devices:oneTimeCode')}</span>
            <input
              className="pair-code-input"
              value={code}
              onChange={(e) => setCode(e.target.value.replace(/\D/g, '').slice(0, 8))}
              inputMode="numeric"
              autoComplete="off"
              autoFocus
              placeholder={t('devices:codePlaceholder')}
              maxLength={9}
            />
          </label>
          <p className="field-note">
            {client.mode === 'preview' && t('preview:demoCode')}
            {t('devices:codeExpiry', { count: left })}
          </p>
        </form>
      )}
      {['Requesting', 'Verifying', 'Saving'].includes(session.stage) && (
        <div className="pair-working" role="status">
          <LoaderCircle size={20} className="is-spinning" />
          <span>{t('common:wait')}</span>
        </div>
      )}
      {session.stage === 'AwaitingApproval' && !incoming && (
        <p className="dialog-copy">{t('devices:approveElsewhere')}</p>
      )}
      {(error || session.error) && (
        <p role="alert" className="form-error">
          {errorText((error || session.error)!)}
        </p>
      )}
      <div className="dialog-actions">
        <Button onClick={close} disabled={busy}>
          {done
            ? t('common:done')
            : failed
              ? t('common:close')
              : incoming
                ? t('devices:reject')
                : t('devices:cancelPairing')}
        </Button>
        {incoming && (
          <Button
            variant="primary"
            disabled={busy || left === 0}
            onClick={() => void run(() => client.acceptPairing(session.id))}
          >
            {t('devices:allowPairing')}
          </Button>
        )}
        {session.stage === 'EnteringCode' && (
          <Button
            variant="primary"
            type="submit"
            form="pair-code-form"
            disabled={busy || code.length !== 8 || left === 0}
          >
            {t('devices:confirmPairing')}
          </Button>
        )}
      </div>
    </Dialog>
  );
}
