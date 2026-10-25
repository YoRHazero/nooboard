import type { ContentStage, ContentTransfer } from '../../client/api';
import { t } from '../../shared/i18n/index';

import { contentPending } from '../../client/api';
export { contentPending, contentActivityState } from '../../client/api';
export const contentCancellable = (stage: ContentStage) =>
  contentPending(stage) && !['Saving', 'Applying', 'Cancelling'].includes(stage);
export const contentStageLabel = (stage: ContentStage) => t(`transfers:stage${stage}`);
export const contentErrorLabel = (task: ContentTransfer) =>
  task.error ? t(`transfers:error${task.error}`) : '';
export function byteLabel(bytes: number) {
  const units = ['B', 'KiB', 'MiB', 'GiB'] as const;
  const index = Math.min(3, Math.max(0, Math.floor(Math.log2(Math.max(1, bytes)) / 10)));
  return t('transfers:byteSize', {
    value: Math.round((bytes / 1024 ** index) * 10) / 10,
    unit: units[index],
  });
}
