import { useSnapshot } from '../../shared/queries/useBackendSnapshotQuery';
import { useActionFeedback } from '../../shared/action-feedback';
import { useUpdateSyncSettingsMutation } from '../settings/queries/useUpdateSyncSettingsMutation';
import { useI18n } from '../../shared/i18n/react';
import { ArrowUpRight } from 'lucide-react';
import { homeStatus, latestTransfer, transferResult } from './status';

export function HomeOverview({ onDevices }: { onDevices: () => void }) {
  const updateSyncSettingsMutation = useUpdateSyncSettingsMutation();

  const { t } = useI18n();
  const settings = useSnapshot((s) => s.settings);
  const peers = useSnapshot((s) => s.peers);
  const activities = useSnapshot((s) => s.activities);
  const { execute } = useActionFeedback([updateSyncSettingsMutation]);
  const status = homeStatus({ settings, peers });
  const latest = latestTransfer({ peers, activities });
  return (
    <div className="home-overview">
      <div className="home-overview__status" role="status">
        <span className={`home-status is-${status.tone}`}>
          <i />
          {status.label}
        </span>
        {status.detail && <span className="home-overview__detail">{status.detail}</span>}
        {!settings.receive && (
          <span className="home-overview__detail">{t('common:receiveOff')}</span>
        )}
      </div>
      {!peers.length ? (
        <button className="text-button" onClick={onDevices}>
          {t('devices:add')}
          <ArrowUpRight size={14} />
        </button>
      ) : settings.paused ? (
        <button
          className="text-button"
          onClick={() => execute(() => updateSyncSettingsMutation.mutateAsync([{ paused: false }]))}
        >
          {t('common:resume')}
          <ArrowUpRight size={14} />
        </button>
      ) : !peers.some((p) => p.online) ? (
        <button className="text-button" onClick={onDevices}>
          {t('home:checkConnection')}
          <ArrowUpRight size={14} />
        </button>
      ) : (
        latest && (
          <span className={`home-receipt is-${latest.state}`} role="status">
            {t('home:recentResult', { result: transferResult(latest) })}
          </span>
        )
      )}
    </div>
  );
}
