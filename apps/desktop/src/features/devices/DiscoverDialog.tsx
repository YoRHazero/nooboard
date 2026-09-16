import { type Problem, problem, errorText, toProblem } from '../../i18n/errors';
import { useI18n } from '../../i18n/react';
import { useEffect, useState } from 'react';
import { ArrowRight, RefreshCw, Radio, Monitor, ChevronDown } from 'lucide-react';
import { useClient, useSnapshot } from '../../api/NooboardProvider';
import { Dialog } from '../../ui/Dialog';
import { Button } from '../../ui/controls';
import { shortNoobId, validAddress } from '../../api/devices';

export function DiscoverDialog({ onClose }: { onClose: () => void }) {
  const { t } = useI18n();
  const client = useClient();
  const { onboarding, peers, settings } = useSnapshot();
  const [refreshing, setRefreshing] = useState(false);
  const [busy, setBusy] = useState(false);
  const [manual, setManual] = useState(false);
  const [address, setAddress] = useState('');
  const [error, setError] = useState<Problem | null>(null);
  const refresh = async () => {
    setRefreshing(true);
    setError(null);
    try {
      await client.discover();
    } catch (e) {
      setError(toProblem(e));
    } finally {
      setTimeout(() => setRefreshing(false), 600);
    }
  };
  useEffect(() => {
    void client.discover().catch((e) => setError(toProblem(e)));
  }, [client]);
  const connect = async (address: string, expected?: string) => {
    if (!validAddress(address)) {
      setError(problem('invalidAddress'));
      return;
    }
    setBusy(true);
    setError(null);
    try {
      await client.beginPairing(address, expected);
      onClose();
    } catch (e) {
      setError(toProblem(e));
    } finally {
      setBusy(false);
    }
  };
  const devices = onboarding?.nearby ?? [];
  return (
    <Dialog title={t('devices:add')} onClose={onClose}>
      <div className="discover-intro">
        <img src="/mascot/icon.png" alt={t('common:bird')} />
        <strong>{t('devices:findBird')}</strong>
      </div>
      <div className="discover-heading">
        <span>
          <Radio size={14} />
          {t('devices:nearby')}
        </span>
        <Button variant="quiet" disabled={refreshing || busy} onClick={() => void refresh()}>
          <RefreshCw size={14} className={refreshing ? 'is-spinning' : ''} />
          {refreshing ? t('devices:searching') : t('devices:refresh')}
        </Button>
      </div>
      <ul className="discover-list" aria-label={t('devices:nearby')}>
        {devices.map((device) => {
          const paired = peers.some((p) => p.noobId === device.noobId);
          return (
            <li key={device.key}>
              <Monitor size={22} />
              <div className="device-name">
                <strong>{device.deviceName}</strong>
                <code title={device.noobId}>{shortNoobId(device.noobId, devices)}</code>
              </div>
              <Button
                disabled={busy || paired}
                onClick={() => void connect(device.addresses[0], device.noobId)}
              >
                {paired ? t('devices:paired') : t('devices:pair')}
                {!paired && <ArrowRight size={14} />}
              </Button>
            </li>
          );
        })}
        {!devices.length && (
          <li className="discover-empty">
            <Radio size={22} />
            <p>
              {t('devices:noneNearby')}
              <span>{t('devices:discoveryHelp')}</span>
            </p>
          </li>
        )}
      </ul>
      {(error || onboarding?.error) && (
        <p role="alert" className="form-error">
          {errorText((error || onboarding?.error)!)}
        </p>
      )}
      <label className="checkbox-label discover-visibility">
        <input
          type="checkbox"
          checked={settings.discoverable ?? true}
          onChange={(e) =>
            void client
              .updateSettings({ discoverable: e.target.checked })
              .catch((e) => setError(toProblem(e)))
          }
        />
        {t('devices:discoverable')}
      </label>
      <div className="discover-manual">
        <button className="text-button" onClick={() => setManual(!manual)} aria-expanded={manual}>
          {t('devices:addByAddress')}
          <ChevronDown size={14} />
        </button>
        {manual && (
          <form
            onSubmit={(e) => {
              e.preventDefault();
              void connect(address.trim());
            }}
          >
            <label className="form-field">
              <span>{t('devices:pairingAddress')}</span>
              <input
                value={address}
                onChange={(e) => setAddress(e.target.value)}
                placeholder="192.168.1.38:24817"
                autoComplete="off"
                spellCheck={false}
              />
            </label>
            <Button variant="primary" disabled={busy || !address.trim()} type="submit">
              {t('devices:startPairing')}
              <ArrowRight size={14} />
            </Button>
          </form>
        )}
      </div>
    </Dialog>
  );
}
