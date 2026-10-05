import type { DesktopEvent } from '../../desktop/api';
import { carriesProps, type Clip, type Gesture } from './motions';
export type { Clip } from './motions';

export type RestMode = 'idle' | 'paused' | 'offline';
export interface PlaybackState {
  active: Clip | null;
  activityId: string | null;
}
type Receipt = { key: string; activityId: string; result?: 'success' | 'error' };

/** Serializes performances, never business operations. Busy activities stay in the mailbox. */
export class Playback {
  private serial = 0;
  private mode: RestMode = 'idle';
  private initialized = false;
  private receipt: Receipt | null = null;
  private state: PlaybackState = { active: null, activityId: null };
  constructor(
    private play: (clip: Clip, serial: number | null) => void,
    private changed: (state: PlaybackState) => void,
  ) {}
  private get decorative() {
    return (
      this.state.active === 'curious' ||
      this.state.active === 'hop' ||
      this.state.active === 'resume'
    );
  }
  private start(clip: Clip, activityId: string | null) {
    this.state = { active: clip, activityId };
    this.changed(this.state);
    this.play(clip, ++this.serial);
  }
  event(event: DesktopEvent) {
    if (event.type === 'reset') {
      this.reset();
      return;
    }
    if (this.mode !== 'idle') return;
    const key = event.contentTask ?? event.sequence;
    if (event.type === 'applied' || event.type === 'rejected') {
      if (event.contentStage === 'Cancelled') {
        if (this.receipt?.key === key) this.receipt = null;
        return;
      }
      if (this.receipt?.key === key) {
        this.receipt.result = event.type === 'applied' ? 'success' : 'error';
        if (!this.state.active || this.decorative) this.showReceipt();
      }
      return;
    }
    // A newer activity makes a delayed receipt unsuitable for a standalone performance.
    this.receipt = null;
    if (event.contentTask && event.contentNode !== 'finished') {
      // File transfers have their own progress UI. No paper is carried at download start.
      if (!this.state.active || this.decorative) this.receipt = { key, activityId: event.sequence };
      return;
    }
    if (this.state.active && !this.decorative) return;
    const clip =
      event.type === 'copied'
        ? 'capture'
        : event.type === 'sent'
          ? 'send'
          : event.contentTask
            ? 'receive_file'
            : 'receive_text';
    if (event.type === 'sent') this.receipt = { key, activityId: event.sequence };
    this.start(clip, event.sequence);
  }
  private showReceipt() {
    const receipt = this.receipt;
    if (!receipt?.result) return;
    this.receipt = null;
    this.start(receipt.result, receipt.activityId);
  }
  gesture(clip: Gesture) {
    if (this.mode !== 'idle' || (this.state.active && !(clip === 'hop' && this.decorative))) return;
    this.start(clip, null);
  }
  finish(serial: number) {
    if (serial !== this.serial || !this.state.active) return;
    if (this.mode === 'idle' && this.receipt?.result) {
      this.showReceipt();
      return;
    }
    this.state = { active: null, activityId: null };
    this.play(this.mode, null);
    this.changed(this.state);
  }
  setMode(mode: RestMode, animate = true) {
    if (this.initialized && mode === this.mode) return;
    const wake = this.initialized && this.mode !== 'idle' && mode !== 'paused';
    this.initialized = true;
    this.mode = mode;
    this.receipt = null;
    // Finish placing carried paper before changing the visible resting state.
    if (animate && carriesProps(this.state.active ?? undefined)) return;
    if (wake && animate) this.start('resume', null);
    else this.reset();
  }
  reset() {
    this.serial++;
    this.receipt = null;
    this.state = { active: null, activityId: null };
    this.play(this.mode, null);
    this.changed(this.state);
  }
}
