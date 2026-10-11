// Generated from Rust IPC DTOs. Run npm run ipc:generate; do not edit.
export type ActivityKind = 'Copied' | 'Sent' | 'Received';
export type ActivityRecord = {
  at: number;
  contentNode?: string | null;
  contentStage?: ContentStage | null;
  contentTask?: string | null;
  deviceName?: string | null;
  kind: ActivityKind;
  messageId?: MessageId | null;
  sequence: string;
  source?: string | null;
  summary: string;
};
export type AddressChange = { type: 'clear' } | { data: string; type: 'set' };
export type AppState = 'Running' | 'Stopping' | 'Stopped' | 'Failed';
export type BackendSnapshot = {
  activities: Array<ActivityRecord>;
  configuration: ConfigurationStatus;
  contentTransfers: Array<ContentTransfer>;
  current: Clipboard;
  fault?: Fault | null;
  historyRevision: string;
  localDevice: LocalDevice;
  manualTargets: Array<string>;
  onboarding: Onboarding;
  peers: Array<Peer>;
  revision: string;
  session: string;
  settings: SyncSettings;
  state: AppState;
  transfers: Array<Transfer>;
};
export type Clipboard = {
  copiedAt: number;
  files: Array<string>;
  imageHeight?: number | null;
  imageWidth?: number | null;
  kind: ClipboardKind;
  preview?: string | null;
  revision: string;
  source?: string | null;
  text?: string | null;
};
export type ClipboardKind =
  'Text' | 'Image' | 'Files' | 'Empty' | 'Unsupported' | 'Sensitive' | 'TooLarge';
export type ConfigurationStatus = {
  effectiveRevision: string;
  restartRequired: boolean;
  savedRevision: string;
};
export type Connection = {
  diagnostic: boolean;
  host: Snapshot;
  remote: RemoteSnapshot;
  snapshot: BackendSnapshot;
};
export type ContentKind = 'Image' | 'Files';
export type ContentStage =
  | 'Preparing'
  | 'Queued'
  | 'Waiting'
  | 'Sending'
  | 'Receiving'
  | 'Verifying'
  | 'Saving'
  | 'Applying'
  | 'Cancelling'
  | 'Completed'
  | 'Saved'
  | 'Failed'
  | 'Cancelled'
  | 'Unconfirmed';
export type ContentTransfer = {
  at: number;
  completedBytes: number;
  deviceName: string;
  error?: TransferFailure | null;
  id: MessageId;
  incoming: boolean;
  key: string;
  kind: ContentKind;
  names: Array<string>;
  peer: string;
  savedPaths: Array<string>;
  stage: ContentStage;
  totalBytes: number;
};
export type Delivery = { deviceName: string; noobId: string; state: DeliveryState };
export type DeliveryState =
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
export type Fault = { message: UiError; sequence: string };
export type Frame =
  | { data: RemoteSnapshot; type: 'remote' }
  | { data: TerminalFrame; type: 'terminal' }
  | { type: 'terminalReset' }
  | { data: BackendSnapshot; type: 'snapshot' }
  | { data: BackendSnapshot; type: 'recovered' }
  | { data: Snapshot; type: 'host' }
  | { data: UiError; type: 'stopped' };
export type HistoryItem = { copiedAt: number; id: string; source: string; text: string };
export type HistoryPage = { hasMore: boolean; items: Array<HistoryItem> };
export type HistoryQuery = { contains: string; offset: number; source: HistorySource };
export type HistorySource = 'all' | 'local' | 'remote';
export type Language = 'system' | 'en' | 'zh-CN';
export type LocalAddress = { interface: string; ip: string; pairingAddress: string };
export type LocalDevice = {
  addressError?: UiError | null;
  addresses: Array<LocalAddress>;
  deviceName: string;
  fingerprint: string;
  noobId: string;
  pairingPort: number;
  platform: string;
  syncPort: number;
};
export type LocalDevicePatch = { deviceName?: string | null; pairingPort?: number | null };
export type MessageId = { sequence: string; session: string };
export type Navigation = { id: number; page: Page };
export type NearbyDevice = {
  addresses: Array<string>;
  deviceName: string;
  key: string;
  noobId: string;
  syncPort: number;
};
export type Onboarding = {
  error?: UiError | null;
  nearby: Array<NearbyDevice>;
  session?: PairingSession | null;
};
export type Page = 'home' | 'transfers' | 'settings';
export type PairingSession = {
  attemptsLeft: number;
  code?: string | null;
  deviceName: string;
  error?: UiError | null;
  expiresAt: number;
  id: string;
  incoming: boolean;
  noobId?: string | null;
  stage: PairingStage;
};
export type PairingStage =
  | 'Requesting'
  | 'AwaitingApproval'
  | 'ShowingCode'
  | 'EnteringCode'
  | 'Verifying'
  | 'Saving'
  | 'Completed'
  | 'Failed';
export type Patch = { closeToTray?: boolean | null; language?: Language | null };
export type Peer = {
  accepting: boolean;
  connectionState: PeerConnectionState;
  deviceName: string;
  fingerprint: string;
  noobId: string;
  online: boolean;
  paused: boolean;
  settings: PeerSettings;
  unpairedNotice?: string | null;
};
export type PeerConnectionState = 'connected' | 'paused' | 'offline' | 'unconnected' | 'unpaired';
export type PeerPatch = { address?: AddressChange | null; autoSend?: boolean | null };
export type PeerSettings = { address?: string | null; autoSend: boolean };
export type ProbeAction = 'copy' | 'receive';
export type RemoteChallenge = {
  address: string;
  algorithm: string;
  fingerprint: string;
  id: string;
  sessionId: string;
};
export type RemoteConnection = { error?: UiError | null; state: RemotePhase };
export type RemoteDirection = 'Upload' | 'Download';
export type RemoteDirectory = {
  entries: Array<RemoteEntry>;
  error?: UiError | null;
  path: string;
  state: RemoteDirectoryState;
};
export type RemoteDirectoryState = 'Loading' | 'Partial' | 'Complete' | 'Stale' | 'Error';
export type RemoteEntry = {
  kind: RemoteKind;
  modified?: number | null;
  name: string;
  path: string;
  size?: number | null;
};
export type RemoteFileTarget = { files: string; generation: string; workspace: string };
export type RemoteKind = 'Directory' | 'File' | 'Symlink' | 'Other' | 'Unknown';
export type RemotePhase = 'Connecting' | 'Ready' | 'Ended' | 'Failed' | 'Closed';
export type RemoteProfile = {
  deviceId: string;
  id: string;
  port: number;
  remembered: boolean;
  username: string;
};
export type RemoteReplay = { bytes: Array<number>; end: string; gap: boolean; start: string };
export type RemoteReply =
  | { type: 'done' }
  | { data: { id: string }; type: 'connected' }
  | { data: RemoteReplay; type: 'replay' };
export type RemoteRequest =
  | {
      data: {
        device: string;
        password?: string | null;
        port: number;
        remember: boolean;
        username: string;
        workspace?: string | null;
      };
      type: 'connect';
    }
  | { data: { accept: boolean; id: string }; type: 'hostKey' }
  | { data: { device: string; port: number }; type: 'forgetHostKey' }
  | { data: { workspace: string }; type: 'close' }
  | { data: { bytes: Array<number>; session: string; workspace: string }; type: 'input' }
  | { data: { columns: number; rows: number; session: string; workspace: string }; type: 'resize' }
  | { data: { after: string; session: string; workspace: string }; type: 'replay' }
  | { data: { path: string; target: RemoteFileTarget }; type: 'navigate' }
  | { data: { more: boolean; path: string; target: RemoteFileTarget }; type: 'list' }
  | { data: { path: string; target: RemoteFileTarget }; type: 'collapse' }
  | { data: { directory: string; target: RemoteFileTarget }; type: 'selectUpload' }
  | { data: { directory: string; paths: Array<string>; target: RemoteFileTarget }; type: 'upload' }
  | { data: { paths: Array<string>; target: RemoteFileTarget }; type: 'download' }
  | { type: 'selectDownloadDirectory' }
  | { data: { id: string }; type: 'cancel' }
  | { data: { id: string }; type: 'retry' }
  | { data: { id: string }; type: 'forgetTransfer' }
  | { data: { id: string }; type: 'reveal' };
export type RemoteSnapshot = {
  challenges: Array<RemoteChallenge>;
  downloadDirectory?: string | null;
  profiles: Array<RemoteProfile>;
  revision: string;
  transfers: Array<RemoteTransfer>;
  workspaces: Array<RemoteWorkspace>;
};
export type RemoteTransfer = {
  destination: string;
  deviceId: string;
  direction: RemoteDirection;
  error?: UiError | null;
  id: string;
  sessionId: string;
  source: string;
  state: RemoteTransferState;
  target?: string | null;
  temporaryPath?: string | null;
  total?: number | null;
  transferred: number;
};
export type RemoteTransferState =
  | 'Queued'
  | 'WaitingForConnection'
  | 'Running'
  | 'Finalizing'
  | 'Succeeded'
  | 'Failed'
  | 'Cancelled'
  | 'Unconfirmed';
export type RemoteWorkspace = {
  address?: string | null;
  deviceId: string;
  directories?: Array<RemoteDirectory> | null;
  error?: UiError | null;
  files: RemoteConnection;
  filesId: string;
  generation: string;
  id: string;
  initialDirectory?: string | null;
  port: number;
  profileId: string;
  root?: string | null;
  terminal: RemoteConnection;
  terminalId: string;
  username: string;
};
export type Reply =
  | { data: RemoteReply; type: 'remote' }
  | { type: 'done' }
  | { data: MessageId; type: 'sent' }
  | { data: { revision: string }; type: 'saved' }
  | { data: HistoryPage; type: 'history' }
  | { data: Snapshot; type: 'host' };
export type Request =
  | { data: RemoteRequest; type: 'remote' }
  | { type: 'discover' }
  | { data: { address: string; expected?: string | null }; type: 'beginPairing' }
  | { data: { id: string }; type: 'acceptPairing' }
  | { data: { code: string; id: string }; type: 'pairingCode' }
  | { data: { id: string }; type: 'dismissPairing' }
  | { data: { patch: SyncSettingsPatch }; type: 'updateSyncSettings' }
  | { data: { patch: LocalDevicePatch }; type: 'updateLocalDevice' }
  | { data: { noobId: string; patch: PeerPatch }; type: 'configurePeer' }
  | { data: { targets: Array<string> }; type: 'selectTargets' }
  | { data: { noobId: string }; type: 'unpair' }
  | { type: 'sendCurrent' }
  | { type: 'selectFiles' }
  | { type: 'selectReceiveDirectory' }
  | { data: { key: string }; type: 'cancelTransfer' }
  | { data: { key: string }; type: 'copyReceived' }
  | { data: { query: HistoryQuery }; type: 'queryHistory' }
  | { data: { id: string }; type: 'copyHistory' }
  | { data: { id: string }; type: 'deleteHistory' }
  | { type: 'clearHistory' }
  | { data: { legacyLanguage?: Language | null; patch: Patch }; type: 'hostPreferences' }
  | { data: { id: number }; type: 'acknowledgeNavigation' }
  | { data: { action: ProbeAction }; type: 'probe' };
export type SendMode = 'manual' | 'automatic';
export type Snapshot = {
  closeToTray: boolean;
  language: Language;
  navigation?: Navigation | null;
  preferenceError: boolean;
  resolvedLanguage: string;
  revision: number;
  trayAvailable: boolean;
  traySupported: boolean;
  version: string;
  visible: boolean;
};
export type SyncSettings = {
  discoverable: boolean;
  history: boolean;
  historyDays: number;
  maxHistoryEntries: number;
  mode: SendMode;
  paused: boolean;
  receive: boolean;
  receiveDirectory?: string | null;
};
export type SyncSettingsPatch = {
  discoverable?: boolean | null;
  history?: boolean | null;
  historyDays?: number | null;
  maxHistoryEntries?: number | null;
  mode?: SendMode | null;
  paused?: boolean | null;
  receive?: boolean | null;
};
export type TerminalFrame = { bytes: Array<number>; offset: string; sessionId: string };
export type Transfer = {
  automatic: boolean;
  bytes: number;
  id: MessageId;
  targets: Array<Delivery>;
};
export type TransferFailure =
  | 'Denied'
  | 'Directory'
  | 'Unsupported'
  | 'TooLarge'
  | 'SourceChanged'
  | 'Integrity'
  | 'Io'
  | 'Clipboard'
  | 'Offline'
  | 'Timeout'
  | 'Busy'
  | 'Cancelled'
  | 'Protocol';
export type UiError = { code: string; params?: { [key: string]: number } };
