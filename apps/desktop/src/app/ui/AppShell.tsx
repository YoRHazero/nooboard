import { AuthenticationPrompt } from '../../features/terminal/api';
import { Outlet, useNavigate, useRouterState } from '@tanstack/react-router';
import { errorText } from '../../shared/i18n/errors';
import { useI18n } from '../../shared/i18n/react';
import { useEffect, useRef, useState, type ReactNode } from 'react';
import { MotionConfig, motion } from 'motion/react';
import {
  House,
  TerminalSquare,
  History,
  Laptop,
  Monitor,
  Pause,
  Play,
  Settings2,
  X,
  ArrowLeftRight,
} from 'lucide-react';
import { useDesktop, useCommand, useSnapshot, useFeedback } from '../../client/react';
import { Button, IconButton } from '../../shared/ui/controls';
import { appVersion } from '../version';

import { UnpairedPrompt } from '../../features/devices/api';
import { PairingPrompt } from '../../features/devices/api';

const paths = {
  home: '/',
  history: '/history',
  transfers: '/transfers',
  devices: '/devices',
  settings: '/settings',
  terminal: '/terminal',
} as const;
type Page = keyof typeof paths;

export function AppShell({ footer }: { footer?: ReactNode }) {
  const { t } = useI18n();
  const pages = {
    home: { title: t('common:home'), icon: House },
    history: { title: t('common:history'), icon: History },
    transfers: { title: t('common:transfers'), icon: ArrowLeftRight },
    terminal: { title: t('terminal:title'), icon: TerminalSquare },
    devices: { title: t('common:devices'), icon: Monitor },
    settings: { title: t('common:settings'), icon: Settings2 },
  };
  const routeNavigate = useNavigate();
  const pathname = useRouterState({ select: (state) => state.location.pathname });
  const page = (Object.keys(paths) as Page[]).find((key) => paths[key] === pathname) ?? 'home';
  const main = useRef<HTMLElement>(null);
  const { localDevice, settings, notice, connectionError, host, appearance } = useSnapshot();
  const lastNavigation = useRef<number | null>(null);
  const [dismissed, setDismissed] = useState<string>();
  const client = useDesktop();
  const { execute } = useCommand();
  const { error, clearError } = useFeedback();
  useEffect(() => {
    const navigation = host.navigation;
    if (!navigation || lastNavigation.current === navigation.id) return;
    lastNavigation.current = navigation.id;
    void routeNavigate({ to: paths[navigation.page] });
    main.current?.scrollTo({ top: 0 });
    clearError();
    void execute(() => client.acknowledgeNavigation(navigation.id));
  }, [host.navigation, client, clearError, execute, routeNavigate]);
  useEffect(() => {
    document.title = `nooboard · ${pages[page].title}`;
  }, [page, t]);
  useEffect(() => {
    main.current?.scrollTo({ top: 0 });
    clearError();
  }, [pathname, clearError]);
  const navigate = (next: Page) => {
    void routeNavigate({ to: paths[next] });
    main.current?.scrollTo({ top: 0 });
    clearError();
  };
  return (
    <MotionConfig reducedMotion={appearance.reducedMotion ? 'always' : 'user'}>
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
          <span className="sidebar-version">nooboard / {host.version ?? appVersion}</span>
        </aside>
        <main
          className={`app-main ${page === 'home' ? 'app-main--home' : page === 'terminal' ? 'app-main--terminal' : ''}`}
          ref={main}
        >
          <div className="main-inner">
            <header className="page-header">
              <h1>{pages[page].title}</h1>
              {page !== 'home' && page !== 'terminal' && (
                <Button
                  variant="quiet"
                  disabled={!!connectionError}
                  className={settings.paused ? 'resume-button' : ''}
                  onClick={() =>
                    execute(() => client.updateSyncSettings({ paused: !settings.paused }))
                  }
                >
                  {settings.paused ? <Play size={15} /> : <Pause size={15} />}
                  {settings.paused ? t('common:resume') : t('common:pause')}
                </Button>
              )}
            </header>
            {connectionError && (
              <div className="error-banner" role="alert">
                <span>{errorText(connectionError)}</span>
                {
                  <Button onClick={() => execute(() => client.reconnect())}>
                    {t('common:reconnect')}
                  </Button>
                }
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
              <Outlet />
            </div>
          </div>
        </main>
        <PairingPrompt />
        <UnpairedPrompt />
        <AuthenticationPrompt />
        {footer}
      </div>
    </MotionConfig>
  );
}
