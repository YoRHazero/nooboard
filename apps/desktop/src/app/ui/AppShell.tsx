import { useConnection } from '../../shared/store';
import { usePreferenceStore } from '../../features/settings/store';
import { useFeedback } from '../../shared/store/index';
import { useSnapshot } from '../../shared/queries/useBackendSnapshotQuery';
import { useActionFeedback } from '../../shared/action-feedback';
import { useReconnectMutation } from '../../shared/queries/useReconnectMutation';
import { useUpdateSyncSettingsMutation } from '../../features/settings/queries/useUpdateSyncSettingsMutation';
import { useAcknowledgeNavigationMutation } from '../../shared/queries/useAcknowledgeNavigationMutation';
import { AuthenticationPrompt, TerminalNotices } from '../../features/terminal/index';
import { Outlet, useNavigate, useRouterState } from '@tanstack/react-router';
import { useI18n } from '../../shared/i18n/react';
import { useEffect, useRef, type ReactNode } from 'react';
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
  ArrowLeftRight,
} from 'lucide-react';
import { Button } from '../../shared/ui/controls';
import { appVersion } from '../version';

import { UnpairedPrompt } from '../../features/devices/index';
import { PairingPrompt } from '../../features/devices/index';

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
  const acknowledgeNavigationMutation = useAcknowledgeNavigationMutation();
  const updateSyncSettingsMutation = useUpdateSyncSettingsMutation();
  const reconnectMutation = useReconnectMutation();

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
  const { localDevice, settings, host } = useSnapshot();
  const appearance = usePreferenceStore((state) => state.appearance);
  const connectionError = useConnection().error;
  const lastNavigation = useRef<number | null>(null);
  const { execute } = useActionFeedback([
    acknowledgeNavigationMutation,
    updateSyncSettingsMutation,
    reconnectMutation,
  ]);
  const { clearError } = useFeedback();
  useEffect(() => {
    const navigation = host.navigation;
    if (!navigation || lastNavigation.current === navigation.id) return;
    lastNavigation.current = navigation.id;
    void routeNavigate({ to: paths[navigation.page] });
    main.current?.scrollTo({ top: 0 });
    clearError();
    void execute(() => acknowledgeNavigationMutation.mutateAsync([navigation.id]));
  }, [host.navigation, clearError, execute, routeNavigate]);
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
                    execute(() =>
                      updateSyncSettingsMutation.mutateAsync([{ paused: !settings.paused }]),
                    )
                  }
                >
                  {settings.paused ? <Play size={15} /> : <Pause size={15} />}
                  {settings.paused ? t('common:resume') : t('common:pause')}
                </Button>
              )}
            </header>
            <div className="page-content" inert={!!connectionError} key={page}>
              <Outlet />
            </div>
          </div>
        </main>
        <PairingPrompt />
        <UnpairedPrompt />
        <AuthenticationPrompt />
        <TerminalNotices />
        {footer}
      </div>
    </MotionConfig>
  );
}
