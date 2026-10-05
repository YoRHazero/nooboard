import { readFile } from 'node:fs/promises';
import { Vector3 } from 'three';
import { GLTFLoader } from 'three/addons/loaders/GLTFLoader.js';
import { expect, it, vi } from 'vitest';
import { ClipPlayer } from './clipPlayer';

async function load() {
  const data = await readFile(
    new URL('../../../public/mascot/bird-business-19.glb', import.meta.url),
  );
  return new GLTFLoader().parseAsync(
    data.buffer.slice(data.byteOffset, data.byteOffset + data.byteLength),
    '',
  );
}

it('restores props exactly at business boundaries, without a visible paper flying across the stage', async () => {
  const gltf = await load();
  const player = new ClipPlayer(gltf.scene, gltf.animations, vi.fn());
  const carrier = gltf.scene.getObjectByName('Paper_Carrier')!;
  const beak = gltf.scene.getObjectByName('09_-_short_upper_beak')!;
  for (const clip of ['send', 'receive_text', 'receive_file'] as const) {
    player.play(clip);
    player.seek(0.9999);
    player.play('idle');
    for (let step = 0; step < 12; step++) {
      player.update(1 / 60);
      gltf.scene.updateMatrixWorld(true);
      expect(carrier.getWorldScale(new Vector3()).length(), `${clip} exit`).toBeLessThan(0.001);
    }
  }
  player.play('receive_text');
  player.seek(4.2 / player.current!.getClip().duration);
  gltf.scene.updateMatrixWorld(true);
  expect(
    carrier.getWorldPosition(new Vector3()).distanceTo(beak.getWorldPosition(new Vector3())),
  ).toBeLessThan(0.04);
  // Hidden windows reset immediately, even in the middle of carrying a letter.
  player.play('idle');
  gltf.scene.updateMatrixWorld(true);
  expect(carrier.getWorldScale(new Vector3()).length()).toBeLessThan(0.001);
  player.dispose(gltf.scene);
});

it('finishes only the current action and restores the sleeping/open-eye poses', async () => {
  const gltf = await load();
  const complete = vi.fn();
  const player = new ClipPlayer(gltf.scene, gltf.animations, complete);
  player.play('curious');
  player.update(0.4);
  player.play('hop');
  for (let frame = 0; frame < 400; frame++) player.update(1 / 60);
  expect(complete).toHaveBeenCalledTimes(1);
  const eye = gltf.scene.getObjectByName('09_-_eye_left')!;
  player.play('paused', 4);
  const closed = eye.scale.y;
  player.play('idle', 0);
  expect(eye.scale.y).toBeGreaterThan(closed * 4);
  player.dispose(gltf.scene);
});
