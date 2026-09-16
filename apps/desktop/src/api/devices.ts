import { t } from '../i18n/index';
import type { DeviceIdentity, Peer } from './contracts';

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
  return peer.online && peer.accepting;
}
export function peerState(peer: Peer) {
  if (!peer.online)
    return { label: t('common:offline'), detail: t('common:waitingConnection'), tone: 'quiet' };
  if (!peer.accepting)
    return { label: t('common:connected'), detail: t('common:peerPaused'), tone: 'limited' };
  return { label: t('common:connected'), detail: t('common:peerAccepting'), tone: 'online' };
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
