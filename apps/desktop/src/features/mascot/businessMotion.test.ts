import { readFile } from 'node:fs/promises';
import { AnimationMixer, Box3, LoopOnce, Mesh, Object3D, Vector3 } from 'three';
import { GLTFLoader } from 'three/addons/loaders/GLTFLoader.js';
import { expect, it } from 'vitest';
import { previewMotions } from './motionPreview';

async function load() {
  const data = await readFile(
    new URL('../../../public/mascot/bird-business-19.glb', import.meta.url),
  );
  return new GLTFLoader().parseAsync(
    data.buffer.slice(data.byteOffset, data.byteOffset + data.byteLength),
    '',
  );
}

function verticesIn(object: Object3D, space: Object3D) {
  const points: Vector3[] = [];
  const inverse = space.matrixWorld.clone().invert();
  object.traverse((child) => {
    if (!(child instanceof Mesh)) return;
    for (let i = 0; i < child.geometry.attributes.position.count; i++) {
      points.push(
        child
          .getVertexPosition(i, new Vector3())
          .applyMatrix4(child.matrixWorld)
          .applyMatrix4(inverse),
      );
    }
  });
  return points;
}

it('exports every business performance with complete, finite transform tracks', async () => {
  const gltf = await load();
  expect(gltf.animations.map((clip) => clip.name)).toEqual(previewMotions);
  const signature = gltf.animations[0].tracks.map((track) => track.name);
  for (const clip of gltf.animations) {
    expect(clip.validate()).toBe(true);
    expect(clip.tracks.map((track) => track.name)).toEqual(signature);
    for (const track of clip.tracks)
      expect(Array.from(track.values).every(Number.isFinite)).toBe(true);
  }
  for (const name of ['Bird_Root', 'Woodboard_Root', 'Mailbox_Root', 'Paper_Carrier']) {
    expect(gltf.scene.getObjectByName(name), name).toBeDefined();
  }
});

it('holds the note at the beak while carrying it and restores open eyes after pausing', async () => {
  const gltf = await load();
  const mixer = new AnimationMixer(gltf.scene);
  const pose = (name: string, time: number) => {
    mixer.stopAllAction();
    const action = mixer.clipAction(gltf.animations.find((clip) => clip.name === name)!);
    action.reset().setLoop(LoopOnce, 1).play();
    action.time = time;
    mixer.update(0);
    gltf.scene.updateMatrixWorld(true);
  };
  for (const [name, time] of [
    ['send', 3.5],
    ['receive_text', 4.2],
  ] as const) {
    pose(name, time);
    const paper = gltf.scene.getObjectByName('Paper_Carrier')!;
    const beak = gltf.scene.getObjectByName('09_-_short_upper_beak')!;
    expect(
      paper.getWorldPosition(new Vector3()).distanceTo(beak.getWorldPosition(new Vector3())),
    ).toBeLessThan(0.04);
  }
  const eye = gltf.scene.getObjectByName('09_-_eye_left')!;
  pose('paused', 2);
  const closed = eye.scale.y;
  pose('offline', 0);
  expect(eye.scale.y).toBeGreaterThan(closed * 4);
  pose('resume', 1);
  expect(eye.scale.y).toBeGreaterThan(closed * 4);
  pose('idle', 0);
  expect(eye.scale.y).toBeGreaterThan(closed * 4);
});

it('keeps files in the mailbox and opens the door only for receiving', async () => {
  const gltf = await load();
  const mixer = new AnimationMixer(gltf.scene);
  const pose = (name: string, time: number) => {
    mixer.stopAllAction();
    const clip = gltf.animations.find((clip) => clip.name === name)!;
    const action = mixer.clipAction(clip).reset().setLoop(LoopOnce, 1).play();
    action.time = Math.min(time, clip.duration - 0.0001);
    mixer.update(0);
    gltf.scene.updateMatrixWorld(true);
  };
  const carrier = gltf.scene.getObjectByName('Paper_Carrier')!;
  const note = gltf.scene.getObjectByName('Mailbox_Note')!;
  const door = gltf.scene.getObjectByName('Mailbox_Door')!;
  pose('idle', 0);
  const closed = door.quaternion.clone();
  pose('send', 4.5);
  expect(door.quaternion.angleTo(closed)).toBeLessThan(0.001);
  // Paper has turned flat at the slot, rather than being dropped through an open door.
  pose('send', (796 - 687) / 24);
  expect(carrier.getWorldPosition(new Vector3()).y).toBeCloseTo(1.115, 3);
  pose('receive_text', 2.6);
  expect(door.quaternion.angleTo(closed)).toBeGreaterThan(0.6);
  for (const time of [0, 2.6, 4.2, 5.6]) {
    pose('receive_file', time);
    expect(carrier.getWorldScale(new Vector3()).length()).toBeLessThan(0.001);
    expect(note.getWorldScale(new Vector3()).length()).toBeGreaterThan(1);
  }
  const file = gltf.animations.find((clip) => clip.name === 'receive_file')!;
  expect(file.duration).toBeCloseTo(136 / 24, 4);
});

it('supports the incoming note and leaves clearance between the bird and mailbox', async () => {
  const gltf = await load();
  const mixer = new AnimationMixer(gltf.scene);
  const mailbox = gltf.scene.getObjectByName('Mailbox_Root')!;
  const tray = gltf.scene.getObjectByName('19_-_Sliding_letter_tray');
  expect(tray, 'receiving paper needs a physical support').toBeDefined();
  const poseFrame = (name: string, frame: number, start: number) => {
    mixer.stopAllAction();
    const action = mixer.clipAction(gltf.animations.find((clip) => clip.name === name)!);
    action.reset().setLoop(LoopOnce, 1).play();
    action.time = (frame - start) / 24;
    mixer.update(0);
    gltf.scene.updateMatrixWorld(true);
  };
  for (const [name, start, from, to] of [
    ['send', 687, 788, 802],
    ['receive_text', 880, 932, 950],
    ['receive_file', 1079, 1131, 1149],
  ] as const) {
    for (let frame = from; frame <= to; frame++) {
      poseFrame(name, frame, start);
      const body = new Box3().setFromPoints(
        verticesIn(gltf.scene.getObjectByName('Body_Mesh')!, mailbox),
      );
      // glTF local +Z points out of the mailbox; the front plane is at +0.53.
      expect(body.min.z, `${name} frame ${frame}: chest enters the box`).toBeGreaterThan(0.6);
    }
  }
  for (const frame of [930, 933, 936]) {
    poseFrame('receive_text', frame, 880);
    const paper = new Box3().setFromPoints(
      verticesIn(gltf.scene.getObjectByName('Mailbox_Note_Single_paper_note')!, mailbox),
    );
    const support = new Box3().setFromPoints(verticesIn(tray!, mailbox));
    expect(paper.max.y - paper.min.y, `frame ${frame}: paper is not flat`).toBeLessThan(0.025);
    expect(
      paper.min.y - support.max.y,
      `frame ${frame}: paper intersects tray`,
    ).toBeGreaterThanOrEqual(-0.005);
    expect(paper.min.y - support.max.y).toBeLessThan(0.025);
    expect(paper.min.x).toBeGreaterThan(support.min.x);
    expect(paper.max.x).toBeLessThan(support.max.x);
    expect(paper.min.z).toBeGreaterThanOrEqual(support.min.z - 0.01);
    expect(paper.max.z).toBeLessThanOrEqual(support.max.z + 0.01);
  }
}, 60000);

it('passes the delivery paper through the aperture rather than the solid door', async () => {
  const gltf = await load();
  const mixer = new AnimationMixer(gltf.scene);
  const action = mixer.clipAction(gltf.animations.find((clip) => clip.name === 'send')!);
  action.reset().setLoop(LoopOnce, 1).play();
  const mailbox = gltf.scene.getObjectByName('Mailbox_Root')!;
  const sheet = gltf.scene.getObjectByName('Single_paper_note') as Mesh;
  let crossings = 0;
  for (let frame = 785; frame <= 802; frame += 0.25) {
    action.time = (frame - 687) / 24;
    mixer.update(0);
    gltf.scene.updateMatrixWorld(true);
    if (sheet.getWorldScale(new Vector3()).length() < 0.02) continue;
    const points = verticesIn(sheet, mailbox);
    const indices = sheet.geometry.index;
    const count = indices?.count ?? points.length;
    for (let triangle = 0; triangle < count; triangle += 3) {
      for (let edge = 0; edge < 3; edge++) {
        const a = points[indices ? indices.getX(triangle + edge) : triangle + edge];
        const next = triangle + ((edge + 1) % 3);
        const b = points[indices ? indices.getX(next) : next];
        const t = (0.611 - a.z) / (b.z - a.z);
        if (!Number.isFinite(t) || t < 0 || t > 1) continue;
        const intersection = a.clone().lerp(b, t);
        if (Math.abs(intersection.x) > 0.595 || intersection.y < 0.245 || intersection.y > 1.655)
          continue;
        crossings++;
        expect(
          Math.abs(intersection.x),
          `frame ${frame}: paper catches the slot side`,
        ).toBeLessThan(0.379);
        expect(
          Math.abs(intersection.y - 1.115),
          `frame ${frame}: paper crosses solid door`,
        ).toBeLessThan(0.033);
      }
    }
  }
  expect(crossings, 'the delivery must actually enter the mailbox').toBeGreaterThan(0);
});

it('reveals the carried note at the receiving handoff without flying in from the board', async () => {
  const gltf = await load();
  const mixer = new AnimationMixer(gltf.scene);
  const action = mixer.clipAction(gltf.animations.find((clip) => clip.name === 'receive_text')!);
  action.reset().setLoop(LoopOnce, 1).play();
  const paper = gltf.scene.getObjectByName('Paper_Carrier')!;
  const beak = gltf.scene.getObjectByName('09_-_short_upper_beak')!;
  for (let frame = 937.25; frame <= 941; frame += 0.25) {
    action.time = (frame - 880) / 24;
    mixer.update(0);
    gltf.scene.updateMatrixWorld(true);
    expect(
      paper.getWorldPosition(new Vector3()).distanceTo(beak.getWorldPosition(new Vector3())),
      `frame ${frame}: visible paper jumps away from the handoff`,
    ).toBeLessThan(0.12);
  }
});

it('withdraws the receiving tray before the carried paper tips downward', async () => {
  const gltf = await load();
  const mixer = new AnimationMixer(gltf.scene);
  const action = mixer.clipAction(gltf.animations.find((clip) => clip.name === 'receive_text')!);
  action.reset().setLoop(LoopOnce, 1).play();
  const tray = gltf.scene.getObjectByName('19_-_Sliding_letter_tray') as Mesh;
  const sheet = gltf.scene.getObjectByName('Single_paper_note') as Mesh;
  tray.geometry.computeBoundingBox();
  const bounds = tray.geometry.boundingBox!;
  for (let frame = 938; frame <= 954; frame += 0.25) {
    action.time = (frame - 880) / 24;
    mixer.update(0);
    gltf.scene.updateMatrixWorld(true);
    const points = verticesIn(sheet, tray);
    const indices = sheet.geometry.index;
    const count = indices?.count ?? points.length;
    let crossings = 0;
    for (let triangle = 0; triangle < count; triangle += 3) {
      for (let edge = 0; edge < 3; edge++) {
        const a = points[indices ? indices.getX(triangle + edge) : triangle + edge];
        const next = triangle + ((edge + 1) % 3);
        const b = points[indices ? indices.getX(next) : next];
        const t = (bounds.max.y - a.y) / (b.y - a.y);
        if (!Number.isFinite(t) || t < 0 || t > 1) continue;
        const intersection = a.clone().lerp(b, t);
        if (
          intersection.x > bounds.min.x + 0.01 &&
          intersection.x < bounds.max.x - 0.01 &&
          intersection.z > bounds.min.z + 0.01 &&
          intersection.z < bounds.max.z - 0.01
        )
          crossings++;
      }
    }
    expect(crossings, `frame ${frame}: carried paper passes through the tray`).toBe(0);
  }
});

it('keeps the bird and carried sheet in front of the board throughout both handoffs', async () => {
  const gltf = await load();
  const mixer = new AnimationMixer(gltf.scene);
  const board = gltf.scene.getObjectByName('Woodboard_Root')!;
  const wood = gltf.scene.getObjectByName('Rounded_oak_writing_board')!;
  const body = gltf.scene.getObjectByName('Body_Mesh')!;
  const sheet = gltf.scene.getObjectByName('Single_paper_note')!;
  for (const [name, start, from, to] of [
    ['send', 687, 687, 750],
    ['receive_text', 880, 1000, 1077],
    ['receive_file', 1079, 1079, 1116],
  ] as const) {
    mixer.stopAllAction();
    const action = mixer.clipAction(gltf.animations.find((clip) => clip.name === name)!);
    action.reset().setLoop(LoopOnce, 1).play();
    for (let frame = from; frame <= to; frame += 0.5) {
      action.time = (frame - start) / 24;
      mixer.update(0);
      gltf.scene.updateMatrixWorld(true);
      const bounds = new Box3().setFromPoints(verticesIn(wood, board));
      for (const object of [body, sheet]) {
        if (object.getWorldScale(new Vector3()).length() < 0.02) continue;
        const overlapping = verticesIn(object, board).filter(
          (point) =>
            point.x > bounds.min.x + 0.005 &&
            point.x < bounds.max.x - 0.005 &&
            point.y > bounds.min.y + 0.005 &&
            point.y < bounds.max.y - 0.005,
        );
        if (!overlapping.length) continue;
        const clearance =
          overlapping.reduce((min, point) => Math.min(min, point.z), Infinity) - bounds.max.z;
        expect(
          clearance,
          `${name} frame ${frame}: ${object.name} enters the writing board`,
        ).toBeGreaterThanOrEqual(-0.002);
      }
    }
  }
}, 60000);

it('supports the mailbox tray on side rails and rests the file on that tray', async () => {
  const gltf = await load();
  const mixer = new AnimationMixer(gltf.scene);
  const mailbox = gltf.scene.getObjectByName('Mailbox_Root')!;
  const tray = gltf.scene.getObjectByName('19_-_Sliding_letter_tray')!;
  const rails = ['20_-_Tray_rail_L', '20_-_Tray_rail_R'].map((name) => {
    const rail = gltf.scene.getObjectByName(name);
    expect(rail, 'tray needs visible support from the mailbox walls').toBeDefined();
    return rail!;
  });
  for (const [name, start, frames] of [
    ['receive_text', 880, [930, 933, 936, 940, 943]],
    ['receive_file', 1079, [1100, 1135, 1140, 1150]],
  ] as const) {
    mixer.stopAllAction();
    const action = mixer.clipAction(gltf.animations.find((clip) => clip.name === name)!);
    action.reset().setLoop(LoopOnce, 1).play();
    for (const frame of frames) {
      action.time = (frame - start) / 24;
      mixer.update(0);
      gltf.scene.updateMatrixWorld(true);
      const support = new Box3().setFromPoints(verticesIn(tray, mailbox));
      for (const rail of rails) {
        const bounds = new Box3().setFromPoints(verticesIn(rail, mailbox));
        expect(Math.abs(support.min.y - bounds.max.y)).toBeLessThan(0.005);
        expect(Math.max(Math.abs(bounds.min.x), Math.abs(bounds.max.x))).toBeGreaterThan(0.595);
        expect(
          Math.min(bounds.max.x, support.max.x) - Math.max(bounds.min.x, support.min.x),
        ).toBeGreaterThan(0.05);
        expect(
          Math.min(bounds.max.z, support.max.z) - Math.max(bounds.min.z, support.min.z),
        ).toBeGreaterThan(0.3);
      }
      if (name !== 'receive_file') continue;
      const paper = new Box3().setFromPoints(
        verticesIn(gltf.scene.getObjectByName('Mailbox_Note_Single_paper_note')!, mailbox),
      );
      expect(paper.min.y - support.max.y).toBeGreaterThanOrEqual(0);
      expect(paper.min.y - support.max.y).toBeLessThan(0.005);
      expect(paper.min.x).toBeGreaterThan(support.min.x);
      expect(paper.max.x).toBeLessThan(support.max.x);
      expect(paper.min.z).toBeGreaterThanOrEqual(support.min.z - 0.005);
      expect(paper.max.z).toBeLessThanOrEqual(support.max.z + 0.005);
    }
  }
});

it('returns from the board to the idle standing position', async () => {
  const gltf = await load();
  const mixer = new AnimationMixer(gltf.scene);
  const bird = gltf.scene.getObjectByName('Bird_Root')!;
  const position = (name: string, atEnd = false) => {
    mixer.stopAllAction();
    const clip = gltf.animations.find((clip) => clip.name === name)!;
    const action = mixer.clipAction(clip).reset().setLoop(LoopOnce, 1).play();
    action.time = atEnd ? clip.duration - 0.0001 : 0;
    mixer.update(0);
    gltf.scene.updateMatrixWorld(true);
    return bird.getWorldPosition(new Vector3());
  };
  const rest = position('idle');
  for (const clip of previewMotions) {
    expect(position(clip).distanceTo(rest), clip + ' start').toBeLessThan(0.01);
    expect(position(clip, true).distanceTo(rest), clip + ' end').toBeLessThan(0.01);
  }
});
