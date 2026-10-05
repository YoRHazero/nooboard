import { AnimationMixer, Box3, Group, Vector3 } from 'three';
import type { GLTF } from 'three/addons/loaders/GLTFLoader.js';

/** Place the Blender character in the existing stage without retargeting legacy actions. */
export function composeMotionPreview(stage: GLTF, character: GLTF) {
  const oldBird = stage.scene.getObjectByName('Bird_Root');
  if (!oldBird) throw new Error('Stage is missing Bird_Root');
  for (const name of ['idle', 'curious', 'hop']) {
    if (!character.animations.some((clip) => clip.name === name))
      throw new Error(`Character is missing ${name}`);
  }
  const mixer = new AnimationMixer(stage.scene);
  const idle = stage.animations.find((clip) => clip.name === 'idle');
  if (!idle) throw new Error('Stage is missing its resting pose');
  mixer.clipAction(idle).play();
  mixer.update(0);
  stage.scene.updateMatrixWorld(true);
  character.scene.updateMatrixWorld(true);
  const oldBounds = new Box3().setFromObject(oldBird);
  const bounds = new Box3().setFromObject(character.scene);
  const scale = oldBounds.getSize(new Vector3()).y / bounds.getSize(new Vector3()).y;
  const oldCenter = oldBounds.getCenter(new Vector3());
  const center = bounds.getCenter(new Vector3());
  const placement = new Group();
  placement.name = 'Bird_Root';
  placement.scale.setScalar(scale);
  placement.position.set(
    oldCenter.x - center.x * scale,
    oldBounds.min.y - bounds.min.y * scale,
    oldCenter.z - center.z * scale,
  );
  oldBird.removeFromParent();
  placement.add(character.scene);
  stage.scene.add(placement);
  stage.animations = character.animations;
  stage.scene.updateMatrixWorld(true);
  return oldBird;
}
