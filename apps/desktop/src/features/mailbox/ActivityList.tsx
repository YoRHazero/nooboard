import { useI18n } from '../../i18n/react';
import { useState } from 'react';
import { ArrowDownLeft, ArrowUpRight, Clipboard } from 'lucide-react';
import type { Activity } from '../../api/contracts';
import { useSnapshot } from '../../api/NooboardProvider';
import { batchSummary } from '../../api/deliveries';
import { shortNoobId } from '../../api/devices';
import { fullTime } from '../../ui/text';
import { TransferDialog } from '../transfers/TransferDialog';
import { contentStageLabel } from '../../api/contentTransfers';

export function ActivityList({
  activities,
  grouped,
  onTransfers,
}: {
  activities: readonly Activity[];
  grouped: boolean;
  onTransfers: (key?: string) => void;
}) {
  const { t } = useI18n();
  const types = {
    copied: { icon: Clipboard, label: t('common:copy') },
    sent: { icon: ArrowUpRight, label: t('common:send') },
    received: { icon: ArrowDownLeft, label: t('common:receive') },
  };
  const snapshot = useSnapshot();
  const [selected, setSelected] = useState<Activity | null>(null);
  const detail = selected && (snapshot.activities.find((a) => a.id === selected.id) ?? selected);
  return (
    <section className="mailbox-events" aria-label={t('home:activity')}>
      <header className="mailbox-section-heading">
        <h3>{grouped ? t('home:batchActivity') : t('home:recentActivity')}</h3>
        {grouped && <span>{t('common:items', { count: activities.length })}</span>}
      </header>
      {activities.length ? (
        <ul className="mailbox-activity-list" tabIndex={0} aria-label={t('home:activityList')}>
          {activities.map((activity) => {
            const type = types[activity.kind];
            const Icon = type.icon;
            const label = activity.contentTask
              ? activity.contentNode === 'started'
                ? t('transfers:nodeStarted')
                : contentStageLabel(activity.contentStage ?? 'Unconfirmed')
              : activity.kind === 'sent'
                ? batchSummary(activity)
                : activity.kind === 'copied'
                  ? t('common:copied')
                  : t('common:received');
            const title = activity.title || t('common:blankText');
            const summary =
              activity.kind === 'sent' && !activity.contentTask
                ? t('home:sendSummary', { count: activity.targets?.length ?? 0, summary: title })
                : activity.sourceName
                  ? `${activity.sourceName} · ${title}`
                  : title;
            const source = activity.sourceNoobId
              ? ` · ${shortNoobId(activity.sourceNoobId, snapshot.peers)}`
              : '';
            return (
              <li
                key={activity.id}
                className="mailbox-activity"
                title={`${type.label} · ${summary}${source}\n${label} · ${fullTime(activity.at)}`}
              >
                <span
                  className={`mailbox-activity__type is-${activity.kind}`}
                  role="img"
                  aria-label={type.label}
                >
                  <Icon size={15} />
                </span>
                {activity.kind === 'sent' || activity.contentTask ? (
                  <button
                    className="mailbox-activity__summary"
                    onClick={() =>
                      activity.contentTask
                        ? onTransfers(activity.contentTask)
                        : setSelected(activity)
                    }
                    aria-label={t('home:resultLabel', { summary: summary })}
                  >
                    {summary}
                  </button>
                ) : (
                  <span className="mailbox-activity__summary">{summary}</span>
                )}
                <span className={`mailbox-activity__state is-${activity.state}`}>{label}</span>
              </li>
            );
          })}
        </ul>
      ) : (
        <p className="field-note">{t('home:noActivity')}</p>
      )}
      {detail && <TransferDialog activity={detail} onClose={() => setSelected(null)} />}
    </section>
  );
}
