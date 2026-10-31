import { useActionFeedback } from '../shared/action-feedback';
import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';
import { isTauri } from '@tauri-apps/api/core';
import { BackendSubscription } from './backend-subscription';
import { NativeBackend } from '../shared/backend';
import { createQueryClient } from './query-client';
import { useConnection } from '../shared/store';
import { PreviewBackend } from '../preview/backend';
import { PreviewToolbar } from '../preview/PreviewToolbar';
import { useI18n } from '../shared/i18n/react';
import { Providers } from './providers';
import { RouterProvider } from '@tanstack/react-router';
import { createAppRouter } from './router';
export async function bootstrap(element: HTMLElement) {
  const preview = isTauri() ? undefined : new PreviewBackend();
  const motionPreview = ['motion-16', 'business-19'].includes(
    new URLSearchParams(window.location.search).get('mascot') ?? '',
  );
  let storage: Storage | undefined;
  try {
    storage = localStorage;
  } catch {
    /* Browser storage may be disabled. */
  }
  const queryClient = createQueryClient();
  const subscription = new BackendSubscription(
    preview ?? new NativeBackend(),
    queryClient,
    storage,
  );
  const router = createAppRouter({
    motionPreview,
    footer: preview ? (
      <PreviewToolbar client={preview} motionPreview={motionPreview} />
    ) : (
      <DiagnosticToolbar subscription={subscription} />
    ),
  });
  const root = createRoot(element);
  root.render(
    <StrictMode>
      <Providers queryClient={queryClient}>
        <RouterProvider router={router} />
      </Providers>
    </StrictMode>,
  );
  void subscription.start().catch(() => {
    /* ConnectionGate displays the structured error. */
  });
  const dispose = () => {
    root.unmount();
    void subscription.dispose();
    window.removeEventListener('pagehide', dispose);
  };
  window.addEventListener('pagehide', dispose, { once: true });
  return dispose;
}
function DiagnosticToolbar({ subscription }: { subscription: BackendSubscription }) {
  useConnection();
  const { t } = useI18n();
  const { execute } = useActionFeedback();
  if (!subscription.diagnostic) return null;
  return (
    <footer className="preview-toolbar">
      <span className="preview-toolbar__label">
        <strong>{t('preview:nativeVerification')}</strong>
        <span>{t('preview:nativeEnvironment')}</span>
      </span>
      <div>
        <button onClick={() => execute(() => subscription.probe('copy'))}>
          {t('preview:copyTest')}
        </button>
        <button onClick={() => execute(() => subscription.probe('receive'))}>
          {t('preview:receiveTest')}
        </button>
      </div>
    </footer>
  );
}
