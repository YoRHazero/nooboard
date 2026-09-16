import { expect, it } from 'vitest';
import { shortNoobId, validAddress, parsePort } from './devices';
import { createSeed, nearbySample } from '../preview/seed';

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
