import { currentLocale, t } from '../i18n/index';
export const firstLine = (text: string) =>
  text.split('\n').find((line) => line.trim()) ?? t('common:blankText');
export const characterCount = (text: string) => [...text].length;
export const clockTime = (time: number) =>
  new Intl.DateTimeFormat(currentLocale(), {
    hour: '2-digit',
    minute: '2-digit',
    hour12: false,
  }).format(time);
export const fullTime = (time: number) =>
  new Intl.DateTimeFormat(currentLocale(), {
    month: 'long',
    day: 'numeric',
    hour: '2-digit',
    minute: '2-digit',
    hour12: false,
  }).format(time);
