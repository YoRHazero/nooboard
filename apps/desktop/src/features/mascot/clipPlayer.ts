import {
  AnimationMixer,
  LoopOnce,
  LoopRepeat,
  type AnimationAction,
  type AnimationClip,
  type Object3D,
} from 'three';
import { carriesProps, motions, type Clip } from './motions';

/** Plays authored poses. Never blends paper between unrelated locations. */
export class ClipPlayer {
  readonly mixer: AnimationMixer;
  private actions = new Map<Clip, AnimationAction>();
  private previous?: AnimationAction;
  private fadeRemaining = 0;
  current?: AnimationAction;
  clip?: Clip;
  constructor(root: Object3D, clips: AnimationClip[], complete: () => void) {
    this.mixer = new AnimationMixer(root);
    for (const name of motions) {
      const clip = clips.find((value) => value.name === name);
      if (!clip) throw new Error(`Missing mascot animation: ${name}`);
      this.actions.set(name, this.mixer.clipAction(clip));
    }
    this.mixer.addEventListener('finished', ({ action }) => {
      if (action === this.current) complete();
    });
  }
  play(clip: Clip, stillTime?: number) {
    const action = this.actions.get(clip)!;
    const blend =
      this.current &&
      this.current !== action &&
      stillTime === undefined &&
      !carriesProps(this.clip) &&
      !carriesProps(clip);
    this.previous?.stop();
    this.previous = undefined;
    if (blend) {
      this.previous = this.current;
      this.previous!.fadeOut(0.16);
      this.fadeRemaining = 0.16;
    } else this.mixer.stopAllAction();
    action.reset().setEffectiveWeight(1).setEffectiveTimeScale(1);
    action.setLoop(clip === 'idle' || clip === 'offline' ? LoopRepeat : LoopOnce, Infinity);
    action.clampWhenFinished = true;
    action.play();
    if (blend) action.fadeIn(0.16);
    this.current = action;
    this.clip = clip;
    if (stillTime !== undefined) action.time = Math.min(stillTime, action.getClip().duration);
    this.mixer.update(0);
  }
  update(delta: number) {
    this.mixer.update(delta);
    if (this.previous && (this.fadeRemaining -= delta) <= 0) {
      this.previous.stop();
      this.previous = undefined;
    }
  }
  seek(progress: number) {
    if (!this.current || !this.clip) return;
    this.play(
      this.clip,
      Math.max(0, Math.min(0.999999, progress)) * this.current.getClip().duration,
    );
  }
  dispose(root: Object3D) {
    this.mixer.stopAllAction();
    this.mixer.uncacheRoot(root);
  }
}
