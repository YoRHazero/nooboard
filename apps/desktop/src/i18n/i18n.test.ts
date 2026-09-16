import { afterEach, expect, it } from 'vitest';
import { i18n, t } from './index';
import { errorText, fail, problem, toProblem } from './errors';
import { fullTime } from '../ui/text';
import { PreviewClient } from '../preview/PreviewClient';
import { deliveryLabel } from '../api/deliveries';

afterEach(async () => {
  await i18n.changeLanguage('zh-CN');
});
it('uses English plural rules and localized numbers', async () => {
  await i18n.changeLanguage('en');
  expect(t('devices:sendTo', { count: 0 })).toBe('Send to 0 devices');
  expect(t('devices:sendTo', { count: 1 })).toBe('Send to 1 device');
  expect(t('devices:sendTo', { count: 2 })).toBe('Send to 2 devices');
  expect(t('common:items', { count: 1000 })).toBe('1,000 items');
  await i18n.changeLanguage('zh-CN');
  expect(t('devices:sendTo', { count: 1 })).toBe('发送到 1 台');
});
it('reformats an existing date, delivery state and stored error after switching', async () => {
  const error = toProblem(fail('pairingCodeRemaining', { count: 1 }));
  const date = new Date(2026, 8, 16, 12, 30).getTime();
  const chinese = fullTime(date);
  expect(errorText(error)).toContain('还可尝试 1 次');
  await i18n.changeLanguage('en');
  expect(errorText(error)).toBe('Incorrect code. 1 attempt left.');
  expect(deliveryLabel('awaitingReceipt')).toBe('Awaiting confirmation');
  expect(fullTime(date)).not.toBe(chinese);
  expect(fullTime(date)).toContain('September');
});
it('accepts structured IPC errors and hides unrecognized diagnostic text', async () => {
  await i18n.changeLanguage('en');
  expect(errorText(toProblem({ code: 'pairingTimeout' }))).toContain('expired');
  expect(toProblem('raw internal database error')).toEqual(problem('generic'));
  expect(toProblem({ code: 'unknown' })).toEqual(problem('generic'));
  expect(toProblem(new Error('本机内部错误'))).toEqual(problem('generic'));
});
it('preserves existing clipboard contents, history, names and pairing session when changing language', async () => {
  const client = new PreviewClient();
  try {
    await client.beginPairing('192.168.1.52:24817');
    const id = client.getSnapshot().onboarding!.session!.id;
    await client.submitPairingCode(id, '00000000');
    const before = client.getSnapshot();
    await i18n.changeLanguage('en');
    expect(client.getSnapshot()).toBe(before);
    expect(errorText(before.onboarding!.session!.error!)).toBe('Incorrect code. 2 attempts left.');
  } finally {
    client.dispose();
  }
});
