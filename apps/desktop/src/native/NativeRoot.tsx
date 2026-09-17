import { type Problem, errorText, toProblem } from '../i18n/errors';
import { useI18n } from '../i18n/react';
import { useEffect, useState } from 'react';
import { App } from '../app/App';
import { NooboardProvider, useCommand } from '../api/NooboardProvider';
import { MailboxProvider } from '../features/mailbox/MailboxProvider';
import { Button } from '../ui/controls';
import type { NativeClient } from './NativeClient';
export function NativeRoot({ client }: { client: NativeClient }) {
  const { t } = useI18n();
  const [ready, setReady] = useState(false);
  const [error, setError] = useState<Problem | null>(null);
  const [attempt, setAttempt] = useState(0);
  useEffect(() => {
    let active = true;
    setError(null);
    void client
      .connect()
      .then(() => {
        if (active) setReady(true);
      })
      .catch((reason) => {
        if (active) setError(toProblem(reason));
      });
    return () => {
      active = false;
      client.dispose();
    };
  }, [client, attempt]);
  if (!ready)
    return (
      <main className="startup-screen">
        <img src="/mascot/icon.png" alt={t('common:bird')} />
        <h1>{error ? t('common:startFailed') : t('common:starting')}</h1>
        {error && <p role="alert">{errorText(error)}</p>}
        {error && (
          <Button variant="primary" onClick={() => setAttempt((v) => v + 1)}>
            {t('common:retry')}
          </Button>
        )}
      </main>
    );
  return (
    <NooboardProvider client={client}>
      <MailboxProvider>
        <App
          onReconnect={() => client.connect()}
          onNavigateHandled={(id) => client.acknowledgeNavigation(id)}
          footer={client.diagnostic ? <DiagnosticToolbar client={client} /> : undefined}
        />
      </MailboxProvider>
    </NooboardProvider>
  );
}
function DiagnosticToolbar({ client }: { client: NativeClient }) {
  const { t } = useI18n();
  const { execute } = useCommand();
  return (
    <footer className="preview-toolbar">
      <span className="preview-toolbar__label">
        <strong>{t('preview:nativeVerification')}</strong>
        <span>{t('preview:nativeEnvironment')}</span>
      </span>
      <div>
        <button onClick={() => execute(() => client.probe('copy'))}>{t('preview:copyTest')}</button>
        <button onClick={() => execute(() => client.probe('receive'))}>
          {t('preview:receiveTest')}
        </button>
      </div>
    </footer>
  );
}
