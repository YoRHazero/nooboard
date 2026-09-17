import { errorText } from '../i18n/errors';
import { useI18n } from '../i18n/react';
import { useEffect, useRef, useState, type ReactNode } from 'react';
import { MotionConfig, motion } from 'motion/react';
import {
  House,
  History,
  Laptop,
  Monitor,
  Pause,
  Play,
  Settings2,
  X,
  ArrowLeftRight,
} from 'lucide-react';
import { TransfersPage } from '../features/transfers/TransfersPage';
import { useClient, useCommand, useSnapshot } from '../api/NooboardProvider';
import { Button, IconButton } from '../ui/controls';
import { HomePage } from '../features/home/HomePage';
import { HistoryPage } from '../features/history/HistoryPage';
import { DevicesPage } from '../features/devices/DevicesPage';
import { SettingsPage } from '../features/settings/SettingsPage';
import { PreviewToolbar } from '../preview/PreviewToolbar';
import type { PreviewClient } from '../preview/PreviewClient';

import { PairingPrompt } from '../features/devices/PairingPrompt';

type Page = 'home' | 'history' | 'transfers' | 'devices' | 'settings';

export function App({
  preview,
  footer,
  onReconnect,
}: {
  preview?: PreviewClient;
  footer?: ReactNode;
  onReconnect?: () => Promise<void>;
}) {
  const { t } = useI18n();
  const pages = {
    home: { title: t('common:home'), icon: House },
    history: { title: t('common:history'), icon: History },
    transfers: { title: t('common:transfers'), icon: ArrowLeftRight },
    devices: { title: t('common:devices'), icon: Monitor },
    settings: { title: t('common:settings'), icon: Settings2 },
  };
  const [page, setPage] = useState<Page>('home');
  const [transferKey, setTransferKey] = useState<string>();
  const main = useRef<HTMLElement>(null);
  const { localDevice, settings, notice, connectionError } = useSnapshot();
  const [dismissed, setDismissed] = useState<string>();
  const client = useClient();
  const { error, clearError, execute } = useCommand();
  useEffect(() => {
    document.documentElement.dataset.theme = settings.theme;
    document.documentElement.dataset.reducedMotion = String(settings.reducedMotion);
  }, [settings.theme, settings.reducedMotion]);
  useEffect(() => {
    document.title = `nooboard · ${pages[page].title}`;
  }, [page, t]);
  const navigate = (next: Page) => {
    setPage(next);
    main.current?.scrollTo({ top: 0 });
    clearError();
  };
  return (
    <MotionConfig reducedMotion={settings.reducedMotion ? 'always' : 'user'}>
      <div className="app-shell">
        <aside className="sidebar">
          <a
            className="brand"
            href="#"
            onClick={(event) => {
              event.preventDefault();
              navigate('home');
            }}
            aria-label={t('common:homeLink')}
          >
            <img src="/mascot/icon.png" alt="" />
            <span>nooboard</span>
          </a>
          <nav aria-label={t('common:navigation')}>
            {(Object.keys(pages) as Page[]).map((key) => {
              const Icon = pages[key].icon;
              return (
                <button
                  key={key}
                  className={`nav-item ${key === page ? 'is-active' : ''} ${key === 'settings' ? 'nav-item--settings' : ''}`}
                  aria-current={page === key ? 'page' : undefined}
                  aria-label={pages[key].title}
                  onClick={() => navigate(key)}
                >
                  <Icon size={19} strokeWidth={1.7} />
                  <span>{pages[key].title}</span>
                  {key === page && (
                    <motion.span
                      className="nav-active-marker"
                      layoutId="nav-marker"
                      transition={{ duration: 0.2 }}
                    />
                  )}
                </button>
              );
            })}
          </nav>
          <div className="sidebar-device">
            <span className="sidebar-device__icon">
              <Laptop size={19} />
            </span>
            <div>
              <strong>{localDevice.deviceName}</strong>
              <span>{t('common:thisDevice')}</span>
            </div>
          </div>
          <span className="sidebar-version">nooboard / 0.1</span>
        </aside>
        <main className={`app-main ${page === 'home' ? 'app-main--home' : ''}`} ref={main}>
          <div className="main-inner">
            <header className="page-header">
              <h1>{pages[page].title}</h1>
              {page !== 'home' && (
                <Button
                  variant="quiet"
                  disabled={!!connectionError}
                  className={settings.paused ? 'resume-button' : ''}
                  onClick={() => execute(() => client.updateSettings({ paused: !settings.paused }))}
                >
                  {settings.paused ? <Play size={15} /> : <Pause size={15} />}
                  {settings.paused ? t('common:resume') : t('common:pause')}
                </Button>
              )}
            </header>
            {connectionError && (
              <div className="error-banner" role="alert">
                <span>{errorText(connectionError)}</span>
                {onReconnect && (
                  <Button onClick={() => execute(onReconnect)}>{t('common:reconnect')}</Button>
                )}
              </div>
            )}
            {notice && dismissed !== notice.id && (
              <div className="error-banner" role="alert">
                <span>{errorText(notice.message)}</span>
                <IconButton label={t('common:dismiss')} onClick={() => setDismissed(notice.id)}>
                  <X size={15} />
                </IconButton>
              </div>
            )}
            {error && (
              <div className="error-banner" role="alert">
                <span>{errorText(error)}</span>
                <IconButton label={t('common:dismiss')} onClick={clearError}>
                  <X size={15} />
                </IconButton>
              </div>
            )}
            <div className="page-content" inert={!!connectionError} key={page}>
              {page === 'home' && (
                <HomePage
                  onDevices={() => navigate('devices')}
                  onSettings={() => navigate('settings')}
                  onTransfers={(key) => {
                    setTransferKey(key);
                    navigate('transfers');
                  }}
                />
              )}
              {page === 'history' && <HistoryPage />}
              {page === 'transfers' && <TransfersPage focusKey={transferKey} />}
              {page === 'devices' && <DevicesPage />}
              {page === 'settings' && <SettingsPage />}
            </div>
          </div>
        </main>
        <PairingPrompt />
        {preview ? <PreviewToolbar client={preview} /> : footer}
      </div>
    </MotionConfig>
  );
}
