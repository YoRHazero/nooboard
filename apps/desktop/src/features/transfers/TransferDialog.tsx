import { useI18n } from '../../shared/i18n/react';
import type { Activity } from '../../shared/types';
import { batchSummary } from './deliveries';
import { Dialog } from '../../shared/ui/Dialog';
import { DeliveryList } from './DeliveryList';
export function TransferDialog({ activity, onClose }: { activity: Activity; onClose: () => void }) {
  const { t } = useI18n();
  return (
    <Dialog title={t('transfers:result')} onClose={onClose}>
      <p className="dialog-copy">
        {batchSummary(activity)} ·{' '}
        {activity.automatic ? t('common:autoSend') : t('common:manualSend')}
      </p>
      <p className="transfer-excerpt" title={activity.title || t('common:blankText')}>
        {activity.title || t('common:blankText')}
      </p>
      <DeliveryList activity={activity} />
      {activity.targets?.some((target) => target.state === 'unconfirmed') && (
        <p className="field-note">{t('transfers:unconfirmedHelp')}</p>
      )}
    </Dialog>
  );
}
