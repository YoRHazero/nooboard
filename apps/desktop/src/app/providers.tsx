import type { ReactNode } from 'react';
import { DesktopProvider, useConnection, useDesktop, type Desktop } from '../desktop/api';
import { MailboxProvider } from '../features/mailbox/MailboxProvider';
import { useI18n } from '../i18n/react';
import { errorText } from '../i18n/errors';
import { Button } from '../ui/controls';
export function Providers({ desktop, children }: { desktop: Desktop; children: ReactNode }) {
  return (
    <DesktopProvider desktop={desktop}>
      <ConnectionGate>
        <MailboxProvider>{children}</MailboxProvider>
      </ConnectionGate>
    </DesktopProvider>
  );
}
function ConnectionGate({ children }: { children: ReactNode }) {
  const state = useConnection();
  const desktop = useDesktop();
  const { t } = useI18n();
  if (state.hasSnapshot) return children;
  return (
    <main className="startup-screen">
      <img src="/mascot/icon.png" alt={t('common:bird')} />
      <h1>{state.error ? t('common:startFailed') : t('common:starting')}</h1>
      {state.error && (
        <>
          <p role="alert">{errorText(state.error)}</p>
          <Button variant="primary" onClick={() => void desktop.reconnect().catch(() => {})}>
            {t('common:retry')}
          </Button>
        </>
      )}
    </main>
  );
}
