import { i18n } from '../../i18n';
import { resolveLanguage, isLanguagePreference } from '../../i18n/language';
import { fail } from '../../i18n/errors';
import type { DesktopBridge } from '../bridge/port';
import type { HostState, HostPreferencesPatch } from '../snapshot/model';
import type { Appearance, Preferences, LanguagePreference } from './model';
export interface PreferenceStorage {
  getItem(key: string): string | null;
  setItem(key: string, value: string): void;
}
const appearanceKey = 'nooboard.appearance.v1';
const languageKey = 'nooboard.language.v1';
export class PreferenceRuntime {
  private state: Preferences;
  private generation = 0;
  private disposed = false;
  private writes = Promise.resolve<unknown>(undefined);
  private listeners = new Set<() => void>();
  private cleanup: (() => void) | undefined;
  constructor(
    private bridge: DesktopBridge,
    private storage: PreferenceStorage | undefined,
    private hostChanged: (host: HostState) => void,
  ) {
    let appearance: Appearance = { theme: 'system', reducedMotion: false };
    let language: LanguagePreference = 'system';
    try {
      const saved = JSON.parse(storage?.getItem(appearanceKey) ?? '{}');
      appearance = {
        theme: ['system', 'light', 'dark'].includes(saved.theme) ? saved.theme : 'system',
        reducedMotion: saved.reducedMotion === true,
      };
      const value = storage?.getItem(languageKey);
      if (isLanguagePreference(value)) language = value;
    } catch {
      /* A corrupt browser preference cannot prevent startup. */
    }
    this.state = { appearance, language };
  }
  getSnapshot = () => this.state;
  subscribe = (listener: () => void) => {
    this.listeners.add(listener);
    return () => {
      this.listeners.delete(listener);
    };
  };
  private publish(next: Preferences) {
    this.state = next;
    this.listeners.forEach((fn) => fn());
  }
  async start() {
    const request = ++this.generation;
    let language = this.state.language;
    try {
      const host = await this.updateHost({}, language);
      language = host.language;
    } catch {
      /* Keep translated startup/retry UI available if the host fails. */
    }
    if (this.disposed) return;
    await this.applyLanguage(language, request);
    if (this.disposed || typeof window === 'undefined') return;
    const refresh = () => {
      void this.refreshLanguage();
    };
    window.addEventListener('languagechange', refresh);
    window.addEventListener('focus', refresh);
    this.cleanup = () => {
      window.removeEventListener('languagechange', refresh);
      window.removeEventListener('focus', refresh);
    };
  }
  async updateHost(
    patch: HostPreferencesPatch,
    legacyLanguage?: LanguagePreference,
  ): Promise<HostState> {
    if (this.disposed) throw fail('stopped');
    const reply = await this.bridge.request({
      type: 'hostPreferences',
      data: { patch, legacyLanguage },
    });
    if (reply.type !== 'host') throw fail('generic');
    if (!this.disposed) this.hostChanged(reply.data);
    return reply.data;
  }
  setAppearance(patch: Partial<Appearance>) {
    if (this.disposed) throw fail('stopped');
    const appearance = { ...this.state.appearance, ...patch };
    this.storage?.setItem(appearanceKey, JSON.stringify(appearance));
    this.publish({ ...this.state, appearance });
  }
  async setLanguage(language: LanguagePreference) {
    if (this.disposed) throw fail('stopped');
    const request = ++this.generation;
    const saved = this.writes
      .catch(() => undefined)
      .then(async () => {
        if (request !== this.generation || this.disposed) return false;
        await this.updateHost({ language });
        return true;
      });
    this.writes = saved;
    if (!(await saved) || request !== this.generation || this.disposed) return;
    await this.applyLanguage(language, request);
    if (request !== this.generation || this.disposed) return;
    try {
      this.storage?.setItem(languageKey, language);
    } catch {
      /* Host preferences remain authoritative. */
    }
  }
  private async refreshLanguage() {
    const request = this.generation;
    try {
      const host = await this.updateHost({});
      if (request === this.generation) await this.applyLanguage(host.language, request);
    } catch {
      /* Retain the current language when the host is unavailable. */
    }
  }
  private async applyLanguage(language: LanguagePreference, request: number) {
    const detected = language === 'system' ? await this.bridge.locale().catch(() => null) : null;
    if (this.disposed || request !== this.generation) return;
    await i18n.changeLanguage(resolveLanguage(language, detected));
    if (this.disposed || request !== this.generation) return;
    if (typeof document !== 'undefined')
      document.documentElement.lang = i18n.resolvedLanguage ?? 'en';
    this.publish({ ...this.state, language });
  }
  dispose() {
    this.disposed = true;
    this.generation++;
    this.cleanup?.();
    this.listeners.clear();
  }
}
