import { toProblem } from '../../i18n/errors';
import type * as Wire from '../bridge/generated';
import type { DesktopSnapshot, DeliveryState, Platform, HostState } from './model';
import type { Appearance } from '../preferences/model';
import { batchState, contentActivityState } from '../activity/project';
export const messageKey = (id: Wire.MessageId) => `${id.session}:${id.sequence}`;
const deliveryStates: Record<Wire.DeliveryState, DeliveryState> = {
  Queued: 'queued',
  Sending: 'sending',
  AwaitingReceipt: 'awaitingReceipt',
  Applied: 'applied',
  Rejected: 'rejected',
  Unconfirmed: 'unconfirmed',
  Offline: 'offline',
  Cancelled: 'cancelled',
  Superseded: 'superseded',
  QueueFull: 'queueFull',
};
export function decode(
  frame: Wire.BackendSnapshot,
  host: HostState,
  appearance: Appearance,
): DesktopSnapshot {
  const transfers = new Map(frame.transfers.map((t) => [messageKey(t.id), t]));
  return {
    session: frame.session,
    revision: frame.revision,
    state: frame.state,
    configuration: frame.configuration,
    host,
    appearance,
    settings: { ...frame.settings, receiveDirectory: frame.settings.receiveDirectory ?? null },
    localDevice: {
      ...frame.localDevice,
      platform: ['macOS', 'Windows', 'Linux', 'Ubuntu'].includes(frame.localDevice.platform)
        ? (frame.localDevice.platform as Platform)
        : undefined,
      addressError: frame.localDevice.addressError
        ? toProblem(frame.localDevice.addressError)
        : null,
    },
    current: {
      ...frame.current,
      id: `${frame.session}:${frame.current.revision}`,
      text: frame.current.text ?? '',
      source: frame.current.source ? 'remote' : 'local',
      sourceNoobId: frame.current.source ?? undefined,
      preview: frame.current.preview ?? null,
      imageWidth: frame.current.imageWidth ?? null,
      imageHeight: frame.current.imageHeight ?? null,
    },
    peers: frame.peers.map((p) => ({
      ...p,
      settings: { ...p.settings, address: p.settings.address ?? null },
    })),
    manualTargets: frame.manualTargets,
    contentTransfers: frame.contentTransfers,
    historyRevision: `${frame.session}:${frame.historyRevision}`,
    onboarding: {
      nearby: frame.onboarding.nearby,
      error: frame.onboarding.error ? toProblem(frame.onboarding.error) : null,
      session: frame.onboarding.session
        ? {
            ...frame.onboarding.session,
            error: frame.onboarding.session.error
              ? toProblem(frame.onboarding.session.error)
              : null,
          }
        : null,
    },
    activities: frame.activities.map((record) => {
      const transfer =
        record.kind === 'Sent' && record.messageId
          ? transfers.get(messageKey(record.messageId))
          : undefined;
      const targets = transfer?.targets.map((target) => ({
        ...target,
        state: deliveryStates[target.state],
      }));
      return {
        id: `${frame.session}:${record.sequence}`,
        contentTask: record.contentTask ?? undefined,
        contentNode:
          record.contentNode === 'started' || record.contentNode === 'finished'
            ? record.contentNode
            : undefined,
        contentStage: record.contentStage ?? undefined,
        messageId: record.messageId ?? undefined,
        kind: record.kind === 'Sent' ? 'sent' : record.kind === 'Copied' ? 'copied' : 'received',
        sourceNoobId: record.source ?? null,
        sourceName: record.deviceName ?? undefined,
        title: record.summary,
        at: record.at,
        targets,
        automatic: transfer?.automatic,
        state: record.contentStage
          ? contentActivityState(record.contentStage)
          : record.kind === 'Sent'
            ? targets
              ? batchState(targets)
              : 'unconfirmed'
            : 'applied',
      };
    }),
    notice: frame.fault
      ? { id: `${frame.session}:${frame.fault.sequence}`, message: toProblem(frame.fault.message) }
      : null,
  };
}
