import { t } from '../../i18n/index';
import type { Activity, DeliveryState } from '../../desktop/api';
export function deliveryLabel(state: DeliveryState): string {
  const labels: Record<DeliveryState, string> = {
    queued: t('common:queued'),
    sending: t('common:sending'),
    awaitingReceipt: t('common:awaitingReceipt'),
    applied: t('common:received'),
    rejected: t('common:notReceived'),
    unconfirmed: t('common:unconfirmed'),
    offline: t('common:unavailable'),
    cancelled: t('common:cancelled'),
    superseded: t('common:superseded'),
    queueFull: t('common:queueFull'),
  };
  return labels[state];
}
export { deliveryPending, batchState } from '../../desktop/api';
export function batchSummary(activity: Activity) {
  const targets = activity.targets ?? [];
  const success = targets.filter((d) => d.state === 'applied').length;
  if (activity.state === 'pending')
    return success
      ? t('common:partReceived', { received: success, total: targets.length })
      : t('common:awaitingReceipt');
  if (activity.state === 'partial')
    return t('common:partReceived', { received: success, total: targets.length });
  if (activity.state === 'applied') return t('common:allReceived', { count: targets.length });
  if (activity.state === 'unconfirmed') return t('common:unconfirmed');
  if (activity.state === 'cancelled') return t('common:cancelledOrReplaced');
  return t('common:notDelivered');
}
