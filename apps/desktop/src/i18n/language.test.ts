import { expect, it } from 'vitest';
import { resolveLanguage } from './language';
it('maps supported system languages and gives explicit choices priority', () => {
  for (const locale of ['zh', 'zh-CN', 'zh-TW', 'zh_Hans_CN'])
    expect(resolveLanguage('system', locale)).toBe('zh-CN');
  for (const locale of ['en-US', 'ja-JP', null, undefined])
    expect(resolveLanguage('system', locale)).toBe('en');
  expect(resolveLanguage('en', 'zh-CN')).toBe('en');
  expect(resolveLanguage('zh-CN', 'en-US')).toBe('zh-CN');
});
