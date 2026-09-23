import { useI18n } from '../../i18n/react';
import { ArrowUpRight, Check, ChevronDown } from 'lucide-react';
import { useDesktop, useCommand, useSnapshot } from '../../desktop/api';
import { canSend } from '../../features/devices/selectors';
import { batchSummary } from '../../features/transfers/deliveries';
import { Button } from '../../ui/controls';
import { characterCount } from '../../ui/text';
import { DeliveryList } from '../transfers/DeliveryList';

export function SendSelection() {
  const { t } = useI18n();
  const { peers, manualTargets, current, settings, activities } = useSnapshot();
  const client = useDesktop();
  const { execute } = useCommand();
  const selected = peers.filter((p) => manualTargets.includes(p.noobId));
  const available = selected.filter(canSend).length;
  const latest = activities.find((a) => a.kind === 'sent');
  return (
    <>
      <section className="send-selection" aria-label={t('common:manualSend')}>
        <div className="send-selection__copy">
          <span className="send-selection__title">
            <Check size={15} />
            {selected.length
              ? t('devices:selected', { count: selected.length })
              : t('devices:chooseManual')}
          </span>
          <p>
            {settings.paused
              ? t('common:paused')
              : !selected.length
                ? t('common:noSelection')
                : available < selected.length
                  ? t('devices:partialAvailable', {
                      available: available,
                      unavailable: selected.length - available,
                    })
                  : t('devices:clipboardSize', { count: characterCount(current.text) })}
          </p>
        </div>
        <Button
          variant="primary"
          disabled={!available || settings.paused || !current.text}
          onClick={() => execute(() => client.sendCurrent())}
        >
          {t('devices:sendTo', { count: selected.length })}
          <ArrowUpRight size={17} />
        </Button>
      </section>
      {latest && (
        <details className="batch-receipt" key={latest.id} open>
          <summary>
            <span>
              {t('devices:recentSend')}
              <small>{latest.automatic ? t('common:automatic') : t('common:manual')}</small>
            </span>
            <span className={`batch-receipt__result is-${latest.state}`} aria-live="polite">
              {batchSummary(latest)}
              <ChevronDown size={14} />
            </span>
          </summary>
          <DeliveryList activity={latest} />
        </details>
      )}
    </>
  );
}
