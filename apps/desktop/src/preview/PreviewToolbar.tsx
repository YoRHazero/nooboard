import { useI18n } from '../i18n/react';
import { useState } from 'react';
import { ArrowDownLeft, CopyPlus, FlaskConical, RotateCcw, Unplug } from 'lucide-react';
import { useCommand, useSnapshot } from '../desktop/api';
import { shortNoobId } from '../features/devices/selectors';
import type { PreviewControls } from './bridge';

export function PreviewToolbar({ client }: { client: PreviewControls }) {
  const { t } = useI18n();
  const { execute } = useCommand();
  const { peers, settings } = useSnapshot();
  const [selected, setSelected] = useState('');
  const peer = peers.find((p) => p.noobId === selected) ?? peers[0];
  return (
    <footer className="preview-toolbar">
      <span className="preview-toolbar__label">
        <FlaskConical size={14} />
        <strong>{t('preview:title')}</strong>
        <span>{t('preview:environment')}</span>
      </span>
      <div>
        <button onClick={() => execute(() => client.sampleCopy())}>
          <CopyPlus size={14} />
          {t('preview:copySample')}
        </button>
        <button onClick={() => execute(() => client.sampleFiles())}>
          {t('preview:fileSamples')}
        </button>
        <select
          aria-label={t('preview:eventDevice')}
          value={peer?.noobId ?? ''}
          disabled={!peers.length}
          onChange={(e) => setSelected(e.target.value)}
        >
          {!peers.length && <option value="">{t('preview:noDevices')}</option>}
          {peers.map((p) => (
            <option key={p.noobId} value={p.noobId}>
              {p.deviceName} · {shortNoobId(p.noobId, peers)}
            </option>
          ))}
        </select>
        <button
          disabled={!peer?.online || settings.paused || !settings.receive}
          onClick={() => execute(() => client.sampleReceive(peer?.noobId))}
        >
          <ArrowDownLeft size={14} />
          {t('preview:receiveSample')}
        </button>
        <button
          disabled={!peer}
          onClick={() => execute(() => client.toggleConnection(peer?.noobId))}
        >
          <Unplug size={14} />
          {peer?.online ? t('preview:disconnect') : t('preview:reconnect')}
        </button>
        <button
          disabled={!peer?.online}
          onClick={() => peer && execute(() => client.toggleAccepting(peer.noobId))}
        >
          {peer?.accepting ? t('preview:stopReceiving') : t('preview:startReceiving')}
        </button>
        <button onClick={() => execute(() => client.reset())}>
          <RotateCcw size={14} />
          {t('preview:reset')}
        </button>
      </div>
    </footer>
  );
}
