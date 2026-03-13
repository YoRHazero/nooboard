# nooboard-network Specification

This document is the authoritative implementation contract for the future `crates/nooboard-network` crate.

If implementation code conflicts with this document, the implementation is wrong. Any intentional behavior change must update this document first.

## 1. Goals

`nooboard-network` is the network domain runtime for nooboard.

It is responsible for:

- LAN auto-discovery and auto-connection
- direct manual connection requests and approvals
- transport, handshake, session lifecycle
- clipboard text sync over active sessions
- file transfer over active sessions

It is not responsible for:

- reading or writing `nooboard.toml`
- reading or writing SQLite or any other persistent storage
- retaining history across process restarts
- frontend/UI behavior
- relay, NAT traversal, or cross-public-network discovery

## 2. Non-Negotiable Boundaries

The implementation MUST obey these boundaries:

- `nooboard-network` MUST NOT depend on `nooboard-storage`.
- `nooboard-network` MUST NOT write files, databases, or caches.
- `nooboard-network` MUST keep all runtime state in memory only.
- `nooboard-network` MUST NOT import `nooboard-app`.
- `nooboard-network` MUST expose a standalone Rust API.
- `nooboard-app` owns config persistence and runtime orchestration.
- `nooboard-config` owns config schema and serialization.

### 2.1 Module-level cohesion rules

The implementation MUST also obey these internal structure rules:

- `runtime.rs` MUST contain only the public `NetworkRuntime` facade, runtime construction, and method delegation.
- `runtime.rs` MUST NOT contain direct seed CRUD logic.
- `runtime.rs` MUST NOT contain snapshot sorting logic.
- `runtime.rs` MUST NOT contain transfer registry logic.
- `runtime.rs` MUST NOT contain protocol-specific logic.
- mutable runtime stores MUST live in focused internal modules.
- snapshot assembly and deterministic ordering MUST live outside `runtime.rs`.
- direct seed storage and search MUST live outside `runtime.rs`.
- session storage and lookup MUST live outside `runtime.rs`.
- helper methods that exist only to mutate or inspect runtime internals for tests are forbidden.

If an implementation change causes `runtime.rs` to become a mixed facade/store/domain-logic file, that implementation is non-conformant.

### 2.2 Runtime state ownership rules

Internal runtime state MUST NOT store a mutable mirror of the full `NetworkConfig`.

Allowed:

- immutable startup parameters in a dedicated immutable struct
- mutable runtime state in dedicated stores

Forbidden:

- keeping `NetworkConfig` inside the mutable runtime state and mutating copies of config-derived values there
- dual-writing user configuration and runtime state inside `nooboard-network`

### 2.3 Testability rules

Internal domain logic MUST be testable without mutating `NetworkRuntime` private state directly.

This means:

- store modules MUST have direct unit tests
- snapshot ordering MUST be testable without going through hidden runtime internals
- tests MUST NOT reach into `runtime.inner.state` to create preconditions for public behavior

## 3. Product Modes

`nooboard-network` supports exactly two connection modes.

### 3.1 LAN Auto Sync

- Uses mDNS only.
- Automatically discovers peers.
- Automatically connects discovered peers.
- Uses `noob_id` only inside LAN mode for peer deduplication and connection direction.
- Does not expose per-peer connect or block controls.
- Is controlled by a single persistent `lan.enabled` flag.

### 3.2 Direct Connect

- Uses manually configured address seeds only.
- Never auto-connects.
- Never auto-reconnects.
- Requires explicit `connect_direct_seed(seed_id)`.
- Requires explicit approval by the receiver after authentication.
- Does not use `noob_id` for any pre-connection routing or deduplication decision.

## 4. Version 1 Scope

Version 1 is IPv4-only.

- The listener binds only IPv4.
- mDNS advertises only IPv4 addresses.
- LAN discovery accepts only IPv4 addresses.
- direct hostname resolution uses only IPv4 results.
- IPv6 addresses, including link-local addresses, are ignored.

This is intentional. IPv6 support is out of scope for v1.

## 5. Configuration Contract

`nooboard-network` consumes a `NetworkConfig` value. It does not load it from disk.

The required public config model is:

```rust
pub struct NetworkConfig {
    pub identity: LocalIdentityConfig,
    pub listen_port: u16,
    pub auth: NetworkAuthConfig,
    pub lan: LanConfig,
    pub direct: DirectConfig,
    pub transport: NetworkTransportConfig,
    pub transfer: NetworkTransferConfig,
}

pub struct LocalIdentityConfig {
    pub noob_id: String,
    pub device_id: String,
}

pub struct NetworkAuthConfig {
    pub token: String,
}

pub struct LanConfig {
    pub enabled: bool,
}

pub struct DirectConfig {
    pub approval_timeout_ms: u64,
    pub seeds: Vec<DirectSeedConfig>,
}

pub struct DirectSeedConfig {
    pub id: uuid::Uuid,
    pub label: String,
    pub host: String,
    pub port: u16,
    pub enabled: bool,
}

pub struct NetworkTransportConfig {
    pub connect_timeout_ms: u64,
    pub handshake_timeout_ms: u64,
    pub ping_interval_ms: u64,
    pub pong_timeout_ms: u64,
    pub max_packet_size: usize,
}

pub struct NetworkTransferConfig {
    pub download_dir: std::path::PathBuf,
    pub max_file_size: u64,
    pub chunk_size: usize,
    pub active_downloads: usize,
    pub decision_timeout_ms: u64,
    pub idle_timeout_ms: u64,
}
```

### 5.1 Mutable vs immutable configuration

The following fields are mutable at runtime:

- `lan.enabled`
- `direct.seeds`

The following fields are immutable after `NetworkRuntime::new(...)`:

- `identity.noob_id`
- `identity.device_id`
- `listen_port`
- `auth.token`
- all `transport` fields
- all `transfer` fields
- `direct.approval_timeout_ms`

Changing immutable fields requires destroying the runtime and creating a new one.

## 6. Public Runtime API

The crate MUST expose an opaque runtime handle with these methods:

```rust
pub type NetworkResult<T> = Result<T, NetworkError>;

#[derive(Clone)]
pub struct NetworkRuntime { /* opaque */ }

impl NetworkRuntime {
    pub fn new(config: NetworkConfig) -> NetworkResult<Self>;

    pub async fn start(&self) -> NetworkResult<()>;
    pub async fn shutdown(&self) -> NetworkResult<()>;

    pub async fn snapshot(&self) -> NetworkResult<NetworkSnapshot>;
    pub fn subscribe(&self) -> NetworkSubscription;

    pub async fn set_lan_enabled(&self, enabled: bool) -> NetworkResult<()>;

    pub async fn list_direct_seeds(&self) -> NetworkResult<Vec<DirectSeedInfo>>;
    pub async fn upsert_direct_seed(
        &self,
        input: UpsertDirectSeedInput,
    ) -> NetworkResult<DirectSeedId>;
    pub async fn remove_direct_seed(&self, id: DirectSeedId) -> NetworkResult<()>;
    pub async fn search_direct_seeds(
        &self,
        query: &str,
    ) -> NetworkResult<Vec<DirectSeedInfo>>;

    pub async fn connect_direct_seed(
        &self,
        id: DirectSeedId,
    ) -> NetworkResult<ConnectDirectOutcome>;
    pub async fn list_pending_direct_requests(
        &self,
    ) -> NetworkResult<Vec<PendingDirectRequest>>;
    pub async fn approve_direct_request(
        &self,
        id: DirectRequestId,
    ) -> NetworkResult<()>;
    pub async fn reject_direct_request(
        &self,
        id: DirectRequestId,
    ) -> NetworkResult<()>;

    pub async fn list_sessions(&self) -> NetworkResult<Vec<SessionInfo>>;
    pub async fn disconnect_session(&self, id: SessionId) -> NetworkResult<()>;

    pub async fn send_text(&self, request: SendTextRequest) -> NetworkResult<()>;
    pub async fn send_files(
        &self,
        request: SendFilesRequest,
    ) -> NetworkResult<Vec<TransferTicket>>;
    pub async fn decide_incoming_transfer(
        &self,
        decision: IncomingTransferDecision,
    ) -> NetworkResult<()>;
    pub async fn cancel_transfer(&self, id: TransferTicket) -> NetworkResult<()>;
}
```

### 6.1 Runtime method semantics

- `new(...)` creates a stopped runtime handle and validates config.
- `start()` is idempotent.
- `shutdown()` is idempotent.
- `snapshot()` returns current in-memory state.
- `subscribe()` returns a live event subscription and does not require async.
- `set_lan_enabled(...)` MUST succeed even when the runtime is stopped; when stopped it only mutates in-memory configuration and MUST NOT start background network tasks.
- `upsert_direct_seed(...)` and `remove_direct_seed(...)` MUST succeed even when the runtime is stopped; they only mutate in-memory seed state.
- `connect_direct_seed(...)` returns when the connect attempt is scheduled, not when it succeeds.
- final connect success or failure is delivered via events.

Required helper types:

```rust
pub struct UpsertDirectSeedInput {
    pub id: Option<DirectSeedId>,
    pub label: String,
    pub host: String,
    pub port: u16,
    pub enabled: bool,
}

pub enum ConnectDirectOutcome {
    Started,
    AlreadyConnected(SessionId),
}
```

## 7. Public Identity and Handle Types

The crate MUST expose these ID types:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SessionId(uuid::Uuid);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct DirectSeedId(uuid::Uuid);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct DirectRequestId(uuid::Uuid);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TransferTicket {
    pub session_id: SessionId,
    pub raw_id: u32,
}
```

The crate MUST NOT use `NoobId` as the primary public handle for sessions or transfers.

## 8. Public Snapshot Types

The runtime MUST expose these snapshot-oriented public types:

```rust
pub enum NetworkStatus {
    Stopped,
    Starting,
    Running,
    Error(String),
}

pub enum ConnectionMode {
    Lan,
    Direct,
}

pub struct NetworkSnapshot {
    pub status: NetworkStatus,
    pub lan_enabled: bool,
    pub lan_peers: Vec<LanPeerInfo>,
    pub direct_seeds: Vec<DirectSeedInfo>,
    pub pending_direct_requests: Vec<PendingDirectRequest>,
    pub sessions: Vec<SessionInfo>,
    pub transfers: TransfersSnapshot,
}

pub struct LanPeerInfo {
    pub noob_id: String,
    pub device_id: String,
    pub addresses: Vec<std::net::SocketAddr>,
    pub last_seen_at_ms: u64,
    pub connected: bool,
}

pub struct DirectSeedInfo {
    pub id: DirectSeedId,
    pub label: String,
    pub host: String,
    pub port: u16,
    pub enabled: bool,
    pub learned_device_id: Option<String>,
    pub last_connected_addr: Option<std::net::SocketAddr>,
}

pub struct PendingDirectRequest {
    pub id: DirectRequestId,
    pub remote_addr: std::net::SocketAddr,
    pub peer_noob_id: String,
    pub peer_device_id: String,
    pub expires_at_ms: u64,
}

pub struct SessionInfo {
    pub id: SessionId,
    pub mode: ConnectionMode,
    pub peer_noob_id: String,
    pub peer_device_id: String,
    pub remote_addr: std::net::SocketAddr,
    pub local_bind_addr: Option<std::net::SocketAddr>,
    pub outbound: bool,
    pub connected_at_ms: u64,
}
```

`DirectSeedInfo.learned_device_id` and `last_connected_addr` are runtime-only metadata. They MUST NOT be persisted by `nooboard-network`.

### 8.1 Snapshot ordering

All snapshot lists MUST be deterministic.

- `lan_peers` MUST be sorted by `noob_id` ascending
- `direct_seeds` MUST preserve user-configured order
- `pending_direct_requests` MUST be sorted by `expires_at_ms` ascending
- `sessions` MUST be sorted by `connected_at_ms` ascending, then `id`
- transfer lists MUST be sorted by their natural recency:
  - `incoming_pending` by `offered_at_ms` ascending
  - `active` by `updated_at_ms` ascending
  - `recent_completed` by `finished_at_ms` descending

## 9. Public Event API

The runtime MUST expose a subscription type:

```rust
pub struct NetworkSubscription { /* opaque */ }

pub enum NetworkEventRecvError {
    Closed,
}

impl NetworkSubscription {
    pub async fn recv(&mut self) -> Result<NetworkEvent, NetworkEventRecvError>;
}
```

The event enum MUST be:

```rust
pub enum NetworkEvent {
    StatusChanged(NetworkStatus),
    LanPeersChanged,
    DirectSeedsChanged,
    PendingDirectRequestsChanged,
    SessionsChanged,
    ConnectionFailed(ConnectionFailure),
    TextReceived {
        session_id: SessionId,
        event_id: String,
        content: String,
        peer_noob_id: String,
        peer_device_id: String,
    },
    IncomingTransferOffered {
        offer: IncomingTransferOffer,
    },
    TransferUpdated {
        transfer: ActiveTransferInfo,
    },
    TransferCompleted {
        transfer: CompletedTransferInfo,
    },
}
```

### 9.1 Connection failure structure

```rust
pub enum ConnectionFailureKind {
    ResolveFailed,
    ConnectFailed,
    TlsHandshakeFailed,
    ProtocolMismatch,
    AuthRejected,
    DirectRejected,
    DirectExpired,
    AlreadyConnected,
    Io,
    Internal,
}

pub struct ConnectionFailure {
    pub kind: ConnectionFailureKind,
    pub mode: ConnectionMode,
    pub peer_noob_id: Option<String>,
    pub peer_device_id: Option<String>,
    pub remote_addr: Option<std::net::SocketAddr>,
    pub local_bind_addr: Option<std::net::SocketAddr>,
    pub detail: String,
}
```

The public event API MUST use structured failures. Raw strings alone are not sufficient.

Required transfer-facing public types:

```rust
pub struct IncomingTransferOffer {
    pub ticket: TransferTicket,
    pub session_id: SessionId,
    pub peer_noob_id: String,
    pub peer_device_id: String,
    pub file_name: String,
    pub file_size: u64,
    pub total_chunks: u32,
    pub offered_at_ms: u64,
}

pub struct ActiveTransferInfo {
    pub ticket: TransferTicket,
    pub session_id: SessionId,
    pub peer_noob_id: String,
    pub peer_device_id: String,
    pub file_name: String,
    pub file_size: u64,
    pub transferred_bytes: u64,
    pub direction: TransferDirection,
    pub state: ActiveTransferState,
    pub updated_at_ms: u64,
}

pub struct CompletedTransferInfo {
    pub ticket: TransferTicket,
    pub session_id: SessionId,
    pub peer_noob_id: String,
    pub peer_device_id: String,
    pub file_name: String,
    pub file_size: u64,
    pub direction: TransferDirection,
    pub outcome: TransferOutcome,
    pub saved_path: Option<std::path::PathBuf>,
    pub message: Option<String>,
    pub finished_at_ms: u64,
}

pub struct TransfersSnapshot {
    pub incoming_pending: Vec<IncomingTransferOffer>,
    pub active: Vec<ActiveTransferInfo>,
    pub recent_completed: Vec<CompletedTransferInfo>,
}

pub enum TransferDirection {
    Upload,
    Download,
}

pub enum ActiveTransferState {
    Queued,
    Starting,
    InProgress,
    Cancelling,
}

pub enum TransferOutcome {
    Succeeded,
    Rejected,
    Cancelled,
    Failed,
}
```

## 10. LAN Auto Sync Behavior

### 10.1 Listener and advertisement

Version 1 listener behavior is fixed:

- The runtime binds `0.0.0.0:listen_port`.
- The runtime MUST NOT bind IPv6.
- The runtime MUST NOT advertise IPv6.

mDNS advertisement rules:

- Service type MUST be `_nooboard._tcp.local.`
- Advertised port MUST equal `listen_port`
- Advertised addresses MUST be derived from actual active IPv4 listener bindings
- Advertised addresses MUST exclude loopback
- Advertised addresses MUST exclude inactive interfaces
- Advertised addresses MUST exclude point-to-point interfaces
- The runtime MUST NOT advertise addresses that are not actually reachable by the listener

TXT records MUST include:

- `pv`: protocol version
- `nid`: local `noob_id`
- `did`: local `device_id`
- `bid`: runtime boot id

### 10.2 LAN peer model

LAN peers are ephemeral.

- A LAN peer exists only while it is discoverable in current runtime memory.
- LAN peers MUST NOT be persisted.
- A LAN peer is keyed by remote `noob_id`.
- Multiple discovered IPv4 addresses for the same `noob_id` MUST merge into one `LanPeerInfo`.

### 10.3 LAN connection direction

If two LAN peers can see each other and there is no active session:

- the node with lexicographically smaller `noob_id` MAY initiate the outbound connection
- the node with lexicographically larger `noob_id` MUST wait for inbound connection

`noob_id` is used here only to avoid duplicate LAN connections.

### 10.4 LAN reconnect policy

LAN mode auto-reconnects only for discovered peers.

Failure backoff for the same LAN peer MUST be:

1. 5 seconds
2. 15 seconds
3. 30 seconds
4. 60 seconds
5. 60 seconds for each subsequent failure

Backoff resets on successful connection or when the peer disappears from discovery.

### 10.5 LAN disable behavior

When `set_lan_enabled(false)` is called:

- mDNS advertisement MUST stop
- mDNS browsing MUST stop
- all LAN sessions MUST disconnect
- direct sessions MUST remain untouched

When `set_lan_enabled(true)` is called:

- advertisement MUST start
- browsing MUST start
- auto-connect behavior resumes

## 11. Direct Connect Behavior

### 11.1 Direct seeds

Direct seeds are user-configured address entries.

- `label` is not unique
- `host` may be an IPv4 literal or hostname
- `port` is required
- disabled seeds MUST stay stored but MUST NOT be connectable
- creating a new seed appends it to the end of the ordered seed list
- updating an existing seed keeps its current position in the ordered seed list

Seed search:

- `search_direct_seeds(query)` MUST match `label`
- it MUST also match `host`
- it MUST also match `learned_device_id` if currently known in memory

### 11.2 Direct hostname resolution

Resolution rules:

- resolve at connect time only
- do not cache DNS results beyond the active connect attempt
- ignore IPv6 results
- try IPv4 results in resolver-returned order
- fail with `ConnectionFailureKind::ResolveFailed` if no IPv4 result exists

### 11.3 Direct connect flow

Direct connect is strictly:

1. user calls `connect_direct_seed(seed_id)`
2. runtime resolves host to IPv4
3. runtime opens TCP
4. runtime completes TLS and authentication
5. receiver creates a pending approval request
6. receiver emits `PendingDirectRequestsChanged`
7. receiver must approve or reject within `approval_timeout_ms`
8. on approval, a session becomes active
9. on rejection or timeout, connection closes

### 11.4 Direct approval timeout

Default required timeout is 30 seconds.

Timeout starts when the receiver creates the pending request, not when TCP dial begins.

On timeout:

- the pending request is removed
- the receiver emits `PendingDirectRequestsChanged`
- the initiator receives `ConnectionFailed` with `ConnectionFailureKind::DirectExpired`
- no session is created

### 11.5 Direct duplicate handling

Direct mode MUST never create multiple active sessions to the same remote logical device.

Duplicate handling occurs in two phases:

- pre-handshake:
  - if an active session already exists for the same remote socket address, return `ConnectDirectOutcome::AlreadyConnected(existing_session_id)`
- post-handshake:
  - if an active session already exists for the same authenticated remote `noob_id`, close the new attempt and emit `ConnectionFailureKind::AlreadyConnected`

If a pending direct request already exists for the same remote logical device, the new inbound attempt MUST be rejected.

### 11.6 Direct reconnect policy

Direct mode MUST NOT reconnect automatically.

This applies to:

- normal disconnects
- network errors
- app restarts
- LAN state changes

Every new direct connection requires a new explicit `connect_direct_seed(...)` call.

### 11.7 Seed removal behavior

Removing a direct seed:

- removes it from the seed store
- MUST NOT disconnect an already active direct session created from it
- MUST prevent future manual connect attempts from that seed id

## 12. Connection Coordinator Rules

All outbound connections MUST go through a single internal coordinator.

The coordinator MUST enforce:

- only one outbound attempt per remote socket address at a time
- direct attempts have priority over LAN attempts
- only one active session per remote logical device

If LAN and direct target the same remote address concurrently:

- direct wins
- LAN skips the attempt

If LAN and direct authenticate to the same remote `noob_id`:

- only one session may survive
- direct does not preempt an already active session
- the later attempt fails with `AlreadyConnected`

## 13. Transport Rules

### 13.1 TLS

Version 1 keeps the current trust model:

- TLS encrypts transport
- certificate verification is disabled
- token-based handshake remains the gate for admission

### 13.2 IPv4 source binding

All outbound IPv4 connections MUST use explicit local source binding.

This means:

- the implementation MUST NOT use `TcpStream::connect(remote_addr)` directly for outbound IPv4
- the implementation MUST select a local IPv4 source address first
- the implementation MUST use `TcpSocket::new_v4()`
- it MUST `bind(local_ipv4:0)`
- then it MUST `connect(remote_ipv4)`

This rule is mandatory and exists to avoid macOS source-address selection failures.

## 14. Protocol Rules

Packets are split into handshake, control, and data domains:

```rust
pub enum Packet {
    Handshake(HandshakePacket),
    Control(ControlPacket),
    Ping { timestamp: u64 },
    Pong { timestamp: u64 },
    Data(DataPacket),
}
```

### 14.1 Handshake packets

```rust
pub enum ConnectionIntent {
    LanSync,
    DirectConnect,
}

pub enum HandshakePacket {
    Hello {
        protocol_version: u16,
        noob_id: String,
        device_id: String,
        intent: ConnectionIntent,
    },
    Challenge {
        nonce: String,
    },
    AuthResponse {
        hash: String,
    },
    AuthAccepted,
    AuthRejected {
        reason: String,
    },
}
```

### 14.2 Control packets

```rust
pub enum DirectRequestStatus {
    PendingApproval,
    Approved,
    Rejected,
    Expired,
    AlreadyConnected,
}

pub enum ControlPacket {
    DirectRequestStatus {
        request_id: DirectRequestId,
        status: DirectRequestStatus,
        expires_at_ms: Option<u64>,
        reason: Option<String>,
    },
    Disconnect {
        reason: Option<String>,
    },
}
```

`DirectRequestStatus` is a protocol concern. It MUST NOT be implemented only in app/UI code.

### 14.3 Data packets

Data packets MUST include:

- clipboard text
- file start
- file decision
- file chunk
- file end
- file cancel

The exact packet names may be preserved from current `nooboard-sync`, but transfer identity MUST become session-centric in the public API.

## 15. Session Rules

Sessions are the only active business-level network objects.

The runtime MUST expose them as:

- session snapshots
- session-targeted send APIs
- session-targeted disconnect APIs

Public transfer identity MUST be `(SessionId, raw_transfer_id)`, not `(NoobId, raw_transfer_id)`.

If a LAN peer disappears from mDNS while a session remains healthy:

- the session stays alive
- only discovery state changes

## 16. Sync Rules

Clipboard text sync:

- may target all active sessions
- may target only specified sessions
- MUST NOT depend on `noob_id` in the public API

File transfer:

- may target one or more active sessions
- approval for incoming files happens after session establishment
- transfer progress and completion MUST be emitted as events

Required public targeting types:

```rust
pub enum SessionTarget {
    AllConnected,
    Sessions(Vec<SessionId>),
}

pub struct SendTextRequest {
    pub event_id: String,
    pub content: String,
    pub target: SessionTarget,
}

pub struct SendFilesRequest {
    pub files: Vec<std::path::PathBuf>,
    pub target: SessionTarget,
}

pub enum IncomingTransferDisposition {
    Accept,
    Reject,
}

pub struct IncomingTransferDecision {
    pub ticket: TransferTicket,
    pub decision: IncomingTransferDisposition,
}
```

## 17. Runtime State Ownership

The runtime owns these in-memory stores:

- LAN peer index
- direct seed store
- pending direct approvals
- session registry
- active transfer registry

The runtime MUST NOT expose internal mutable references to these stores.

## 18. Error Model

Public command errors are for immediate command failures only.

Required public command error cases:

```rust
pub enum NetworkError {
    InvalidConfig(String),
    NotRunning,
    DirectSeedNotFound(DirectSeedId),
    DirectRequestNotFound(DirectRequestId),
    SessionNotFound(SessionId),
    ChannelClosed,
    Internal(String),
}
```

Asynchronous connection failures MUST be delivered through `NetworkEvent::ConnectionFailed`, not returned from `connect_direct_seed(...)`.

## 19. nooboard-app Integration Contract

`nooboard-app` is required to:

- load config from `nooboard-config`
- build `NetworkConfig`
- create and own `NetworkRuntime`
- persist direct seed changes before calling runtime mutation APIs
- recreate `NetworkRuntime` when immutable config changes

`nooboard-app` MUST NOT assume:

- LAN peers persist across restarts
- direct learned metadata persists across restarts
- pending direct requests survive restarts

## 20. Forbidden Behaviors

The implementation MUST NOT:

- keep `manual_peers: Vec<SocketAddr>` as a public model
- couple direct connect and LAN discovery into one candidate pool
- use `noob_id` for pre-connection direct routing
- auto-reconnect direct sessions
- advertise IPv6 in v1
- scan all local addresses and advertise them blindly
- persist network runtime state
- use `SettingsPatch` as the primary network command surface
- use `TcpStream::connect` directly for outbound IPv4
- make `noob_id` the primary public key for sessions or transfers

## 21. Acceptance Criteria

The implementation is considered conformant only if all of the following hold:

- `nooboard-network` builds without depending on `nooboard-storage`
- LAN mode auto-discovers and auto-connects over IPv4 only
- direct seeds never auto-connect
- direct approval timeout is enforced at 30 seconds by default
- one direct session per remote logical device is enforced
- one active session per logical device across LAN and direct is enforced
- outbound IPv4 sockets explicitly bind local source addresses
- `nooboard-app` can manage direct seeds without `SettingsPatch::SetManualPeers`
- public transfer identity is session-centric
- no runtime network state is persisted by the crate itself

## 22. Required First Implementation Order

The implementation order MUST be:

1. public types and `NetworkRuntime` skeleton
2. listener and outbound IPv4 source binding
3. transport, protocol, auth
4. session registry and coordinator
5. LAN runtime
6. direct runtime and approval flow
7. text sync
8. file transfer
9. `nooboard-app` integration

Do not start by porting old `nooboard-sync` engine internals wholesale. Build to this spec directly.
