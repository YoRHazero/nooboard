import { type Problem } from '../i18n/errors';
// Rust IPC identifiers are strings, preserving u64 values beyond Number.MAX_SAFE_INTEGER.
export type MessageId = { session: string; sequence: string };
export interface NativeSettings {
  receive_directory?: string | null;
  pairing_listen_address?: string;
  discoverable?: boolean;
  mode: 'Manual' | 'Automatic';
  receive: boolean;
  paused: boolean;
  history: boolean;
  max_history_entries: number;
  history_days: number;
  device_name: string;
  listen_address: string;
}
export type NativeDeliveryState =
  | 'Queued'
  | 'Sending'
  | 'AwaitingReceipt'
  | 'Applied'
  | 'Rejected'
  | 'Unconfirmed'
  | 'Offline'
  | 'Cancelled'
  | 'Superseded'
  | 'QueueFull';
export interface NativePeer {
  noob_id: string;
  device_name: string;
  fingerprint: string;
  settings: { address: string | null; auto_send: boolean };
  online: boolean;
  accepting: boolean;
}
export interface NativeTransfer {
  id: MessageId;
  automatic: boolean;
  bytes: number;
  targets: { noob_id: string; device_name: string; state: NativeDeliveryState }[];
}
export interface NativeSnapshot {
  content_transfers?: {
    key: string;
    id: MessageId;
    peer: string;
    device_name: string;
    incoming: boolean;
    kind: 'Image' | 'Files';
    names: string[];
    total_bytes: number;
    completed_bytes: number;
    prepared_bytes: number;
    stage: import('../api/contracts').ContentStage;
    error: import('../api/contracts').TransferFailure | null;
    saved_paths: string[];
    at_ms: number;
  }[];
  local_network: {
    sync_port: number;
    pairing_port: number;
    addresses: { interface: string; ip: string; pairing_address: string }[];
    error: Problem | null;
  };
  onboarding?: {
    nearby: {
      key: string;
      noob_id: string;
      device_name: string;
      addresses: string[];
      sync_port: number;
    }[];
    pairing_address: string;
    discovery_error: Problem | null;
    session: {
      id: string;
      incoming: boolean;
      device_name: string;
      noob_id: string | null;
      stage: import('../api/contracts').PairingStage;
      code: string | null;
      expires_at_ms: number;
      attempts_left: number;
      error: Problem | null;
    } | null;
  };
  platform?: import('../api/contracts').Platform | null;
  session: string;
  revision: string;
  history_revision: string;
  status: {
    noob_id: string;
    fingerprint: string;
    listen_address: string;
    settings: NativeSettings;
    peers: NativePeer[];
    manual_targets: string[];
    transfers: NativeTransfer[];
  };
  current: {
    revision: string;
    kind: 'Text' | 'Empty' | 'Unsupported' | 'Sensitive' | 'TooLarge' | 'Image' | 'Files';
    files?: string[];
    preview?: string | null;
    image_width?: number | null;
    image_height?: number | null;
    text: string | null;
    source: string | null;
    copied_at_ms: number;
  };
  activities: {
    content_task?: string | null;
    content_node?: 'started' | 'finished' | null;
    content_stage?: import('../api/contracts').ContentStage | null;
    sequence: string;
    kind: 'Copied' | 'Sent' | 'Received';
    summary: string;
    at_ms: number;
    source: string | null;
    device_name: string | null;
    message_id: MessageId | null;
  }[];
  fault: { sequence: string; peer: string | null; message: Problem } | null;
}
export type NativeFrame =
  | { type: 'snapshot' | 'recovered'; data: NativeSnapshot }
  | { type: 'desktop'; data: import('../api/contracts').DesktopState }
  | { type: 'stopped'; data: Problem };
export interface NativeConnection {
  snapshot: NativeSnapshot;
  diagnostic: boolean;
  desktop?: import('../api/contracts').DesktopState;
}
export interface NativeHistoryPage {
  items: { id: string; text: string; source: string; copied_at_ms: number }[];
  has_more: boolean;
}
export const messageKey = (id: MessageId) => `${id.session}:${id.sequence}`;
