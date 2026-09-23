import { version } from '../../../package.json';
import type { Peer, DeviceIdentity } from '../../desktop/api';

export const exampleFileNames = ['设计稿.png', 'Notes.txt'];

export const examplePeer: Peer = {
  noobId: '8a4d72f109c3b65e4a20d81f765b30ce62da9e14f7c0539b12e846af90d52b73',
  deviceName: '工作室的电脑',
  platform: 'Windows',
  fingerprint: 'b31ad605894672fceac580d710943e1ad846f25f64cb819135e9dc015b5b6ade',
  settings: { address: '192.168.1.38:24816', autoSend: true },
  online: true,
  accepting: true,
};
export const examplePeers: Peer[] = [
  examplePeer,
  {
    noobId: 'c6e139a05f874c2e8a9240653d672190ba4d9e817a920036e4b20c9f13ad8230',
    deviceName: '工作室的电脑',
    platform: 'Ubuntu',
    fingerprint: '47a069de5b2891f070fad28ea6c2810de72b0a6c6f4b8170e386cc2b6d71954b',
    settings: { address: '100.83.0.12:24816', autoSend: false },
    online: true,
    accepting: true,
  },
  {
    noobId: '2f80cdab3906e4519a0da592788a4f30c1e7958ac0ea214901f7e3a99b6d4075',
    deviceName: '随身 MacBook',
    platform: 'macOS',
    fingerprint: 'a067dc7c5b9af8a66c40c19e3d9e8471a109d6a8d5c18a652fb9852346ea1097',
    settings: { address: '192.168.1.52:24816', autoSend: true },
    online: false,
    accepting: false,
  },
];
export const nearbySample: DeviceIdentity = {
  noobId: 'e405278da7b348def3983d1624bc9781d4baf1257643ce03f738189562fb1a74',
  deviceName: '书房的电脑',
  platform: 'Windows',
  fingerprint: '3790c851ac93e2d1827594a67fe28b8405ef379ca18385d9504fe243796afb10',
};

export const examples = [
  '周五产品同步\n\n1. 确认设备配对流程\n2. 整理文字历史的交互细节\n3. 下周一起看第一版界面',
  'https://developer.mozilla.org/zh-CN/docs/Web/JavaScript',
  '给今天留一点空白。\n\n下午去河边走走，回来再把没写完的那一页补上。',
  'const message = "Hello, nooboard!";\nconsole.log(message);',
];

import type { BackendSnapshot, Snapshot, HistoryItem } from '../../desktop/bridge/generated';
export function hostFixture(): Snapshot {
  return {
    revision: 0,
    version,
    visible: true,
    closeToTray: false,
    language: 'system',
    resolvedLanguage: 'en',
    trayAvailable: false,
    traySupported: false,
    preferenceError: false,
    navigation: null,
  };
}
export function historyFixture(): HistoryItem[] {
  return examples.map((text, index) => ({
    id: String(index + 1),
    text,
    source: index === 1 ? examplePeer.noobId : 'local',
    copiedAt: Date.now() - index * 60_000,
  }));
}
export function snapshotFixture(): BackendSnapshot {
  return {
    session: crypto.randomUUID(),
    revision: '1',
    historyRevision: '1',
    state: 'Running',
    configuration: { savedRevision: '1', effectiveRevision: '1', restartRequired: false },
    settings: {
      mode: 'manual',
      paused: false,
      receive: true,
      discoverable: true,
      history: true,
      historyDays: 30,
      maxHistoryEntries: 1000,
      receiveDirectory: '/Downloads/Nooboard',
    },
    localDevice: {
      deviceName: '我的 MacBook',
      platform: 'macOS',
      syncPort: 24816,
      pairingPort: 24817,
      addresses: [{ interface: 'en0', ip: '192.168.1.24', pairingAddress: '192.168.1.24:24817' }],
      addressError: null,
      noobId: 'a81f2c90d56a97b120030f1ed7916a28c56da0178ebf4913b287e91f384057ca',
      fingerprint: 'c48f720a416b390f84ae1247d81b295f317a9e2107cf218b8e04b79d5f20841e',
    },
    current: {
      revision: '1',
      kind: 'Text',
      text: examples[0],
      source: null,
      copiedAt: Date.now(),
      files: [],
    },
    peers: examplePeers.map(({ platform: _, ...peer }) => structuredClone(peer)),
    manualTargets: examplePeers.slice(0, 2).map((p) => p.noobId),
    transfers: [],
    contentTransfers: [],
    onboarding: { nearby: [], session: null, error: null },
    activities: [
      {
        sequence: '1',
        source: examplePeer.noobId,
        deviceName: examplePeer.deviceName,
        kind: 'Received',
        summary: '会议链接',
        at: Date.now() - 240_000,
      },
    ],
    fault: null,
  };
}

export const exampleReceived = '来自另一台设备的笔记。';
