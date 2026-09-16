import type { StageTarget } from '../mascot/targets';

export interface OpenPanel {
  target: StageTarget;
  mode: 'preview' | 'pinned';
  focusHeading: boolean;
}

/** Hover follows the pointer; only activation pins a panel. */
export class PanelInteraction {
  private state: OpenPanel | null = null;

  constructor(private changed: (state: OpenPanel | null) => void) {}
  getSnapshot = () => this.state;

  private publish(state: OpenPanel | null) {
    this.state = state;
    this.changed(state);
  }
  move(over: StageTarget | 'panel' | null) {
    if (this.state?.mode === 'pinned') return;
    if (over === 'panel' || over === this.state?.target) return;
    if (over) this.publish({ target: over, mode: 'preview', focusHeading: false });
    else this.dismiss();
  }

  activate(target: StageTarget) {
    // Every stage object is outside the panel: this click only releases an existing pin.
    if (this.state?.mode === 'pinned') this.dismiss();
    else this.publish({ target, mode: 'pinned', focusHeading: true });
  }
  pin() {
    if (this.state?.mode === 'preview')
      this.publish({ ...this.state, mode: 'pinned', focusHeading: false });
  }
  dismiss() {
    if (this.state) this.publish(null);
  }
  cancelPreview() {
    if (this.state?.mode === 'preview') this.dismiss();
  }
}
