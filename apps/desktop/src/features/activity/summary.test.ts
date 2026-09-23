import { expect, it } from 'vitest';
import { activitySummary } from './summary';

it('keeps only a compact first nonempty line without splitting emoji', () => {
  expect(activitySummary('\n\r\n  会议   笔记  \n后续全文')).toBe('会议 笔记');
  expect(activitySummary(' \n\t')).toBe('');
  expect(activitySummary('🐥'.repeat(200))).toBe('🐥'.repeat(160) + '…');
});
