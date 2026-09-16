import { errorText } from '../../i18n/errors';
import { useI18n } from '../../i18n/react';
import { useState } from 'react';
import { ArrowDownLeft, Check, Copy, FileText, History, Search, Trash2, X } from 'lucide-react';
import { useClient, useCommand, useSnapshot } from '../../api/NooboardProvider';
import type { EntryId, HistoryQuery } from '../../api/contracts';
import { useHistory } from './useHistory';
import { Button, EmptyState, IconButton } from '../../ui/controls';
import { Dialog } from '../../ui/Dialog';
import { characterCount, clockTime, firstLine, fullTime } from '../../ui/text';

export function HistoryPage() {
  const { t } = useI18n();
  const { current, settings } = useSnapshot();
  const client = useClient();
  const { execute } = useCommand();
  const [search, setSearch] = useState('');
  const [source, setSource] = useState<HistoryQuery['source']>('all');
  const [selected, setSelected] = useState<EntryId | null>(null);
  const [confirmClear, setConfirmClear] = useState(false);
  const [copied, setCopied] = useState<string | null>(null);
  const { rows, hasMore, loading, error, more, retry } = useHistory(search, source);
  const item = rows.find((row) => row.id === selected) ?? rows[0];
  return (
    <section className="history-page">
      <div className="history-toolbar">
        <label className="search-field">
          <Search size={17} />
          <input
            aria-label={t('history:searchLabel')}
            placeholder={t('history:searchPlaceholder')}
            value={search}
            onChange={(event) => setSearch(event.target.value)}
          />
          {search && (
            <IconButton label={t('history:clearSearch')} onClick={() => setSearch('')}>
              <X size={14} />
            </IconButton>
          )}
        </label>
        <Button
          variant="quiet"
          disabled={!rows.length || loading}
          onClick={() => setConfirmClear(true)}
        >
          <Trash2 size={15} />
          {t('history:clear')}
        </Button>
      </div>
      <div className="history-filter">
        <div className="text-tabs" aria-label={t('history:source')}>
          {[
            ['all', t('history:all')],
            ['local', t('history:localCopy')],
            ['remote', t('history:receivedText')],
          ].map(([value, label]) => (
            <button
              key={value}
              aria-pressed={source === value}
              onClick={() => setSource(value as HistoryQuery['source'])}
            >
              {label}
            </button>
          ))}
        </div>
        <span className="small muted">
          {loading
            ? t('common:loading')
            : t(hasMore ? 'history:recordCountMore' : 'history:recordCount', {
                count: rows.length,
              })}
        </span>
      </div>
      {error && (
        <div className="error-banner" role="alert">
          <span>{errorText(error)}</span>
          <Button onClick={retry}>{t('common:retry')}</Button>
        </div>
      )}
      {!rows.length ? (
        <EmptyState
          icon={search ? <Search size={28} /> : <History size={28} />}
          title={
            loading ? t('history:loading') : search ? t('history:noResults') : t('history:empty')
          }
        >
          {loading
            ? null
            : search
              ? t('history:searchHelp')
              : settings.history
                ? null
                : t('history:disabled')}
        </EmptyState>
      ) : (
        <div className="history-workspace">
          <div className="history-index" aria-label={t('history:entries')}>
            {rows.map((row) => (
              <button
                key={row.id}
                className={`history-row ${item?.id === row.id ? 'is-selected' : ''}`}
                aria-pressed={item?.id === row.id}
                onClick={() => {
                  setSelected(row.id);
                  setCopied(null);
                }}
              >
                <span className="history-row__icon">
                  {row.source === 'remote' ? <ArrowDownLeft size={17} /> : <FileText size={17} />}
                </span>
                <span className="history-row__content">
                  <strong>{firstLine(row.text)}</strong>
                  <span>{row.text.replaceAll('\n', ' ')}</span>
                  <small>
                    {row.source === 'remote' ? t('history:peer') : t('history:local')}
                    <span>·</span>
                    {clockTime(row.copiedAt)}
                  </small>
                </span>
              </button>
            ))}
            {hasMore && (
              <Button
                variant="quiet"
                className="history-more"
                disabled={loading}
                onClick={() => void more()}
              >
                {loading ? t('common:loading') : t('history:loadMore')}
              </Button>
            )}
          </div>
          {item && (
            <article className="history-detail">
              <div className="history-detail__meta">
                <span>
                  {item.source === 'remote' ? t('history:receivedText') : t('history:localCopy')}
                </span>
                <time>{fullTime(item.copiedAt)}</time>
              </div>
              <pre>{item.text}</pre>
              <footer>
                <span className="small muted">
                  {t('common:characters', { count: characterCount(item.text) })}
                  {current.text === item.text ? ` · ${t('home:currentClipboard')}` : ''}
                </span>
                <div className="button-row">
                  <IconButton
                    label={t('history:delete')}
                    onClick={() => execute(() => client.deleteHistory(item.id))}
                  >
                    <Trash2 size={16} />
                  </IconButton>
                  <Button
                    variant="primary"
                    onClick={() =>
                      execute(async () => {
                        await client.copyHistory(item.id);
                        setCopied(item.text);
                      })
                    }
                  >
                    {copied === item.text ? <Check size={16} /> : <Copy size={16} />}
                    {copied === item.text ? t('common:copied') : t('history:copyAgain')}
                  </Button>
                </div>
              </footer>
            </article>
          )}
        </div>
      )}
      {confirmClear && (
        <Dialog title={t('history:clearTitle')} onClose={() => setConfirmClear(false)}>
          <p className="dialog-copy">{t('history:clearEffect')}</p>
          <div className="dialog-actions">
            <Button onClick={() => setConfirmClear(false)}>{t('common:cancel')}</Button>
            <Button
              variant="danger"
              onClick={() =>
                execute(async () => {
                  await client.clearHistory();
                  setConfirmClear(false);
                })
              }
            >
              {t('history:clear')}
            </Button>
          </div>
        </Dialog>
      )}
    </section>
  );
}
