import { type Problem } from '../i18n/errors';
export type SendMode = 'manual' | 'automatic';
export type Theme = 'system' | 'light' | 'dark';
export type Platform = 'Windows' | 'Ubuntu' | 'macOS';
export interface Settings {
  discoverable?: boolean;
  mode: SendMode;
  paused: boolean;
  receive: boolean;
  history: boolean;
  historyDays: number;
  maxHistoryEntries: number;
  theme: Theme;
  reducedMotion: boolean;
}
export type EntryId = number | string;
export interface HistoryQuery {
  contains: string;
  source: 'all' | 'local' | 'remote';
  offset: number;
}
export interface HistoryPage {
  items: TextItem[];
  hasMore: boolean;
}
export interface TextItem {
  id: EntryId;
  kind?: 'Text' | 'Empty' | 'Unsupported' | 'Sensitive' | 'TooLarge';
  text: string;
  source: 'local' | 'remote';
  sourceNoobId?: string;
  copiedAt: number;
}
export interface DeviceIdentity {
  noobId: string;
  deviceName: string;
  fingerprint: string;
  /** Presentation metadata; the Rust backend does not currently advertise platform. */
  platform?: Platform;
}
export interface LocalDevice extends DeviceIdentity {
  syncPort: number;
  pairingPort: number;
  addresses: readonly LocalAddress[];
  addressError: Problem | null;
}
export interface LocalAddress {
  interface: string;
  ip: string;
  pairingAddress: string;
}
export interface LocalDevicePatch {
  deviceName?: string;
  pairingPort?: number;
}
export interface PeerSettings {
  address: string | null;
  autoSend: boolean;
}
export interface Peer extends DeviceIdentity {
  settings: PeerSettings;
  online: boolean;
  accepting: boolean;
}
export type DeliveryState =
  | 'queued'
  | 'sending'
  | 'awaitingReceipt'
  | 'applied'
  | 'rejected'
  | 'unconfirmed'
  | 'offline'
  | 'cancelled'
  | 'superseded'
  | 'queueFull';
export interface Delivery {
  noobId: string;
  deviceName: string;
  state: DeliveryState;
}
export type ActivityState =
  'pending' | 'applied' | 'partial' | 'rejected' | 'unconfirmed' | 'cancelled';
export interface Activity {
  /** Presentation identity, shared by all updates to a batch. */
  id: number;
  messageId?: { session: string; sequence: number | string };
  sourceNoobId: string | null;
  sourceName?: string;
  targets?: readonly Delivery[];
  automatic?: boolean;
  kind: 'sent' | 'received' | 'copied';
  state: ActivityState;
  title: string;
  at: number;
}
export interface NearbyDevice {
  key: string;
  noobId: string;
  deviceName: string;
  addresses: string[];
  syncPort: number;
}
export type PairingStage =
  | 'Requesting'
  | 'AwaitingApproval'
  | 'ShowingCode'
  | 'EnteringCode'
  | 'Verifying'
  | 'Saving'
  | 'Completed'
  | 'Failed';
export interface PairingSession {
  id: string;
  incoming: boolean;
  deviceName: string;
  noobId: string | null;
  stage: PairingStage;
  code: string | null;
  expiresAt: number;
  attemptsLeft: number;
  error: Problem | null;
}
export interface Onboarding {
  nearby: readonly NearbyDevice[];
  error: Problem | null;
  session: PairingSession | null;
}
export interface DesktopSnapshot {
  onboarding?: Onboarding;
  historyRevision?: string;
  notice?: { id: string; message: Problem };
  connectionError?: Problem;
  localDevice: LocalDevice;
  current: TextItem;
  history: readonly TextItem[];
  peers: readonly Peer[];
  manualTargets: readonly string[];
  settings: Settings;
  activities: readonly Activity[];
}
export type DesktopEvent =
  | { type: 'copied'; sequence: number }
  | { type: 'sent'; sequence: number }
  | { type: 'applied'; sequence: number }
  | { type: 'received'; sequence: number }
  | { type: 'rejected'; sequence: number }
  | { type: 'reset' };
/** Shared view contract for native and explicitly separate design-preview clients. */
export interface DesktopClient {
  readonly mode: 'native' | 'preview';
  discover(): Promise<void>;
  beginPairing(address: string, expected?: string): Promise<void>;
  acceptPairing(id: string): Promise<void>;
  submitPairingCode(id: string, code: string): Promise<void>;
  dismissPairing(id: string): Promise<void>;
  queryHistory(query: HistoryQuery): Promise<HistoryPage>;
  getSnapshot(): DesktopSnapshot;
  subscribe(listener: () => void): () => void;
  onEvent(listener: (event: DesktopEvent) => void): () => void;
  sendCurrent(): Promise<void>;
  selectTargets(noobIds: string[]): Promise<void>;
  configurePeer(noobId: string, settings: Partial<PeerSettings>): Promise<void>;
  updateLocalDevice(patch: LocalDevicePatch): Promise<void>;
  copyHistory(id: EntryId): Promise<void>;
  deleteHistory(id: EntryId): Promise<void>;
  clearHistory(): Promise<void>;
  updateSettings(settings: Partial<Settings>): Promise<void>;
  unpair(noobId: string): Promise<void>;
}
