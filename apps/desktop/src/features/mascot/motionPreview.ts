import type { Clip } from './motions';
export { motions as previewMotions } from './motions';
export type PreviewMotion = Clip;
export interface MotionPreview {
  changed: (motion: PreviewMotion) => void;
}
