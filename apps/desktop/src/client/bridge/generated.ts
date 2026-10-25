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
  terminal: TerminalWorkspaceView;
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
export type Connection = { diagnostic: boolean; host: Snapshot; snapshot: BackendSnapshot };
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
export type ErrorParam = number | string;
export type Fault = { message: UiError; sequence: string };
export type Frame =
  | { data: TerminalEnded; type: 'terminalEnded' }
  | { type: 'terminalLagged' }
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
  connectionAddress?: string | null;
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
export type Reply =
  | { data: { id: string }; type: 'terminalSaved' }
  | { data: { target: TerminalTarget }; type: 'terminalOpened' }
  | { data: TerminalDirectory; type: 'terminalDirectory' }
  | { type: 'done' }
  | { data: MessageId; type: 'sent' }
  | { data: { revision: string }; type: 'saved' }
  | { data: HistoryPage; type: 'history' }
  | { data: Snapshot; type: 'host' };
export type Request =
  | { data: { action: TerminalAction; owner: string }; type: 'terminal' }
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
export type TerminalAction =
  | { data: { config: TerminalConfigInput; id?: string | null }; type: 'saveConfig' }
  | { data: { target: TerminalTarget }; type: 'copyConfig' }
  | { data: { target: TerminalTarget }; type: 'deleteConfig' }
  | { data: { name: string; subscription: string }; type: 'startLocal' }
  | { data: { reuse: TerminalReuse; subscription: string; target: TerminalTarget }; type: 'start' }
  | { data: { subscription: string; target: TerminalTarget }; type: 'attach' }
  | { data: { sequence: string; subscription: string }; type: 'ack' }
  | { data: { subscription: string }; type: 'detach' }
  | { data: { target: TerminalTarget }; type: 'stop' }
  | { data: { bytes: Array<number>; target: TerminalTarget }; type: 'input' }
  | { data: { cols: number; rows: number; target: TerminalTarget }; type: 'resize' }
  | { data: { id: string }; type: 'continue' }
  | { data: { id: string; policy: TerminalLifetime }; type: 'lifetime' }
  | { data: { accept: boolean; id: string }; type: 'confirmHost' }
  | { data: { id: string; password: string; remember: boolean }; type: 'password' }
  | { data: { id: string }; type: 'cancelPrompt' }
  | {
      data: { previous?: TerminalHostKey | null; replacement: TerminalHostKey; targetId: string };
      type: 'replaceHost';
    }
  | { data: { id: string; path?: string | null }; type: 'browse' }
  | { data: { id: string; path: string }; type: 'mkdir' }
  | { data: { destination: string; id: string; source: string }; type: 'rename' }
  | {
      data: { conflict: TerminalConflict; directory: string; folder: boolean; id: string };
      type: 'upload';
    }
  | { data: { conflict: TerminalConflict; id: string; paths: Array<string> }; type: 'download' }
  | { data: { id: string; paths: Array<string> }; type: 'deleteFiles' }
  | { data: { id: string }; type: 'stopFiles' }
  | { data: { options: TerminalOptionsView }; type: 'options' };
export type TerminalAddress = { kind: TerminalAddressKind; value: string };
export type TerminalAddressKind = 'host' | 'device';
export type TerminalAuthEdit = 'keep' | 'password' | 'privateKey';
export type TerminalAuthKind = 'password' | 'privateKey';
export type TerminalCommandInput = { command: string; name: string; sshTarget?: string | null };
export type TerminalConfigInput =
  | { data: TerminalSessionInput; kind: 'terminal' }
  | { data: TerminalCommandInput; kind: 'command' }
  | { data: TerminalTunnelInput; kind: 'tunnel' }
  | { data: TerminalSshInput; kind: 'ssh' };
export type TerminalConfigView = { id: string } & (
  | { data: TerminalSessionInput; kind: 'terminal' }
  | { data: TerminalCommandInput; kind: 'command' }
  | { data: TerminalTunnelInput; kind: 'tunnel' }
  | { data: TerminalSshView; kind: 'ssh' }
);
export type TerminalConflict = 'fail' | 'skip' | 'overwrite';
export type TerminalDependency = { commandId: string; reuse: TerminalReuse };
export type TerminalDependencyView = {
  commandId: string;
  gate: TerminalGate;
  id: string;
  owner: TerminalTarget;
  reason?: string | null;
  runId?: string | null;
};
export type TerminalDirectory = { entries: Array<TerminalDirectoryEntry>; path: string };
export type TerminalDirectoryEntry = { kind: string; name: string; size?: string | null };
export type TerminalEnded = {
  cleanup: Array<string>;
  error?: string | null;
  id: string;
  kind: string;
  name: string;
  result: string;
};
export type TerminalFileRun = {
  bytes: string;
  completed: string;
  error?: string | null;
  id: string;
  operation: string;
  phase: TerminalPhase;
  terminalId: string;
  total?: string | null;
  totalBytes?: string | null;
};
export type TerminalForward = {
  destinationHost?: string | null;
  destinationPort?: number | null;
  kind: TerminalForwardKind;
  listenHost: string;
  listenPort: number;
};
export type TerminalForwardKind = 'local' | 'remote' | 'dynamic';
export type TerminalGate =
  'pending' | 'waitingForInstance' | 'waitingForRelease' | 'released' | 'failed' | 'cancelled';
export type TerminalHostKey = { algorithm: string; bytes: Array<number> };
export type TerminalIssueView = {
  kind: string;
  message?: string | null;
  previous?: TerminalHostKey | null;
  received?: TerminalHostKey | null;
  targetId?: string | null;
};
export type TerminalLifetime = 'dependent' | 'independent';
export type TerminalOptionsView = {
  connectMs: string;
  forceStopMs: string;
  gracefulStopMs: string;
  replayBytes: string;
  requestMs: string;
  shutdownMs: string;
};
export type TerminalOutputFrame =
  | { data: { bytes: Array<number>; offset: string; sequence: string }; type: 'data' }
  | { data: { offset: string; sequence: string }; type: 'gap' }
  | { type: 'closed' };
export type TerminalPhase = 'waiting' | 'starting' | 'running' | 'stopping';
export type TerminalPromptData =
  | { data: { address: string; fingerprint: string; key: TerminalHostKey }; kind: 'host' }
  | { data: { username: string }; kind: 'password' };
export type TerminalPromptView = {
  id: string;
  owner: TerminalTarget;
  prompt: TerminalPromptData;
  targetId: string;
};
export type TerminalResourceKind = 'terminal' | 'command' | 'tunnel' | 'ssh';
export type TerminalReuse = 'shared' | 'alwaysNew';
export type TerminalRunView = {
  address?: string | null;
  command?: string | null;
  configId?: string | null;
  error?: string | null;
  forward?: TerminalForward | null;
  lifetime?: TerminalLifetime | null;
  name: string;
  phase: TerminalPhase;
  readyMarkerReceived: boolean;
  sshTarget?: string | null;
  target: TerminalTarget;
};
export type TerminalSessionInput = {
  dependencies: Array<TerminalDependency>;
  name: string;
  sshTarget?: string | null;
};
export type TerminalSshInput = {
  address: TerminalAddress;
  auth: TerminalAuthEdit;
  name: string;
  passphrase?: string | null;
  port: number;
  privateKey?: string | null;
  username: string;
};
export type TerminalSshView = {
  address: TerminalAddress;
  auth: TerminalAuthKind;
  name: string;
  passwordSaved: boolean;
  port: number;
  trustedKey?: TerminalHostKey | null;
  username: string;
};
export type TerminalTarget = { id: string; kind: TerminalResourceKind };
export type TerminalTunnelInput = {
  dependencies: Array<TerminalDependency>;
  forward: TerminalForward;
  name: string;
  sshTarget: string;
};
export type TerminalWorkspaceView = {
  closing: boolean;
  configs: Array<TerminalConfigView>;
  dependencies: Array<TerminalDependencyView>;
  files: Array<TerminalFileRun>;
  issue?: TerminalIssueView | null;
  options: TerminalOptionsView;
  prompts: Array<TerminalPromptView>;
  revision: string;
  runs: Array<TerminalRunView>;
};
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
export type UiError = { code: string; params?: { [key: string]: ErrorParam } };
