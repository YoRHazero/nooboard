import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { i18n } from './index';
import { changeLanguage, initializeLanguage, loadLanguage, resolveLanguage } from './language';
const nativeLocale = vi.hoisted(() => vi.fn<() => Promise<string | null>>());
vi.mock('@tauri-apps/plugin-os', () => ({ locale: nativeLocale }));
let stored = new Map<string, string>();
let cleanup: (() => void) | undefined;
beforeEach(async () => {
  stored = new Map();
  vi.stubGlobal('localStorage', {
    getItem: (key: string) => stored.get(key) ?? null,
    setItem: (key: string, value: string) => stored.set(key, value),
  });
  vi.stubGlobal('navigator', { languages: ['en-US'], language: 'en-US' });
  vi.stubGlobal('document', { documentElement: { lang: '' } });
  vi.stubGlobal('window', new EventTarget());
  nativeLocale.mockResolvedValue('en-US');
  await changeLanguage('system');
});
afterEach(async () => {
  cleanup?.();
  cleanup = undefined;
  vi.unstubAllGlobals();
  await i18n.changeLanguage('zh-CN');
});
it('maps supported system languages and gives explicit choices priority', () => {
  for (const locale of ['zh', 'zh-CN', 'zh-TW', 'zh_Hans_CN'])
    expect(resolveLanguage('system', locale)).toBe('zh-CN');
  for (const locale of ['en-US', 'ja-JP', null, undefined])
    expect(resolveLanguage('system', locale)).toBe('en');
  expect(resolveLanguage('en', 'zh-CN')).toBe('en');
  expect(resolveLanguage('zh-CN', 'en-US')).toBe('zh-CN');
});
it('persists a validated choice and updates the document language before resolving', async () => {
  await changeLanguage('zh-CN');
  expect(loadLanguage()).toBe('zh-CN');
  expect(document.documentElement.lang).toBe('zh-CN');
  stored.set('nooboard.language.v1', 'invalid');
  expect(loadLanguage()).toBe('system');
});
it('uses native locale and prevents an older system lookup overriding a manual choice', async () => {
  nativeLocale.mockResolvedValue('zh-CN');
  cleanup = await initializeLanguage(true);
  expect(i18n.resolvedLanguage).toBe('zh-CN');
  let resolve!: (value: string) => void;
  nativeLocale.mockImplementationOnce(
    () =>
      new Promise((done) => {
        resolve = done;
      }),
  );
  const pending = changeLanguage('system');
  await changeLanguage('en');
  resolve('zh-CN');
  await pending;
  expect(i18n.resolvedLanguage).toBe('en');
  expect(loadLanguage()).toBe('en');
});
it('keeps switching usable when preference storage is unavailable', async () => {
  vi.stubGlobal('localStorage', {
    getItem() {
      throw new Error();
    },
    setItem() {
      throw new Error();
    },
  });
  expect(loadLanguage()).toBe('system');
  await expect(changeLanguage('zh-CN')).resolves.toBeUndefined();
  expect(document.documentElement.lang).toBe('zh-CN');
});
