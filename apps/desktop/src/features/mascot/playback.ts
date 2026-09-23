import type { DesktopEvent } from '../../desktop/api';

export type Clip = 'idle' | 'capture' | 'send' | 'receive_text' | 'error' | 'paused' | 'offline';
export type RestMode = 'idle' | 'paused' | 'offline';
export interface PlaybackState {
  active: Clip | null;
  activityId: string | null;
}

/** Serializes performances, never business operations. */
export class Playback {
  private serial = 0;
  private mode: RestMode = 'idle';
  private state: PlaybackState = { active: null, activityId: null };
  constructor(
    private play: (clip: Clip, serial: number | null) => void,
    private changed: (state: PlaybackState) => void,
  ) {}
  event(event: DesktopEvent) {
    if (event.type === 'reset') {
      this.reset();
      return;
    }
    const clip: Clip | null = (
      {
        copied: 'capture',
        sent: 'send',
        received: 'receive_text',
        rejected: 'error',
        applied: null,
      } as const
    )[event.type];
    if (!clip || this.mode !== 'idle' || this.state.active) return;
    this.state = { active: clip, activityId: event.type === 'rejected' ? null : event.sequence };
    this.changed(this.state);
    this.play(clip, ++this.serial);
  }
  finish(serial: number) {
    if (serial !== this.serial || !this.state.active) return;
    this.state = { active: null, activityId: null };
    this.play(this.mode, null);
    this.changed(this.state);
  }
  setMode(mode: RestMode) {
    if (mode === this.mode) return;
    this.mode = mode;
    this.reset();
  }
  reset() {
    this.serial++;
    this.state = { active: null, activityId: null };
    this.play(this.mode, null);
    this.changed(this.state);
  }
}
