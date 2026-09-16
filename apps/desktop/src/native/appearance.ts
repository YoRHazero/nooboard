import type { Appearance } from './NativeStore';
const key = 'nooboard.appearance.v1';
export function loadAppearance(): Appearance {
  try {
    const value = JSON.parse(localStorage.getItem(key) ?? '{}');
    return {
      theme: ['light', 'dark', 'system'].includes(value.theme) ? value.theme : 'system',
      reducedMotion: value.reducedMotion === true,
    };
  } catch {
    return { theme: 'system', reducedMotion: false };
  }
}
export function saveAppearance(value: Appearance) {
  localStorage.setItem(key, JSON.stringify(value));
}
