import { useI18n } from '../../i18n/react';
import { useState } from 'react';
import { ArrowUpRight, ChevronDown, File, Image } from 'lucide-react';
import { useDesktop, useCommand, useSnapshot } from '../../desktop/api';
import { canSend, shortNoobId } from '../../features/devices/selectors';
import { Button } from '../../ui/controls';
import { characterCount, clockTime } from '../../ui/text';
import { TargetPicker } from '../transfers/TargetPicker';

export function ClipboardPanel() {
  const { t } = useI18n();
  const { current, peers, manualTargets, settings } = useSnapshot();
  const client = useDesktop();
  const { execute } = useCommand();
  const [choosing, setChoosing] = useState(false);
  const selected = peers.filter((p) => manualTargets.includes(p.noobId));
  const available = selected.filter(canSend).length;
  const image = current.kind === 'Image';
  const files = current.kind === 'Files';
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
        <span>
          {image
            ? t('transfers:image')
            : files
              ? t('transfers:fileList', { count: current.files?.length ?? 0 })
              : t('common:characters', { count: characterCount(current.text) })}
        </span>
      </p>
      {image ? (
        <div className="clipboard-image">
          {current.preview ? (
            <img src={current.preview} alt={t('transfers:imagePreview')} />
          ) : (
            <Image size={38} strokeWidth={1} />
          )}
          <span>
            {current.imageWidth && current.imageHeight
              ? `${current.imageWidth} × ${current.imageHeight}`
              : t('transfers:image')}
          </span>
        </div>
      ) : files ? (
        <ul className="clipboard-files" aria-label={t('transfers:files')}>
          {current.files?.map((name, index) => (
            <li key={index}>
              <File size={17} />
              <span>{name}</span>
            </li>
          ))}
        </ul>
      ) : (
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
      )}
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
            disabled={!available || settings.paused || (!current.text && !image && !files)}
            onClick={() => execute(() => client.sendCurrent())}
          >
            {image
              ? t('transfers:sendImage')
              : files
                ? t('transfers:sendFiles')
                : t('home:sendText')}
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
