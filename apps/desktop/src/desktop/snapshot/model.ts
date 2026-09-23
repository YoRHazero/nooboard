import type { Problem } from '../../i18n/errors';
import type * as Wire from '../bridge/generated';
import type { Appearance } from '../preferences/model';
export type {
  ContentStage,
  TransferFailure,
  PairingStage,
  MessageId,
  LocalDevicePatch,
  HistoryQuery,
  SyncSettingsPatch,
  ConfigurationStatus,
} from '../bridge/generated';
export type SyncSettings = Omit<Wire.SyncSettings, 'receiveDirectory'> & {
  receiveDirectory: string | null;
};
export type EntryId = string;
export type Platform = 'Windows' | 'Ubuntu' | 'Linux' | 'macOS';
export interface DeviceIdentity {
  noobId: string;
  deviceName: string;
  fingerprint: string;
  platform?: Platform;
}
export type LocalAddress = Wire.LocalAddress;
export interface LocalDevice extends DeviceIdentity {
  syncPort: number;
  pairingPort: number;
  addresses: readonly LocalAddress[];
  addressError: Problem | null;
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
export interface TextItem {
  id: EntryId;
  text: string;
  source: 'local' | 'remote';
  sourceNoobId?: string;
  copiedAt: number;
}
export interface ClipboardItem extends TextItem {
  kind: Wire.ClipboardKind;
  files: readonly string[];
  preview: string | null;
  imageWidth: number | null;
  imageHeight: number | null;
}
export interface HistoryPage {
  items: TextItem[];
  hasMore: boolean;
}
export type ContentTransfer = Wire.ContentTransfer;
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
  id: string;
  contentTask?: string;
  contentNode?: 'started' | 'finished';
  contentStage?: Wire.ContentStage;
  messageId?: Wire.MessageId;
  sourceNoobId: string | null;
  sourceName?: string;
  targets?: readonly Delivery[];
  automatic?: boolean;
  kind: 'sent' | 'received' | 'copied';
  state: ActivityState;
  title: string;
  at: number;
}
export type NearbyDevice = Wire.NearbyDevice;
export type PairingSession = Omit<Wire.PairingSession, 'error'> & { error: Problem | null };
export interface Onboarding {
  nearby: readonly NearbyDevice[];
  error: Problem | null;
  session: PairingSession | null;
}
export type HostState = Wire.Snapshot;
export type HostPreferencesPatch = Wire.Patch;
export interface DesktopSnapshot {
  session: string;
  revision: string;
  state: Wire.AppState;
  configuration: Wire.ConfigurationStatus;
  host: HostState;
  appearance: Appearance;
  onboarding: Onboarding;
  historyRevision: string;
  notice: { id: string; message: Problem } | null;
  connectionError?: Problem;
  localDevice: LocalDevice;
  current: ClipboardItem;
  contentTransfers: readonly ContentTransfer[];
  peers: readonly Peer[];
  manualTargets: readonly string[];
  settings: SyncSettings;
  activities: readonly Activity[];
}
/** Presentation cues only; no business outcome is derived from playing them. */
export type DesktopEvent =
  | { type: 'copied' | 'sent' | 'applied' | 'received' | 'rejected'; sequence: string }
  | { type: 'reset' };
export interface ConnectionState {
  phase: 'idle' | 'connecting' | 'ready' | 'offline' | 'disposed';
  error: Problem | null;
  hasSnapshot: boolean;
}
