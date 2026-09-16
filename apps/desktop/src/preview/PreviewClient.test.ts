import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { PreviewClient } from './PreviewClient';
import { examplePeers, nearbySample } from './seed';

const [first, second, third] = examplePeers.map((p) => p.noobId);
describe('multi-device preview behavior', () => {
  let client: PreviewClient;
  beforeEach(() => {
    vi.useFakeTimers();
    client = new PreviewClient();
  });
  afterEach(() => {
    client.dispose();
    vi.useRealTimers();
  });
  const sends = () => client.getSnapshot().activities.filter((a) => a.kind === 'sent');

  it('keeps history deletion separate from the current clipboard', async () => {
    const current = client.getSnapshot().current;
    await client.deleteHistory(current.id);
    expect(client.getSnapshot().current).toEqual(current);
    expect(client.getSnapshot().history.some((row) => row.id === current.id)).toBe(false);
    await client.clearHistory();
    expect(client.getSnapshot().current).toEqual(current);
  });
  it('keeps manual selection independent of automatic fan-out and does not replay', async () => {
    const events = vi.fn();
    client.onEvent(events);
    await client.selectTargets([second]);
    await client.updateSettings({ mode: 'automatic' });
    await client.toggleConnection(first);
    await client.toggleConnection(first);
    expect(events).not.toHaveBeenCalled();
    await client.sampleCopy();
    expect(sends()[0].targets?.map((d) => d.noobId)).toEqual([first]);
    expect(client.getSnapshot().manualTargets).toEqual([second]);
    await client.sendCurrent();
    expect(sends()[0].targets?.map((d) => d.noobId)).toEqual([second]);
  });
  it('records local copies while paused, without transmitting them', async () => {
    await client.updateSettings({ mode: 'automatic', paused: true });
    const events = vi.fn();
    client.onEvent(events);
    await client.sampleCopy();
    expect(events.mock.calls.map(([event]) => event.type)).toEqual(['copied']);
    expect(client.getSnapshot().history[0].text).toBe(client.getSnapshot().current.text);
    await expect(client.sendCurrent()).rejects.toThrow('暂停');
  });
  it('serializes received copies without forwarding them to any paired device', async () => {
    await client.updateSettings({ mode: 'automatic' });
    await client.configurePeer(second, { autoSend: true });
    const events = vi.fn();
    client.onEvent(events);
    await Promise.all([client.sampleReceive(first), client.sampleReceive(second)]);
    expect(events.mock.calls.map(([event]) => event.type)).toEqual(['received', 'received']);
    expect(client.getSnapshot().current.sourceNoobId).toBe(second);
    expect(
      client
        .getSnapshot()
        .activities.slice(0, 2)
        .map((a) => a.sourceNoobId),
    ).toEqual([second, first]);
    await client.updateSettings({ receive: false });
    await expect(client.sampleReceive(first)).rejects.toThrow('未开启接收');
  });
  it('emits one activity per multi-target send and independently applies receipts', async () => {
    const events = vi.fn();
    client.onEvent(events);
    const text = client.getSnapshot().current.text.split('\n')[0];
    await client.sendCurrent();
    const id = sends()[0].id;
    expect(sends()[0].targets).toHaveLength(2);
    await vi.advanceTimersByTimeAsync(1800);
    expect(sends()[0].targets?.map((d) => d.state)).toEqual(['applied', 'awaitingReceipt']);
    expect(sends()[0].state).toBe('pending');
    await client.sampleCopy();
    await vi.runAllTimersAsync();
    expect(sends().find((a) => a.id === id)?.title).toBe(text);
    expect(sends().find((a) => a.id === id)?.state).toBe('applied');
    expect(events.mock.calls.filter(([e]) => e.type === 'sent')).toHaveLength(1);
    expect(events.mock.calls.filter(([e]) => e.type === 'applied')).toHaveLength(1);
  });
  it('does not replay unavailable manual targets on reconnect', async () => {
    await client.selectTargets([first, third]);
    await client.sendCurrent();
    expect(sends()[0].targets?.[1].state).toBe('offline');
    await client.toggleConnection(third);
    await vi.runAllTimersAsync();
    expect(sends()[0].state).toBe('partial');
    expect(sends()[0].targets?.[1].state).toBe('offline');
  });
  it('keeps the other receipt valid when one device disconnects in flight', async () => {
    await client.sendCurrent();
    await vi.advanceTimersByTimeAsync(250);
    await client.toggleConnection(first);
    await client.toggleConnection(first);
    await vi.runAllTimersAsync();
    expect(sends()[0].targets?.map((d) => d.state)).toEqual(['unconfirmed', 'applied']);
    expect(sends()[0].state).toBe('partial');
  });
  it('unpairs only the selected device and blocks receipts from its old generation', async () => {
    await client.sendCurrent();
    await vi.advanceTimersByTimeAsync(250);
    await client.unpair(first);
    expect(client.getSnapshot().manualTargets).toEqual([second]);
    expect(client.getSnapshot().peers.find((p) => p.noobId === second)?.online).toBe(true);
    await client.beginPairing('192.168.1.38:24817', first);
    await client.submitPairingCode(client.getSnapshot().onboarding!.session!.id, '48217396');
    await vi.runAllTimersAsync();
    expect(sends()[0].targets?.map((d) => d.state)).toEqual(['unconfirmed', 'applied']);
    expect(client.getSnapshot().peers.find((p) => p.noobId === first)?.settings.autoSend).toBe(
      false,
    );
  });
  it('cancels queued work but preserves in-flight work when pausing', async () => {
    await client.sendCurrent();
    const active = sends()[0].id;
    await vi.advanceTimersByTimeAsync(100);
    await client.sendCurrent();
    const queued = sends()[0].id;
    await client.updateSettings({ paused: true });
    await vi.runAllTimersAsync();
    expect(sends().find((a) => a.id === active)?.state).toBe('applied');
    expect(sends().find((a) => a.id === queued)?.state).toBe('cancelled');
  });
  it('coalesces only adjacent queued automatic work without crossing a manual boundary', async () => {
    await client.selectTargets([first]);
    await client.updateSettings({ mode: 'automatic' });
    await client.sampleCopy();
    const auto1 = sends()[0].id;
    await client.sampleCopy();
    const auto2 = sends()[0].id;
    await client.sendCurrent();
    const manual = sends()[0].id;
    await client.sampleCopy();
    const auto3 = sends()[0].id;
    await vi.runAllTimersAsync();
    const state = (id: number) => sends().find((a) => a.id === id)?.targets?.[0].state;
    expect(state(auto1)).toBe('superseded');
    for (const id of [auto2, manual, auto3]) expect(state(id)).toBe('applied');
  });
  it('code pairing preserves existing preferences without duplicate IDs, and rejects self pairing', async () => {
    await client.beginPairing('192.168.1.38:24817', first);
    await client.submitPairingCode(client.getSnapshot().onboarding!.session!.id, '48217396');
    expect(client.getSnapshot().peers).toHaveLength(3);
    expect(client.getSnapshot().peers.find((p) => p.noobId === first)?.settings.autoSend).toBe(
      true,
    );
    expect(client.getSnapshot().manualTargets).toContain(first);
    await expect(
      client.beginPairing('127.0.0.1:24817', client.getSnapshot().localDevice.noobId),
    ).rejects.toThrow('本机');
    await client.beginPairing('192.168.1.52:24817', nearbySample.noobId);
    await client.submitPairingCode(client.getSnapshot().onboarding!.session!.id, '48217396');
    expect(client.getSnapshot().peers).toHaveLength(4);
    expect(client.getSnapshot().manualTargets).not.toContain(nearbySample.noobId);
    const before = client.getSnapshot().localDevice;
    await client.updateLocalDevice({ deviceName: '新名字' });
    expect(client.getSnapshot().localDevice.noobId).toBe(before.noobId);
  });
  it('updates pairing addresses only after a valid port change', async () => {
    const before = client.getSnapshot().localDevice;
    for (const patch of [
      { pairingPort: 0 },
      { pairingPort: 65536 },
      { pairingPort: before.syncPort },
    ]) {
      await expect(client.updateLocalDevice(patch)).rejects.toThrow();
      expect(client.getSnapshot().localDevice).toBe(before);
    }
    await client.updateLocalDevice({ pairingPort: 30001 });
    const after = client.getSnapshot().localDevice;
    expect(after.syncPort).toBe(before.syncPort);
    expect(after.noobId).toBe(before.noobId);
    expect(after.addresses.map((address) => address.pairingAddress)).toEqual([
      '192.168.1.24:30001',
      '100.83.0.8:30001',
    ]);
  });
  it('records copy summaries without pairing or persisted text history', async () => {
    for (const peer of [...client.getSnapshot().peers]) await client.unpair(peer.noobId);
    await client.updateSettings({ history: false });
    await client.sampleCopy();
    const activity = client.getSnapshot().activities[0];
    expect(activity.kind).toBe('copied');
    expect(activity.sourceNoobId).toBeNull();
    expect(client.getSnapshot().history.some((row) => row.id === activity.id)).toBe(false);
  });
  it('bounds each manual queue and retains pending batches beyond the recent list', async () => {
    await client.selectTargets([first]);
    for (let i = 0; i < 35; i++) await client.sendCurrent();
    expect(sends().filter((a) => a.state === 'pending')).toHaveLength(32);
    expect(sends().filter((a) => a.targets?.[0].state === 'queueFull')).toHaveLength(3);
    await vi.runAllTimersAsync();
    expect(sends().filter((a) => a.state === 'applied')).toHaveLength(32);
  });
});
