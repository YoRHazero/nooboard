import { t } from '../../shared/i18n/index';
import type { Activity, DesktopSnapshot } from '../../shared/types';
import { canSend } from '../devices/index';
import { batchSummary } from '../transfers/index';
import { contentStageLabel } from '../transfers/index';

export function homeStatus({ settings, peers }: DesktopSnapshot) {
  if (settings.paused) return { label: t('common:paused'), detail: '', tone: 'quiet' };
  if (!peers.length) return { label: t('home:notPaired'), detail: '', tone: 'quiet' };
  const online = peers.filter((p) => p.online).length;
  if (!online)
    return {
      label: peers.every((peer) => peer.connectionState === 'offline')
        ? t('home:allOffline')
        : t('home:noneConnected'),
      detail: '',
      tone: 'quiet',
    };
  const automatic = peers.filter((p) => p.settings.autoSend && canSend(p)).length;
  return {
    label: t('home:connected', { count: online }),
    detail:
      settings.mode === 'automatic'
        ? t('home:autoAvailable', { count: automatic })
        : t('common:manualSend'),
    tone: 'online',
  };
}
export function transferResult(activity: Activity) {
  if (activity.contentTask)
    return activity.contentNode === 'started'
      ? t('transfers:nodeStarted')
      : contentStageLabel(activity.contentStage ?? 'Unconfirmed');
  return activity.kind === 'sent' ? batchSummary(activity) : t('home:textReceived');
}
export function latestTransfer(state: DesktopSnapshot) {
  const ids = new Set(state.peers.map((p) => p.noobId));
  return state.activities.find(
    (item) =>
      item.kind !== 'copied' &&
      ((item.sourceNoobId && ids.has(item.sourceNoobId)) ||
        item.targets?.some((target) => ids.has(target.noobId))),
  );
}
