import type { DeliveryState } from '../api/contracts';
type Job = { activity: number; peer: string; automatic: boolean };
type Lane = { queue: Job[]; active?: Job; scheduled: boolean };

/** Independent simulated writers; receipts never occupy a writer's queue. */
export class PreviewTransfers {
  private lanes = new Map<string, Lane>();
  private timers = new Set<ReturnType<typeof setTimeout>>();
  private generations = new Map<string, number>();
  constructor(private update: (activity: number, peer: string, state: DeliveryState) => void) {}
  private later(callback: () => void, delay: number) {
    const timer = setTimeout(() => {
      this.timers.delete(timer);
      callback();
    }, delay);
    this.timers.add(timer);
  }
  enqueue(job: Job) {
    const lane = this.lanes.get(job.peer) ?? { queue: [], scheduled: false };
    this.lanes.set(job.peer, lane);
    if (job.automatic && lane.queue.at(-1)?.automatic) {
      const old = lane.queue.pop()!;
      this.update(old.activity, old.peer, 'superseded');
    }
    if (lane.queue.length >= 32) {
      this.update(job.activity, job.peer, 'queueFull');
      return;
    }
    lane.queue.push(job);
    this.pump(job.peer, lane);
  }
  private pump(peer: string, lane: Lane) {
    if (lane.active || lane.scheduled || !lane.queue.length) return;
    lane.scheduled = true;
    const generation = this.generations.get(peer) ?? 0;
    this.later(() => {
      if (generation !== (this.generations.get(peer) ?? 0)) return;
      lane.scheduled = false;
      const job = lane.queue.shift();
      if (!job) return;
      lane.active = job;
      this.update(job.activity, peer, 'sending');
      this.later(() => {
        if (generation !== (this.generations.get(peer) ?? 0)) return;
        lane.active = undefined;
        this.update(job.activity, peer, 'awaitingReceipt');
        this.pump(peer, lane);
        const delay = 800 + parseInt(peer.slice(0, 2), 16) * 5;
        this.later(() => {
          if (generation === (this.generations.get(peer) ?? 0))
            this.update(job.activity, peer, 'applied');
        }, delay);
      }, 160);
    }, 80);
  }
  cancelQueued(peer: string, automaticOnly: boolean) {
    const lane = this.lanes.get(peer);
    if (!lane) return;
    lane.queue = lane.queue.filter((job) => {
      if (automaticOnly && !job.automatic) return true;
      this.update(job.activity, peer, 'cancelled');
      return false;
    });
  }
  disconnect(peer: string) {
    this.generations.set(peer, (this.generations.get(peer) ?? 0) + 1);
    this.lanes.delete(peer);
  }
  dispose() {
    this.timers.forEach(clearTimeout);
    this.timers.clear();
    this.lanes.clear();
    this.generations.clear();
  }
}
