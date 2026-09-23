import type { ActivityState, Delivery, DeliveryState, ContentStage } from '../snapshot/model';
export const deliveryPending = (state: DeliveryState) =>
  ['queued', 'sending', 'awaitingReceipt'].includes(state);
export function batchState(targets: readonly Delivery[]): ActivityState {
  if (targets.some((d) => deliveryPending(d.state))) return 'pending';
  if (targets.length && targets.every((d) => d.state === 'applied')) return 'applied';
  if (targets.some((d) => d.state === 'applied')) return 'partial';
  if (targets.some((d) => d.state === 'unconfirmed')) return 'unconfirmed';
  if (targets.length && targets.every((d) => ['cancelled', 'superseded'].includes(d.state)))
    return 'cancelled';
  return 'rejected';
}
export const contentPending = (stage: ContentStage) =>
  !['Completed', 'Saved', 'Failed', 'Cancelled', 'Unconfirmed'].includes(stage);
export function contentActivityState(stage: ContentStage): ActivityState {
  if (contentPending(stage)) return 'pending';
  if (stage === 'Completed') return 'applied';
  if (stage === 'Saved') return 'partial';
  if (stage === 'Cancelled') return 'cancelled';
  if (stage === 'Unconfirmed') return 'unconfirmed';
  return 'rejected';
}
