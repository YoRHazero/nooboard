import type { DesktopBridge } from './bridge/port';
import type { PreferenceStorage } from './preferences/runtime';
import type { Appearance, Preferences, LanguagePreference } from './preferences/model';
import type {
  DesktopSnapshot,
  DesktopEvent,
  ConnectionState,
  HistoryQuery,
  HistoryPage,
  LocalDevicePatch,
  PeerSettings,
  SyncSettingsPatch,
  HostPreferencesPatch,
  MessageId,
} from './snapshot/model';
import { createRuntime } from './runtime';
export type * from './snapshot/model';
export type * from './preferences/model';
export {
  DesktopProvider,
  useDesktop,
  useSnapshot,
  useConnection,
  usePreferences,
  useCommand,
  useFeedback,
} from './react';
export {
  batchState,
  deliveryPending,
  contentPending,
  contentActivityState,
} from './activity/project';

/** Components receive this handle; it does not expose its lifecycle owner. */
export interface Desktop {
  readonly mode: 'native' | 'preview';
  getSnapshot(): DesktopSnapshot;
  subscribe(listener: () => void): () => void;
  onEvent(listener: (event: DesktopEvent) => void): () => void;
  getConnection(): ConnectionState;
  subscribeConnection(listener: () => void): () => void;
  getPreferences(): Preferences;
  subscribePreferences(listener: () => void): () => void;
  reconnect(): Promise<void>;
  discover(): Promise<void>;
  beginPairing(address: string, expected?: string): Promise<void>;
  acceptPairing(id: string): Promise<void>;
  submitPairingCode(id: string, code: string): Promise<void>;
  dismissPairing(id: string): Promise<void>;
  queryHistory(query: HistoryQuery): Promise<HistoryPage>;
  sendCurrent(): Promise<MessageId>;
  selectFiles(): Promise<void>;
  selectReceiveDirectory(): Promise<void>;
  cancelTransfer(key: string): Promise<void>;
  copyReceived(key: string): Promise<void>;
  selectTargets(noobIds: string[]): Promise<void>;
  configurePeer(noobId: string, patch: Partial<PeerSettings>): Promise<void>;
  updateLocalDevice(patch: LocalDevicePatch): Promise<string>;
  updateSyncSettings(patch: SyncSettingsPatch): Promise<string>;
  updateHostPreferences(patch: HostPreferencesPatch): Promise<void>;
  updateAppearance(patch: Partial<Appearance>): Promise<void>;
  setLanguage(language: LanguagePreference): Promise<void>;
  copyHistory(id: string): Promise<void>;
  deleteHistory(id: string): Promise<void>;
  clearHistory(): Promise<void>;
  unpair(noobId: string): Promise<void>;
  acknowledgeNavigation(id: number): Promise<void>;
}
/** Owned by bootstrap; closing a WebView connection never stops core. */
export interface DesktopRuntime {
  start(): Promise<void>;
  dispose(): Promise<void>;
  probe(action: 'copy' | 'receive'): Promise<void>;
  readonly diagnostic: boolean;
}
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
