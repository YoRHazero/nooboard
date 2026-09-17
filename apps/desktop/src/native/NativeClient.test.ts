import { describe, expect, it } from 'vitest';
import { NativeClient, type NativeTransport } from './NativeClient';
import { NativeStore } from './NativeStore';
import type { NativeSnapshot, NativeFrame, NativeConnection } from './wire';
import type { DesktopEvent } from '../api/contracts';

const appearance = { theme: 'system' as const, reducedMotion: false };
function frame(revision = '1'): NativeSnapshot {
  return {
    local_network: { sync_port: 24816, pairing_port: 24817, addresses: [], error: null },
    session: 'session',
    revision,
    history_revision: '0',
    platform: 'macOS',
    status: {
      noob_id: 'local',
      fingerprint: 'fingerprint',
      listen_address: '127.0.0.1:24816',
      settings: {
        mode: 'Manual',
        receive: true,
        paused: false,
        history: true,
        max_history_entries: 1000,
        history_days: 30,
        device_name: '本机',
        listen_address: '0.0.0.0:24816',
      },
      peers: [],
      manual_targets: [],
      transfers: [],
    },
    current: {
      revision: '18446744073709551615',
      kind: 'Text',
      text: '当前正文',
      source: null,
      copied_at_ms: 1,
    },
    activities: [],
    fault: null,
  };
}
function sent(snapshot: NativeSnapshot, sequence = '9007199254740993') {
  const id = { session: snapshot.session, sequence };
  snapshot.activities = [
    {
      sequence,
      kind: 'Sent',
      summary: '发送摘要',
      at_ms: 2,
      source: null,
      device_name: null,
      message_id: id,
    },
  ];
  snapshot.status.transfers = [
    {
      id,
      automatic: false,
      bytes: 12,
      targets: [
        { noob_id: 'a', device_name: '同名', state: 'AwaitingReceipt' },
        { noob_id: 'b', device_name: '同名', state: 'AwaitingReceipt' },
      ],
    },
  ];
  return snapshot;
}
class Bridge implements NativeTransport {
  receivers: ((frame: NativeFrame) => void)[] = [];
  resolve: ((value: NativeConnection) => void)[] = [];
  calls: { command: string; args?: Record<string, unknown> }[] = [];
  connect(receive: (frame: NativeFrame) => void) {
    this.receivers.push(receive);
    return new Promise<NativeConnection>((resolve) => {
      this.resolve.push(resolve);
    });
  }
  async invoke<T>(command: string, args?: Record<string, unknown>): Promise<T> {
    this.calls.push({ command, args });
    return undefined as T;
  }
}

describe('native snapshot and command bridge', () => {
  it('keeps file progress separate from immutable mailbox nodes and never announces success at 100% bytes', () => {
    const store = new NativeStore(appearance);
    const events: DesktopEvent[] = [];
    store.onEvent((event) => events.push(event));
    store.apply(frame());
    const progress = frame('2');
    progress.current = { ...progress.current, kind: 'Files', text: null, files: ['photo.png'] };
    progress.content_transfers = [
      {
        key: 'out:files:b',
        id: { session: 's', sequence: '5' },
        peer: 'b',
        device_name: '设备',
        incoming: false,
        kind: 'Files',
        names: ['photo.png'],
        total_bytes: 1024,
        completed_bytes: 500,
        prepared_bytes: 1024,
        stage: 'Sending',
        error: null,
        saved_paths: [],
        at_ms: 1,
      },
    ];
    progress.activities = [
      {
        sequence: '1',
        kind: 'Sent',
        summary: 'photo.png',
        at_ms: 1,
        source: 'b',
        device_name: '设备',
        message_id: null,
        content_task: 'out:files:b',
        content_node: 'started',
        content_stage: 'Waiting',
      },
    ];
    store.apply(progress);
    expect(store.getSnapshot().current.kind).toBe('Files');
    expect(store.getSnapshot().history).toHaveLength(0);
    const finalizing = structuredClone(progress);
    finalizing.revision = '3';
    finalizing.content_transfers![0].completed_bytes = 1024;
    finalizing.content_transfers![0].stage = 'Verifying';
    store.apply(finalizing);
    expect(events.map((event) => event.type)).toEqual(['sent']);
    expect(store.getSnapshot().activities[0].contentStage).toBe('Waiting');
    expect(store.getSnapshot().contentTransfers![0].stage).toBe('Verifying');
    const finished = structuredClone(finalizing);
    finished.revision = '4';
    finished.content_transfers![0].stage = 'Saved';
    finished.content_transfers![0].error = 'Clipboard';
    finished.activities.unshift({
      ...finished.activities[0],
      sequence: '2',
      content_node: 'finished',
      content_stage: 'Saved',
    });
    store.apply(finished);
    expect(store.getSnapshot().activities[0].state).toBe('partial');
    expect(events.map((event) => event.type)).toEqual(['sent', 'rejected']);
    store.apply({ ...finished, revision: '5' }, true);
    expect(events).toHaveLength(2);
  });
  it('sends only task keys for file actions and delegates selection to native dialogs', async () => {
    const bridge = new Bridge();
    const client = new NativeClient(bridge, appearance);
    await client.selectFiles();
    await client.selectReceiveDirectory();
    await client.cancelTransfer('out:s:5:b');
    await client.copyReceived('in:s:7:a');
    expect(bridge.calls).toEqual([
      { command: 'desktop_select_files', args: undefined },
      { command: 'desktop_receive_directory', args: undefined },
      { command: 'desktop_transfer_action', args: { key: 'out:s:5:b', action: 'cancel' } },
      { command: 'desktop_transfer_action', args: { key: 'in:s:7:a', action: 'copy' } },
    ]);
  });
  it('recovers old activities without replay, keeps exact IDs, and ignores stale snapshots', () => {
    const store = new NativeStore(appearance);
    const events: DesktopEvent[] = [];
    store.onEvent((e) => events.push(e));
    store.apply(sent(frame()));
    const initial = store.getSnapshot();
    expect(initial.current.id).toBe('session:18446744073709551615');
    expect(initial.activities[0].messageId?.sequence).toBe('9007199254740993');
    expect(initial.localDevice.platform).toBe('macOS');
    store.apply(frame('0'));
    expect(store.getSnapshot()).toBe(initial);
    expect(events).toEqual([]);
  });
  it('keeps one send identity while independent receipts settle and never assumes delivery from enqueue', () => {
    const store = new NativeStore(appearance);
    const events: DesktopEvent[] = [];
    store.onEvent((e) => events.push(e));
    store.apply(frame());
    const sending = sent(frame('2'));
    store.apply(sending);
    const id = store.getSnapshot().activities[0].id;
    expect(events).toEqual([{ type: 'sent', sequence: id }]);
    expect(store.getSnapshot().activities[0].state).toBe('pending');
    const first = structuredClone(sending);
    first.revision = '3';
    first.status.transfers[0].targets[0].state = 'Applied';
    store.apply(first);
    expect(store.getSnapshot().activities[0].state).toBe('pending');
    const second = structuredClone(first);
    second.revision = '4';
    second.status.transfers[0].targets[1].state = 'Rejected';
    store.apply(second);
    expect(store.getSnapshot().activities[0]).toMatchObject({
      id,
      state: 'partial',
      title: '发送摘要',
    });
    expect(events).toEqual([
      { type: 'sent', sequence: id },
      { type: 'rejected', sequence: id },
    ]);
    expect(store.getSnapshot().activities).toHaveLength(1);
  });
  it('buffers snapshots arriving before connect resolves, suppresses old channels, and recovers unchanged state', async () => {
    const bridge = new Bridge();
    const client = new NativeClient(bridge, appearance);
    const events: DesktopEvent[] = [];
    client.onEvent((e) => events.push(e));
    const starting = client.connect();
    bridge.receivers[0]({ type: 'snapshot', data: sent(frame('3')) });
    bridge.resolve[0]({ snapshot: frame(), diagnostic: false });
    await starting;
    expect(client.getSnapshot().activities).toHaveLength(1);
    expect(events).toEqual([]);
    bridge.receivers[0]({ type: 'stopped', data: { code: 'stopped' } });
    expect(client.getSnapshot().connectionError).toEqual({ code: 'stopped' });
    const reconnect = client.connect();
    bridge.resolve[1]({ snapshot: sent(frame('3')), diagnostic: false });
    await reconnect;
    expect(client.getSnapshot().connectionError).toBeUndefined();
    bridge.receivers[0]({ type: 'stopped', data: { code: 'closing' } });
    expect(client.getSnapshot().connectionError).toBeUndefined();
  });
  it('resets session activity identities without playing recovered events', () => {
    const store = new NativeStore(appearance);
    const events: DesktopEvent[] = [];
    store.onEvent((e) => events.push(e));
    store.apply(sent(frame('7')));
    const id = store.getSnapshot().activities[0].id;
    const fresh = frame();
    fresh.session = 'new';
    store.apply(sent(fresh));
    expect(store.getSnapshot().activities[0].id).not.toBe(id);
    expect(events).toEqual([{ type: 'reset' }]);
  });
  it('keeps independent peer patches and string history IDs in commands', async () => {
    const bridge = new Bridge();
    const client = new NativeClient(bridge, appearance);
    await client.configurePeer('a', { autoSend: true });
    await client.configurePeer('b', { address: null });
    await client.copyHistory('9223372036854775807');
    await client.updateSettings({ mode: 'automatic' });
    await client.updateLocalDevice({ pairingPort: 30001 });
    expect(bridge.calls).toEqual([
      {
        command: 'desktop_peer_settings',
        args: { noobId: 'a', patch: { address: null, autoSend: true, hasAddress: false } },
      },
      {
        command: 'desktop_peer_settings',
        args: { noobId: 'b', patch: { address: null, autoSend: null, hasAddress: true } },
      },
      { command: 'desktop_history_action', args: { action: 'copy', id: '9223372036854775807' } },
      { command: 'desktop_settings', args: { patch: { mode: 'Automatic' } } },
      { command: 'desktop_settings', args: { patch: { pairingPort: 30001 } } },
    ]);
  });
});

it('restores the authoritative pairing prompt without creating a trusted peer and clears a finished code', () => {
  const store = new NativeStore(appearance);
  const snapshot = frame();
  snapshot.onboarding = {
    nearby: [],
    pairing_address: '127.0.0.1:24817',
    discovery_error: null,
    session: {
      id: 'pair-1',
      incoming: true,
      device_name: '另一只肥啾',
      noob_id: 'remote',
      stage: 'ShowingCode',
      code: '01234567',
      expires_at_ms: 120000,
      attempts_left: 3,
      error: null,
    },
  };
  store.apply(snapshot);
  expect(store.getSnapshot().onboarding?.session).toMatchObject({
    id: 'pair-1',
    code: '01234567',
    expiresAt: 120000,
  });
  expect(store.getSnapshot().peers).toHaveLength(0);
  const finished = structuredClone(snapshot);
  finished.revision = '2';
  finished.onboarding!.session = null;
  store.apply(finished);
  expect(store.getSnapshot().onboarding?.session).toBeNull();
});

it('uses live ports and interface addresses instead of wildcard bind settings', () => {
  const store = new NativeStore(appearance);
  const snapshot = frame();
  snapshot.local_network = {
    sync_port: 30200,
    pairing_port: 30201,
    addresses: [{ interface: 'en0', ip: '192.168.1.24', pairing_address: '192.168.1.24:30201' }],
    error: null,
  };
  store.apply(snapshot);
  expect(store.getSnapshot().localDevice).toMatchObject({
    syncPort: 30200,
    pairingPort: 30201,
    addresses: [{ interface: 'en0', ip: '192.168.1.24', pairingAddress: '192.168.1.24:30201' }],
    addressError: null,
  });
  snapshot.revision = '2';
  snapshot.local_network.addresses = [];
  store.apply(snapshot);
  expect(store.getSnapshot().localDevice.addresses).toEqual([]);
});
