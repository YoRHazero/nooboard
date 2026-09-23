import { expect, it } from 'vitest';
import { createSeed } from '../../preview/seed';
import { homeStatus, latestTransfer, transferResult } from './status';

it('summarizes each connection and the currently available automatic scope', () => {
  const state = createSeed();
  expect(homeStatus(state).label).toBe('2 台设备已连接');
  state.settings.mode = 'automatic';
  expect(homeStatus(state).detail).toBe('文字自动同步 · 1 台可接收');
  state.settings.paused = true;
  expect(homeStatus(state).label).toBe('同步已暂停');
});
it('does not attribute removed peers’ transfers to new devices', () => {
  const state = createSeed();
  expect(latestTransfer(state)?.kind).toBe('received');
  state.peers = state.peers.slice(1);
  expect(latestTransfer(state)).toBeUndefined();
});
it('reports partial delivery without claiming all recipients received it', () => {
  const activity = {
    ...createSeed().activities[0],
    kind: 'sent' as const,
    targets: [{ noobId: 'peer', deviceName: '电脑', state: 'applied' as const }],
  };
  expect(
    transferResult({
      ...activity,
      state: 'partial',
      targets: [
        activity.targets![0],
        { noobId: 'other', deviceName: '同名', state: 'unconfirmed' },
      ],
    }),
  ).toBe('1/2 台已接收');
});
