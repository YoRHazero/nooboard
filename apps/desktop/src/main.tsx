import { initializeLanguage } from './i18n/language';
import React from 'react';
import { isTauri } from '@tauri-apps/api/core';
import { NativeClient } from './native/NativeClient';
import { NativeRoot } from './native/NativeRoot';
import ReactDOM from 'react-dom/client';
import { App } from './app/App';
import { NooboardProvider } from './api/NooboardProvider';
import { PreviewClient } from './preview/PreviewClient';
import { MailboxProvider } from './features/mailbox/MailboxProvider';
import './app/theme.css';
import './app/layout.css';
import './ui/controls.css';
import './features/home/home.css';
import './features/home/panels.css';
import './features/mascot/mascot.css';
import './features/mailbox/mailbox.css';
import './features/history/history.css';
import './features/devices/devices.css';
import './features/transfers/transfers.css';
import './features/settings/settings.css';

async function bootstrap() {
  const disposeLanguage = await initializeLanguage(isTauri());
  if (import.meta.hot) import.meta.hot.dispose(disposeLanguage);
  const root = ReactDOM.createRoot(document.getElementById('root')!);
  if (isTauri()) {
    // A menu failure must not prevent access to settings and the clipboard.
    void import('./native/menu')
      .then(({ initializeMenu }) => initializeMenu())
      .then((dispose) => {
        if (import.meta.hot) import.meta.hot.dispose(dispose);
      })
      .catch((error: unknown) => console.error('Native menu initialization failed', error));
    const native = new NativeClient();
    root.render(
      <React.StrictMode>
        <NativeRoot client={native} />
      </React.StrictMode>,
    );
    if (import.meta.hot) import.meta.hot.dispose(() => native.dispose());
  } else {
    const preview = new PreviewClient();
    root.render(
      <React.StrictMode>
        <NooboardProvider client={preview}>
          <MailboxProvider>
            <App preview={preview} />
          </MailboxProvider>
        </NooboardProvider>
      </React.StrictMode>,
    );
    if (import.meta.hot) import.meta.hot.dispose(() => preview.dispose());
  }
}
void bootstrap();
