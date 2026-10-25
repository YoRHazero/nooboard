import { Select, SelectItem } from '../shared/ui/Select';
import { useI18n } from '../shared/i18n/react';
import { useState } from 'react';
import { ArrowDownLeft, CopyPlus, FlaskConical, RotateCcw } from 'lucide-react';
import { useCommand, useSnapshot } from '../client/react';
import { shortNoobId } from '../features/devices/api';
import type { PreviewControls } from './bridge';
import type { PeerConnectionState } from '../client/bridge/generated';

export function PreviewToolbar({
  client,
  motionPreview,
}: {
  client: PreviewControls;
  motionPreview?: boolean;
}) {
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
        <a className="text-button" href={motionPreview ? '?' : '?mascot=business-19'}>
          {t(motionPreview ? 'preview:originalStage' : 'preview:birdMotion')}
        </a>
        <button onClick={() => execute(() => client.sampleCopy())}>
          <CopyPlus size={14} />
          {t('preview:copySample')}
        </button>
        <button onClick={() => execute(() => client.sampleFiles())}>
          {t('preview:fileSamples')}
        </button>
        <Select
          aria-label={t('preview:eventDevice')}
          value={peer?.noobId ?? ''}
          disabled={!peers.length}
          onValueChange={(e) => setSelected(e)}
        >
          {!peers.length && <SelectItem value="">{t('preview:noDevices')}</SelectItem>}
          {peers.map((p) => (
            <SelectItem key={p.noobId} value={p.noobId}>
              {p.deviceName} · {shortNoobId(p.noobId, peers)}
            </SelectItem>
          ))}
        </Select>
        <button
          disabled={!peer?.online || settings.paused || !settings.receive}
          onClick={() => execute(() => client.sampleReceive(peer?.noobId))}
        >
          <ArrowDownLeft size={14} />
          {t('preview:receiveSample')}
        </button>
        <Select
          aria-label={t('preview:connectionScenario')}
          value={peer?.connectionState ?? 'offline'}
          disabled={!peer}
          onValueChange={(e) =>
            peer && execute(() => client.setPeerState(peer.noobId, e as PeerConnectionState))
          }
        >
          <SelectItem value="connected">{t('common:connected')}</SelectItem>
          <SelectItem value="paused">{t('devices:pausedState')}</SelectItem>
          <SelectItem value="offline">{t('common:offline')}</SelectItem>
          <SelectItem value="unconnected">{t('devices:unconnected')}</SelectItem>
          <SelectItem value="unpaired">{t('devices:unpairedState')}</SelectItem>
        </Select>
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
