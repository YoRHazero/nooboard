import { readFile } from 'node:fs/promises';
import { AnimationMixer, Box3, Mesh, Vector3 } from 'three';
import { GLTFLoader } from 'three/addons/loaders/GLTFLoader.js';
import { expect, it } from 'vitest';
import { composeMotionPreview } from './composeMotionPreview';
import { createTargets } from './targets';
import { OrthographicCamera } from 'three';

async function load(name: string) {
  const data = await readFile(new URL(`../../../public/mascot/${name}.glb`, import.meta.url));
  return new GLTFLoader().parseAsync(
    data.buffer.slice(data.byteOffset, data.byteOffset + data.byteLength),
    '',
  );
}

it('ships complete Blender clips with matching resting poses and visible motion', async () => {
  const gltf = await load('bird-wing-18');
  // A GLB export with Apply Modifiers disabled ships the angular 45-vertex cage.
  for (const name of ['14-WING-L', '14-WING-R']) {
    const wing = gltf.scene.getObjectByName(name);
    expect(wing).toBeInstanceOf(Mesh);
    expect((wing as Mesh).geometry.getAttribute('position').count).toBeGreaterThan(100);
  }
  expect(gltf.animations.map((clip) => clip.name)).toEqual(['idle', 'curious', 'hop']);
  const signatures = gltf.animations.map((clip) => clip.tracks.map((track) => track.name));
  expect(signatures[1]).toEqual(signatures[0]);
  expect(signatures[2]).toEqual(signatures[0]);
  expect(signatures[0]).toEqual(
    expect.arrayContaining([
      'Nooboard_Form_09.position',
      'Body.scale',
      'Head.quaternion',
      '09_-_eye_left.scale',
      '09_-_eye_right.scale',
    ]),
  );
  for (const clip of gltf.animations) {
    expect(clip.validate()).toBe(true);
    for (const track of clip.tracks) {
      const size = track.getValueSize();
      const initial = gltf.animations[0].tracks.find((candidate) => candidate.name === track.name)!;
      for (let i = 0; i < size; i++) {
        expect(track.values[i]).toBeCloseTo(initial.values[i], 5);
        expect(track.values[track.values.length - size + i]).toBeCloseTo(initial.values[i], 5);
      }
    }
  }
  const blink = gltf.animations[0].tracks.find((track) => track.name === '09_-_eye_left.scale')!;
  expect(Math.min(...Array.from(blink.values).filter((_, index) => index % 3 === 1))).toBeLessThan(
    0.01,
  );
  const jump = gltf.animations[2].tracks.find(
    (track) => track.name === 'Nooboard_Form_09.position',
  )!;
  expect(Math.max(...Array.from(jump.values).filter((_, index) => index % 3 === 1))).toBeCloseTo(
    0.3,
    5,
  );
});

it('fits the original stage, keeps props and interaction bounds stable, and lifts both feet', async () => {
  const [stage, bird] = await Promise.all([load('nooboard'), load('bird-wing-18')]);
  const oldBird = composeMotionPreview(stage, bird);
  expect(oldBird.parent).toBeNull();
  expect(stage.scene.getObjectByName('Woodboard_Root')).toBeDefined();
  expect(stage.scene.getObjectByName('Mailbox_Root')).toBeDefined();
  const root = stage.scene.getObjectByName('Bird_Root')!;
  const bounds = new Box3().setFromObject(root);
  expect(bounds.min.y).toBeCloseTo(0.0305, 3);
  expect(bounds.getSize(new Vector3()).y).toBeCloseTo(2.4331, 3);
  const targets = createTargets(stage.scene);
  const camera = new OrthographicCamera(-4.1, 4.1, 2.3, -2.3, 0.1, 100);
  camera.position.set(3, 1.25 + Math.hypot(3, 12.5) * Math.tan((5 * Math.PI) / 180), 12.6);
  camera.lookAt(0, 1.25, 0.1);
  const restingTargets = targets(camera);
  for (const rect of Object.values(restingTargets)) {
    expect(Object.values(rect).every(Number.isFinite)).toBe(true);
    expect(rect.width).toBeGreaterThan(0);
    expect(rect.height).toBeGreaterThan(0);
  }
  const feet = [...stage.scene.getObjectByName('Nooboard_Form_09')!.children].filter((node) =>
    node.name.includes('Foot'),
  );
  expect(feet).toHaveLength(2);
  const footY = feet.map((foot) => foot.getWorldPosition(new Vector3()).y);
  const mixer = new AnimationMixer(stage.scene);
  mixer.clipAction(stage.animations.find((clip) => clip.name === 'hop')!).play();
  mixer.update(0.9);
  stage.scene.updateMatrixWorld(true);
  feet.forEach((foot, index) => {
    expect(foot.getWorldPosition(new Vector3()).y - footY[index]).toBeCloseTo(
      0.3 * root.scale.y,
      5,
    );
  });
  expect(targets(camera)).toEqual(restingTargets);
});
