import en_common from './locales/en/common.json';
import en_devices from './locales/en/devices.json';
import en_history from './locales/en/history.json';
import en_home from './locales/en/home.json';
import en_settings from './locales/en/settings.json';
import en_transfers from './locales/en/transfers.json';
import en_preview from './locales/en/preview.json';
import en_errors from './locales/en/errors.json';
import zh_common from './locales/zh-CN/common.json';
import zh_devices from './locales/zh-CN/devices.json';
import zh_history from './locales/zh-CN/history.json';
import zh_home from './locales/zh-CN/home.json';
import zh_settings from './locales/zh-CN/settings.json';
import zh_transfers from './locales/zh-CN/transfers.json';
import zh_preview from './locales/zh-CN/preview.json';
import zh_errors from './locales/zh-CN/errors.json';

export const resources = {
  en: {
    common: en_common,
    devices: en_devices,
    history: en_history,
    home: en_home,
    settings: en_settings,
    transfers: en_transfers,
    preview: en_preview,
    errors: en_errors,
  },
  'zh-CN': {
    common: zh_common,
    devices: zh_devices,
    history: zh_history,
    home: zh_home,
    settings: zh_settings,
    transfers: zh_transfers,
    preview: zh_preview,
    errors: zh_errors,
  },
} as const;
