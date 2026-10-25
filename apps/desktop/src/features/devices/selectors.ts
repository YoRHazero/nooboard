import { t } from '../../shared/i18n/index';
import type { DeviceIdentity, NearbyDevice, Peer } from '../../client/api';

/** Keep live socket endpoints distinct from discovery hints and saved dial addresses. */
export function peerAddresses(peer: Peer, nearby: readonly NearbyDevice[]) {
  const current = peer.online ? peer.connectionAddress : null;
  const discovered = [
    ...new Set(
      nearby
        .filter((device) => device.noobId === peer.noobId)
        .flatMap((device) =>
          device.addresses.map(
            (address) => `${address.slice(0, address.lastIndexOf(':'))}:${device.syncPort}`,
          ),
        ),
    ),
  ];
  return {
    current,
    discovered,
    saved: !current && !discovered.length ? peer.settings.address : null,
  };
}

export function shortNoobId(
  id: string,
  identities: readonly Pick<DeviceIdentity, 'noobId'>[],
  minimum = 8,
) {
  let length = minimum;
  while (
    length < id.length &&
    identities.some((other) => other.noobId !== id && other.noobId.startsWith(id.slice(0, length)))
  )
    length += 2;
  return id.slice(0, length);
}
export function canSend(peer: Peer) {
  return peer.online && peer.accepting && peer.connectionState === 'connected';
}
export function peerState(peer: Peer, localPaused = false) {
  switch (peer.connectionState) {
    case 'unpaired':
      return {
        label: t('devices:unpairedState'),
        detail: t('devices:pairAgainHelp'),
        tone: 'limited',
      };
    case 'unconnected':
      return {
        label: t('devices:unconnected'),
        detail: t('devices:autoConnecting'),
        tone: 'quiet',
      };
    case 'offline':
      return { label: t('common:offline'), detail: t('devices:waitingDiscovery'), tone: 'quiet' };
    case 'paused':
      return {
        label: t('devices:pausedState'),
        detail: t(
          localPaused
            ? peer.paused
              ? 'devices:bothPaused'
              : 'devices:localPaused'
            : 'devices:remotePaused',
        ),
        tone: 'limited',
      };
    case 'connected':
      return {
        label: t('common:connected'),
        detail: peer.accepting ? t('common:peerAccepting') : t('devices:notReceiving'),
        tone: peer.accepting ? 'online' : 'limited',
      };
  }
}
export function validAddress(address: string) {
  const match = address.match(/^(\[[\da-fA-F:]+\]|[a-zA-Z\d.-]+):(\d{1,5})$/);
  return address.length <= 512 && !!match && +match[2] >= 1 && +match[2] <= 65535;
}
export function validName(name: string) {
  return (
    !!name.trim() && Array.from(name).length <= 80 && !/[\u0000-\u001f\u007f-\u009f]/.test(name)
  );
}

export function parsePort(value: string): number | null {
  const text = value.trim();
  const port = Number(text);
  return /^\d{1,5}$/.test(text) && port >= 1 && port <= 65535 ? port : null;
}
