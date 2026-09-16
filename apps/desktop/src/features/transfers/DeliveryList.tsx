import { useI18n } from '../../i18n/react';
import type { Activity } from '../../api/contracts';
import { deliveryLabel } from '../../api/deliveries';
import { DeviceName } from '../devices/DeviceName';
export function DeliveryList({ activity }: { activity: Activity }) {
  const { t } = useI18n();
  const targets = activity.targets ?? [];
  return (
    <ul className="delivery-list" aria-label={t('transfers:deliveryList')}>
      {targets.map((target) => (
        <li key={target.noobId}>
          <DeviceName device={target} identities={targets} />
          <span className={`delivery-state is-${target.state}`}>{deliveryLabel(target.state)}</span>
        </li>
      ))}
    </ul>
  );
}
