import { useI18n } from '../../i18n/react';
import { Network } from 'lucide-react';
import type { LocalDevice } from '../../api/contracts';

/** Read-only connection candidates; values always come from the running backend. */
export function LocalAddresses({ device }: { device: LocalDevice }) {
  const { t } = useI18n();
  return (
    <section className="local-addresses" aria-labelledby="local-addresses-title">
      <h3 id="local-addresses-title">
        <Network size={15} />
        {t('devices:localAddress')}
      </h3>
      {device.addressError ? (
        <p className="field-note" role="status">
          {t('devices:addressUnavailable')}
        </p>
      ) : device.addresses.length ? (
        <ul className="local-address-list" aria-label={t('devices:localPairingAddresses')}>
          {device.addresses.map((address) => (
            <li key={address.ip}>
              <code>{address.pairingAddress}</code>
            </li>
          ))}
        </ul>
      ) : (
        <p className="field-note" role="status">
          {t('devices:noAddress')}
        </p>
      )}
    </section>
  );
}
