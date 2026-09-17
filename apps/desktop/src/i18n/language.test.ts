import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { i18n } from './index';
import { changeLanguage, initializeLanguage, loadLanguage, resolveLanguage } from './language';
const nativeLocale = vi.hoisted(() => vi.fn<() => Promise<string | null>>());
const nativePreferences = vi.hoisted(() => vi.fn());
vi.mock('@tauri-apps/plugin-os', () => ({ locale: nativeLocale }));
vi.mock('@tauri-apps/api/core', () => ({ invoke: nativePreferences }));
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
  let nativeLanguage: string | undefined;
  nativePreferences.mockReset().mockImplementation(async (_command, args) => {
    nativeLanguage = args.patch?.language ?? nativeLanguage ?? args.legacyLanguage ?? 'system';
    return { language: nativeLanguage };
  });
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

it('uses the persisted native language instead of an older localStorage value', async () => {
  stored.set('nooboard.language.v1', 'zh-CN');
  nativePreferences.mockResolvedValue({ language: 'en' });
  cleanup = await initializeLanguage(true);
  expect(i18n.resolvedLanguage).toBe('en');
  expect(nativePreferences).toHaveBeenCalledWith('desktop_preferences', {
    legacyLanguage: 'zh-CN',
  });
});

it('does not change the displayed language when a native save fails', async () => {
  cleanup = await initializeLanguage(true);
  nativePreferences.mockRejectedValueOnce({ code: 'desktopPreferences' });
  await expect(changeLanguage('zh-CN')).rejects.toEqual({ code: 'desktopPreferences' });
  expect(i18n.resolvedLanguage).toBe('en');
});
