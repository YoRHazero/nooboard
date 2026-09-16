import { Box3, Vector3, type Camera, type Object3D } from 'three';

export type StageTarget = 'board' | 'bird' | 'mailbox';
export interface TargetRect {
  left: number;
  top: number;
  width: number;
  height: number;
}
export type TargetLayout = Record<StageTarget, TargetRect>;

/** Capture the resting silhouette once; animation never moves the interaction targets. */
export function createTargets(root: Object3D) {
  const names: Record<StageTarget, string[]> = {
    board: ['Woodboard_Root', 'Easel_Stand'],
    bird: ['Bird_Root'],
    mailbox: ['Mailbox_Root'],
  };
  const bounds = Object.entries(names).map(([key, objects]) => {
    const box = new Box3();
    objects.forEach((name) => {
      const object = root.getObjectByName(name);
      if (object) box.expandByObject(object);
    });
    return { key: key as StageTarget, box };
  });
  return (camera: Camera): TargetLayout => {
    camera.updateMatrixWorld();
    return Object.fromEntries(
      bounds.map(({ key, box }) => {
        const points = [box.min.x, box.max.x].flatMap((x) =>
          [box.min.y, box.max.y].flatMap((y) =>
            [box.min.z, box.max.z].map((z) => new Vector3(x, y, z).project(camera)),
          ),
        );
        const left = Math.min(...points.map((p) => (p.x + 1) * 50));
        const top = Math.min(...points.map((p) => (1 - p.y) * 50));
        const right = Math.max(...points.map((p) => (p.x + 1) * 50));
        const bottom = Math.max(...points.map((p) => (1 - p.y) * 50));
        return [key, { left, top, width: right - left, height: bottom - top }];
      }),
    ) as TargetLayout;
  };
}
