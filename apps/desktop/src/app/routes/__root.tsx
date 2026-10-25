import { createRootRouteWithContext, Link } from '@tanstack/react-router';
import { AppShell } from '../ui/AppShell';
import type { AppRouterContext } from '../routerContext';
import { useI18n } from '../../shared/i18n/react';
export const Route = createRootRouteWithContext<AppRouterContext>()({
  component: Root,
  notFoundComponent: NotFound,
});
function Root() {
  const { footer } = Route.useRouteContext();
  return <AppShell footer={footer} />;
}
function NotFound() {
  const { t } = useI18n();
  return (
    <div className="empty-state">
      <p>{t('common:pageNotFound')}</p>
      <Link to="/">{t('common:home')}</Link>
    </div>
  );
}
