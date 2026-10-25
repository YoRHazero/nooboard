import type { Desktop, DesktopRuntime } from './api';
import type { DesktopBridge } from './bridge/port';
import type { PreferenceStorage } from './preferences/runtime';
import { createRuntime } from './runtime';
export function createDesktop(
  bridge: DesktopBridge,
  storage?: PreferenceStorage,
): { runtime: DesktopRuntime; desktop: Desktop } {
  return createRuntime(bridge, storage);
}
export async function createNativeDesktop(storage?: PreferenceStorage) {
  const { TauriBridge } = await import('./bridge/tauri');
  return createDesktop(new TauriBridge(), storage);
}
