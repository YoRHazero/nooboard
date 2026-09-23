import { useEffect, useRef, useState } from 'react';
import { ArrowDownLeft, ArrowUpRight, Copy, File, FolderOpen, Image, X } from 'lucide-react';
import { useI18n } from '../../i18n/react';
import { useDesktop, useCommand, useSnapshot } from '../../desktop/api';
import { canSend, shortNoobId } from '../../features/devices/selectors';
import {
  byteLabel,
  contentCancellable,
  contentErrorLabel,
  contentPending,
  contentStageLabel,
} from '../../features/transfers/content';
import { Button } from '../../ui/controls';
import { fullTime } from '../../ui/text';
import { TargetPicker } from './TargetPicker';

export function TransfersPage({ focusKey }: { focusKey?: string }) {
  const { t } = useI18n();
  const { contentTransfers = [], settings, peers, manualTargets } = useSnapshot();
  const client = useDesktop();
  const { execute } = useCommand();
  const [choosing, setChoosing] = useState(false);
  const [filter, setFilter] = useState<'all' | 'sending' | 'receiving'>('all');
  const list = useRef<HTMLUListElement>(null);
  const available = peers.some((p) => manualTargets.includes(p.noobId) && canSend(p));
  useEffect(() => {
    if (focusKey) {
      setFilter('all');
      requestAnimationFrame(() => {
        const row = Array.from(list.current?.children ?? []).find(
          (el) => (el as HTMLElement).dataset.key === focusKey,
        ) as HTMLElement | undefined;
        row?.scrollIntoView({ block: 'center' });
        row?.focus();
      });
    }
  }, [focusKey]);
  const rows = contentTransfers.filter(
    (row) => filter === 'all' || (filter === 'receiving' ? row.incoming : !row.incoming),
  );
  return (
    <div className="transfers-page">
      <div className="transfers-toolbar">
        <p className="field-note">{t('transfers:manualOnly')}</p>
        <div className="transfers-actions">
          <Button variant="quiet" disabled={!peers.length} onClick={() => setChoosing(true)}>
            {t('home:sendTo', { count: manualTargets.length })}
          </Button>
          <Button
            variant="primary"
            disabled={!available || settings.paused}
            onClick={() => execute(() => client.selectFiles())}
          >
            <FolderOpen size={16} />
            {t('transfers:chooseFiles')}
          </Button>
        </div>
      </div>
      <div className="transfer-directory">
        <FolderOpen size={16} />
        <div>
          <span>{t('transfers:receiveDirectory')}</span>
          <p title={settings.receiveDirectory ?? undefined}>
            {settings.receiveDirectory || t('transfers:noDirectory')}
          </p>
        </div>
        <button
          className="text-button"
          onClick={() => execute(() => client.selectReceiveDirectory())}
        >
          {t('transfers:changeDirectory')}
        </button>
      </div>
      <div className="transfer-filter segmented" aria-label={t('transfers:filter')}>
        {(['all', 'sending', 'receiving'] as const).map((value) => (
          <button key={value} aria-pressed={filter === value} onClick={() => setFilter(value)}>
            {t(`transfers:filter${value}`)}
          </button>
        ))}
      </div>
      <ul className="content-transfers" ref={list} aria-label={t('common:transfers')}>
        {rows.map((task) => {
          const pending = contentPending(task.stage);
          const done = task.completedBytes;
          const Icon = task.kind === 'Image' ? Image : File;
          const Direction = task.incoming ? ArrowDownLeft : ArrowUpRight;
          const error = contentErrorLabel(task);
          return (
            <li
              className={`content-transfer ${focusKey === task.key ? 'is-focused' : ''}`}
              key={task.key}
              data-key={task.key}
              tabIndex={-1}
            >
              <span className="content-transfer__icon">
                <Icon size={23} strokeWidth={1.4} />
              </span>
              <div className="content-transfer__body">
                <div className="content-transfer__heading">
                  <strong title={task.names.join(', ')}>
                    {task.kind === 'Image'
                      ? t('transfers:image')
                      : task.names[0] || t('transfers:files')}
                    {task.names.length > 1 && (
                      <span> {t('transfers:moreFiles', { count: task.names.length - 1 })}</span>
                    )}
                  </strong>
                  <span className={`content-transfer__state is-${task.stage}`} role="status">
                    {contentStageLabel(task.stage)}
                  </span>
                </div>
                <div className="content-transfer__meta">
                  <Direction size={13} />
                  <span>
                    {task.deviceName} · {shortNoobId(task.peer, peers)}
                  </span>
                  <time>{fullTime(task.at)}</time>
                </div>
                {pending && (
                  <div className="content-transfer__progress">
                    <progress
                      aria-label={t('transfers:progressLabel', {
                        name: task.names[0] || t('transfers:image'),
                      })}
                      max={task.totalBytes || 1}
                      value={task.totalBytes ? done : undefined}
                    />
                    <span>
                      {byteLabel(done)} / {byteLabel(task.totalBytes)}
                    </span>
                  </div>
                )}
                {!pending && <p className="content-transfer__size">{byteLabel(task.totalBytes)}</p>}
                {error && (
                  <p className={`field-note ${task.stage === 'Failed' ? 'transfer-error' : ''}`}>
                    {error}
                  </p>
                )}
                {(task.savedPaths.length > 0 || task.names.length > 1) && (
                  <details className="content-transfer__files">
                    <summary>
                      {task.savedPaths.length
                        ? t('transfers:savedFiles')
                        : t('transfers:fileList', { count: task.names.length })}
                    </summary>
                    <ul>
                      {(task.savedPaths.length ? task.savedPaths : task.names).map(
                        (name, index) => (
                          <li key={index}>{name}</li>
                        ),
                      )}
                    </ul>
                  </details>
                )}
              </div>
              <div className="content-transfer__actions">
                {contentCancellable(task.stage) && (
                  <Button
                    variant="quiet"
                    onClick={() => execute(() => client.cancelTransfer(task.key))}
                  >
                    <X size={14} />
                    {t('common:cancel')}
                  </Button>
                )}
                {task.incoming && task.savedPaths.length > 0 && !pending && (
                  <Button
                    variant="quiet"
                    onClick={() => execute(() => client.copyReceived(task.key))}
                  >
                    <Copy size={14} />
                    {t('transfers:copyAgain')}
                  </Button>
                )}
              </div>
            </li>
          );
        })}
      </ul>
      {!rows.length && (
        <div className="transfers-empty">
          <ArrowUpRight size={30} strokeWidth={1} />
          <p>{t('transfers:empty')}</p>
          <span>{t('transfers:emptyHelp')}</span>
        </div>
      )}
      {choosing && <TargetPicker onClose={() => setChoosing(false)} />}
    </div>
  );
}
