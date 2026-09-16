import { useI18n } from '../../i18n/react';
import { useState } from 'react';
import { ArrowUpRight, ChevronDown } from 'lucide-react';
import { useClient, useCommand, useSnapshot } from '../../api/NooboardProvider';
import { canSend, shortNoobId } from '../../api/devices';
import { Button } from '../../ui/controls';
import { characterCount, clockTime } from '../../ui/text';
import { TargetPicker } from '../transfers/TargetPicker';

export function ClipboardPanel() {
  const { t } = useI18n();
  const { current, peers, manualTargets, settings } = useSnapshot();
  const client = useClient();
  const { execute } = useCommand();
  const [choosing, setChoosing] = useState(false);
  const selected = peers.filter((p) => manualTargets.includes(p.noobId));
  const available = selected.filter(canSend).length;
  const source = peers.find((p) => p.noobId === current.sourceNoobId);
  const sourceLabel =
    current.source === 'local'
      ? t('history:localCopy')
      : source
        ? `${source.deviceName} · ${shortNoobId(source.noobId, peers)}`
        : t('home:fromPeer');
  const sendNotice = settings.paused
    ? t('common:paused')
    : !peers.length
      ? t('common:unpaired')
      : !selected.length
        ? t('common:noSelection')
        : available < selected.length
          ? t('home:partialAvailable', { count: available })
          : null;
  return (
    <>
      <p className="stage-panel__meta">
        <span className="clipboard-source" title={sourceLabel}>
          {sourceLabel} · {clockTime(current.copiedAt)}
        </span>
        <span>{t('common:characters', { count: characterCount(current.text) })}</span>
      </p>
      <pre className="clipboard-sheet" tabIndex={0} aria-label={t('home:clipboardText')}>
        {current.text ||
          (current.kind === 'Sensitive'
            ? t('home:sensitive')
            : current.kind === 'Unsupported'
              ? t('home:unsupported')
              : current.kind === 'TooLarge'
                ? t('home:tooLarge')
                : t('common:emptyClipboard'))}
      </pre>
      <div className="clipboard-footer">
        <div className="stage-panel__actions">
          <button
            className="text-button clipboard-targets"
            disabled={!peers.length}
            onClick={() => setChoosing(true)}
            aria-label={t('home:chooseTargets')}
          >
            {selected.length
              ? t('home:sendTo', { count: selected.length })
              : t('home:chooseDevices')}
            <ChevronDown size={13} />
          </button>
          <Button
            variant="primary"
            disabled={!available || settings.paused || !current.text}
            onClick={() => execute(() => client.sendCurrent())}
          >
            {t('home:sendText')}
            <ArrowUpRight size={17} />
          </Button>
        </div>
        {sendNotice && (
          <p className="field-note" role="status">
            {sendNotice}
          </p>
        )}
      </div>
      {choosing && <TargetPicker onClose={() => setChoosing(false)} />}
    </>
  );
}
