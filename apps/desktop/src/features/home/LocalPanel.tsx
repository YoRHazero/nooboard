import { useSnapshot } from '../../shared/queries/useBackendSnapshotQuery';
import { useActionFeedback } from '../../shared/action-feedback';
import { useUpdateSyncSettingsMutation } from '../settings/queries/useUpdateSyncSettingsMutation';
import { useI18n } from '../../shared/i18n/react';
import { ArrowUpRight, MousePointer2, Pause, Play, Zap } from 'lucide-react';
import { Button } from '../../shared/ui/controls';

export function LocalPanel({ onSettings }: { onSettings: () => void }) {
  const updateSyncSettingsMutation = useUpdateSyncSettingsMutation();

  const { t } = useI18n();
  const { settings } = useSnapshot();
  const { execute } = useActionFeedback([updateSyncSettingsMutation]);
  return (
    <>
      <div className="local-mode">
        <p className="field-note">{t('home:sendMode')}</p>
        <div className="segmented" aria-label={t('home:sendMode')}>
          <button
            aria-pressed={settings.mode === 'manual'}
            onClick={() =>
              execute(() => updateSyncSettingsMutation.mutateAsync([{ mode: 'manual' }]))
            }
          >
            <MousePointer2 size={14} />
            {t('common:manualSend')}
          </button>
          <button
            aria-pressed={settings.mode === 'automatic'}
            onClick={() =>
              execute(() => updateSyncSettingsMutation.mutateAsync([{ mode: 'automatic' }]))
            }
          >
            <Zap size={14} />
            {t('common:autoSync')}
          </button>
        </div>
      </div>
      <div className="local-pause">
        <div>
          <h3>{settings.paused ? t('common:paused') : t('home:syncOn')}</h3>
        </div>
        <Button
          onClick={() =>
            execute(() => updateSyncSettingsMutation.mutateAsync([{ paused: !settings.paused }]))
          }
        >
          {settings.paused ? <Play size={14} /> : <Pause size={14} />}
          {settings.paused ? t('common:resume') : t('common:pause')}
        </Button>
      </div>
      <button className="text-button panel-link" onClick={onSettings}>
        {t('home:moreSettings')}
        <ArrowUpRight size={14} />
      </button>
    </>
  );
}
