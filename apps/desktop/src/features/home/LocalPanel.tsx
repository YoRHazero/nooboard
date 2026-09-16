import { useI18n } from '../../i18n/react';
import { ArrowUpRight, MousePointer2, Pause, Play, Zap } from 'lucide-react';
import { useClient, useCommand, useSnapshot } from '../../api/NooboardProvider';
import { Button } from '../../ui/controls';

export function LocalPanel({ onSettings }: { onSettings: () => void }) {
  const { t } = useI18n();
  const { settings } = useSnapshot();
  const client = useClient();
  const { execute } = useCommand();
  return (
    <>
      <div className="local-mode">
        <div className="segmented" aria-label={t('home:sendMode')}>
          <button
            aria-pressed={settings.mode === 'manual'}
            onClick={() => execute(() => client.updateSettings({ mode: 'manual' }))}
          >
            <MousePointer2 size={14} />
            {t('common:manualSend')}
          </button>
          <button
            aria-pressed={settings.mode === 'automatic'}
            onClick={() => execute(() => client.updateSettings({ mode: 'automatic' }))}
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
        <Button onClick={() => execute(() => client.updateSettings({ paused: !settings.paused }))}>
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
