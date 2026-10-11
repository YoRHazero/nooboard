import { expect, it } from 'vitest';
import {
  shortNoobId,
  validAddress,
  parsePort,
  peerState,
  canSend,
  peerAddresses,
} from './selectors';
import { createSeed, nearbySample } from '../../preview/seed';

it('extends ambiguous prefixes while allowing repeated names', () => {
  const devices = [
    { noobId: 'abcd1234ff00', deviceName: '电脑' },
    { noobId: 'abcd1234ff99', deviceName: '电脑' },
  ];
  expect(shortNoobId(devices[0].noobId, devices)).toBe('abcd1234ff00');
  expect(shortNoobId(devices[1].noobId, devices)).toBe('abcd1234ff99');
  expect(shortNoobId('abcdef019876', devices)).toBe('abcdef01');
});
it('keeps fixture IDs separate from certificate fingerprints', () => {
  const seed = createSeed();
  for (const device of [seed.localDevice, ...seed.peers, nearbySample]) {
    expect(device.noobId).toMatch(/^[a-f0-9]{64}$/);
    expect(device.fingerprint).toMatch(/^[a-f0-9]{64}$/);
    expect(device.noobId).not.toBe(device.fingerprint);
  }
});
it('accepts LAN and VPN addresses with bounded ports', () => {
  for (const address of ['192.168.1.8:24816', 'zero:24816', '[fd00::1]:24816'])
    expect(validAddress(address)).toBe(true);
  for (const address of ['host', 'host:0', 'host:65536', 'http://host:80', 'host:80/a'])
    expect(validAddress(address)).toBe(false);
});

it('accepts only decimal ports within the explicit user range', () => {
  for (const value of ['1', '24817', '65535', ' 24816 '])
    expect(parsePort(value)).toBe(Number(value));
  for (const value of ['', '0', '65536', '-1', '2.5', '1e3', '0x50', '0.0.0.0:24816'])
    expect(parsePort(value)).toBeNull();
});

it('does not describe a connected device with receiving disabled as paused', () => {
  const peer = { ...createSeed().peers[0], accepting: false };
  expect(peerState(peer).label).toBe('已连接');
  expect(peerState(peer).detail).toBe('对方暂不接收');
  expect(canSend(peer)).toBe(false);
});

it('explains who paused and prevents sending even before the receiving flag catches up', () => {
  const peer = {
    ...createSeed().peers[0],
    connectionState: 'paused' as const,
    accepting: true,
    paused: false,
  };
  expect(peerState(peer, true).detail).toBe('本机已暂停同步');
  expect(canSend(peer)).toBe(false);
  peer.paused = true;
  expect(peerState(peer, true).detail).toBe('双方已暂停同步');
  expect(peerState(peer, false).detail).toBe('对方已暂停同步');
});

it('keeps discovery, connection attempts and explicit revocation distinct', () => {
  const peer = { ...createSeed().peers[0], online: false };
  expect(peerState({ ...peer, connectionState: 'offline' }).label).toBe('离线');
  expect(peerState({ ...peer, connectionState: 'unconnected' }).label).toBe('未连接');
  expect(peerState({ ...peer, connectionState: 'unpaired' }).label).toBe('未配对');
});

it('keeps the actual endpoint separate from saved addresses and discovered sync ports', () => {
  const peer = { ...createSeed().peers[0], connectionAddress: '192.168.1.99:53421' };
  const nearby = {
    key: 'peer',
    noobId: peer.noobId,
    deviceName: peer.deviceName,
    addresses: ['192.168.1.99:24817', '[fe80::1%4]:24817', '192.168.1.99:24817'],
    syncPort: 24818,
  };
  expect(peerAddresses(peer, [nearby, { ...nearby, noobId: 'other' }])).toEqual({
    current: '192.168.1.99:53421',
    discovered: ['192.168.1.99:24818', '[fe80::1%4]:24818'],
    saved: null,
  });
  expect(peerAddresses(peer, []).saved).toBeNull();
});

it('only presents a saved address as a fallback and never labels it as a live connection', () => {
  const peer = { ...createSeed().peers[0], online: false };
  expect(peerAddresses(peer, [])).toEqual({
    current: null,
    discovered: [],
    saved: peer.settings.address,
  });
  expect(peerAddresses({ ...peer, settings: { ...peer.settings, address: null } }, [])).toEqual({
    current: null,
    discovered: [],
    saved: null,
  });
});
