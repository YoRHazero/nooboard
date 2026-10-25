import type { TerminalClient } from './terminal';
import type { FileDrop } from './bridge/port';
export type { FileDrop } from './bridge/port';
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
export type * from './snapshot/model';
export type * from './preferences/model';
export {
  batchState,
  deliveryPending,
  contentPending,
  contentActivityState,
} from './activity/project';

/** Components receive this handle; it does not expose its lifecycle owner. */
export interface Desktop {
  readonly terminal: TerminalClient;
  readonly mode: 'native' | 'preview';
  onFileDrop(listener: (event: FileDrop) => void): Promise<() => void>;
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

export { terminalKey } from './terminal';
export type { TerminalClientState, TerminalBuffer } from './terminal';
export type {
  TerminalAction,
  TerminalConflict,
  TerminalConfigInput,
  TerminalConfigView,
  TerminalDependency,
  TerminalDirectory,
  TerminalDirectoryEntry,
  TerminalEnded,
  TerminalForward,
  TerminalHostKey,
  TerminalOptionsView,
  TerminalOutputFrame,
  TerminalPromptView,
  TerminalResourceKind,
  TerminalReuse,
  TerminalRunView,
  TerminalTarget,
  TerminalWorkspaceView,
} from './bridge/generated';
