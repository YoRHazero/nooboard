import type { ActivityState, ContentStage, ContentTransfer } from './contracts';
import { t } from '../i18n/index';

const terminal = new Set<ContentStage>([
  'Completed',
  'Saved',
  'Failed',
  'Cancelled',
  'Unconfirmed',
]);
export const contentPending = (stage: ContentStage) => !terminal.has(stage);
export const contentCancellable = (stage: ContentStage) =>
  contentPending(stage) && !['Saving', 'Applying', 'Cancelling'].includes(stage);
export const contentStageLabel = (stage: ContentStage) => t(`transfers:stage${stage}`);
export const contentErrorLabel = (task: ContentTransfer) =>
  task.error ? t(`transfers:error${task.error}`) : '';
export function contentActivityState(stage: ContentStage): ActivityState {
  if (contentPending(stage)) return 'pending';
  if (stage === 'Completed') return 'applied';
  if (stage === 'Saved') return 'partial';
  if (stage === 'Cancelled') return 'cancelled';
  if (stage === 'Unconfirmed') return 'unconfirmed';
  return 'rejected';
}
export function byteLabel(bytes: number) {
  const units = ['B', 'KiB', 'MiB', 'GiB'] as const;
  const index = Math.min(3, Math.max(0, Math.floor(Math.log2(Math.max(1, bytes)) / 10)));
  return t('transfers:byteSize', {
    value: Math.round((bytes / 1024 ** index) * 10) / 10,
    unit: units[index],
  });
}
