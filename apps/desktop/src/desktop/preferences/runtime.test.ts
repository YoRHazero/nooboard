import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { PreferenceRuntime } from './runtime';
import { PreviewBridge } from '../../preview/bridge';
import { i18n } from '../../i18n';
let bridge: PreviewBridge;
let preferences: PreferenceRuntime;
let stored: Map<string, string>;
beforeEach(() => {
  vi.stubGlobal('window', new EventTarget());
  vi.stubGlobal('document', { documentElement: { lang: '' } });
  bridge = new PreviewBridge();
  stored = new Map();
  vi.spyOn(bridge, 'locale').mockResolvedValue('en-US');
  preferences = new PreferenceRuntime(
    bridge,
    {
      getItem: (key) => stored.get(key) ?? null,
      setItem: (key, value) => {
        stored.set(key, value);
      },
    },
    () => {},
  );
});
afterEach(async () => {
  preferences.dispose();
  bridge.dispose();
  vi.unstubAllGlobals();
  await i18n.changeLanguage('zh-CN');
});
it('does not allow an older system lookup to overwrite a manual choice', async () => {
  await preferences.start();
  let resolve!: (locale: string) => void;
  vi.mocked(bridge.locale).mockImplementationOnce(
    () =>
      new Promise((done) => {
        resolve = done;
      }),
  );
  const old = preferences.setLanguage('system');
  await vi.waitFor(() => expect(resolve).toBeDefined());
  await preferences.setLanguage('zh-CN');
  resolve('en-US');
  await old;
  expect(i18n.resolvedLanguage).toBe('zh-CN');
  expect(preferences.getSnapshot().language).toBe('zh-CN');
  expect(stored.get('nooboard.language.v1')).toBe('zh-CN');
});
it('retains the displayed language when saving fails', async () => {
  await preferences.start();
  vi.spyOn(bridge, 'request').mockRejectedValueOnce({ code: 'desktopPreferences' });
  await expect(preferences.setLanguage('zh-CN')).rejects.toEqual({ code: 'desktopPreferences' });
  expect(preferences.getSnapshot().language).toBe('system');
  expect(i18n.resolvedLanguage).toBe('en');
});
it('does not publish unsaved appearance and releases focus listeners on disposal', async () => {
  preferences.dispose();
  preferences = new PreferenceRuntime(
    bridge,
    {
      getItem: () => null,
      setItem: () => {
        throw new Error('full');
      },
    },
    () => {},
  );
  await preferences.start();
  const before = preferences.getSnapshot();
  expect(() => preferences.setAppearance({ theme: 'dark' })).toThrow('full');
  expect(preferences.getSnapshot()).toBe(before);
  const request = vi.spyOn(bridge, 'request');
  preferences.dispose();
  request.mockClear();
  window.dispatchEvent(new Event('focus'));
  expect(request).not.toHaveBeenCalled();
});
it('does not let delayed startup override a user language choice', async () => {
  let resolve!: (locale: string) => void;
  vi.mocked(bridge.locale).mockImplementationOnce(
    () =>
      new Promise((done) => {
        resolve = done;
      }),
  );
  const start = preferences.start();
  await vi.waitFor(() => expect(resolve).toBeDefined());
  await preferences.setLanguage('zh-CN');
  resolve('en-US');
  await start;
  expect(preferences.getSnapshot().language).toBe('zh-CN');
});
