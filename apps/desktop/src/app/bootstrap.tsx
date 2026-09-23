import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';
import { isTauri } from '@tauri-apps/api/core';
import {
  createDesktop,
  createNativeDesktop,
  useCommand,
  type DesktopRuntime,
} from '../desktop/api';
import { PreviewBridge } from '../preview/bridge';
import { PreviewToolbar } from '../preview/PreviewToolbar';
import { useI18n } from '../i18n/react';
import { Providers } from './providers';
import { App } from './App';
export async function bootstrap(element: HTMLElement) {
  const preview = isTauri() ? undefined : new PreviewBridge();
  let storage: Storage | undefined;
  try {
    storage = localStorage;
  } catch {
    /* Browser storage may be disabled. */
  }
  const { runtime, desktop } = preview
    ? createDesktop(preview, storage)
    : await createNativeDesktop(storage);
  const root = createRoot(element);
  root.render(
    <StrictMode>
      <Providers desktop={desktop}>
        <App
          footer={
            preview ? <PreviewToolbar client={preview} /> : <DiagnosticToolbar runtime={runtime} />
          }
        />
      </Providers>
    </StrictMode>,
  );
  void runtime.start().catch(() => {
    /* ConnectionGate displays the structured error. */
  });
  const dispose = () => {
    root.unmount();
    void runtime.dispose();
    window.removeEventListener('pagehide', dispose);
  };
  window.addEventListener('pagehide', dispose, { once: true });
  return dispose;
}
function DiagnosticToolbar({ runtime }: { runtime: DesktopRuntime }) {
  const { t } = useI18n();
  const { execute } = useCommand();
  if (!runtime.diagnostic) return null;
  return (
    <footer className="preview-toolbar">
      <span className="preview-toolbar__label">
        <strong>{t('preview:nativeVerification')}</strong>
        <span>{t('preview:nativeEnvironment')}</span>
      </span>
      <div>
        <button onClick={() => execute(() => runtime.probe('copy'))}>
          {t('preview:copyTest')}
        </button>
        <button onClick={() => execute(() => runtime.probe('receive'))}>
          {t('preview:receiveTest')}
        </button>
      </div>
    </footer>
  );
}
