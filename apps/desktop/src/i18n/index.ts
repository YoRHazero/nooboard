import i18next from 'i18next';
import { initReactI18next } from 'react-i18next';
import { resources } from './resources';

declare module 'i18next' {
  interface CustomTypeOptions {
    defaultNS: 'common';
    resources: typeof resources.en;
    enableSelector: false;
    strictKeyChecks: true;
  }
}

export const i18n = i18next.createInstance();
void i18n.use(initReactI18next).init({
  resources,
  lng: 'zh-CN',
  fallbackLng: 'en',
  supportedLngs: ['zh-CN', 'en'],
  defaultNS: 'common',
  initAsync: false,
  interpolation: { escapeValue: false },
});
export const t = i18n.t.bind(i18n);
export const currentLocale = () => (i18n.resolvedLanguage === 'zh-CN' ? 'zh-CN' : 'en');
