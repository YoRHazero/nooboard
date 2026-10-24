import { useTranslation } from 'react-i18next';

/** All bundled namespaces are ready before React mounts. */
export const useI18n = () =>
  useTranslation([
    'common',
    'devices',
    'history',
    'home',
    'settings',
    'transfers',
    'preview',
    'errors',
  ] as const);
