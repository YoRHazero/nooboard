import fs from 'node:fs/promises';
import assert from 'node:assert/strict';
import path from 'node:path';

// Package Blender's baked samples; never generate geometry or author motion here.
const source = process.argv[2];
if (!source)
  throw new Error('Usage: node scripts/package-bird-motion.mjs <Blender scene.glb> [output.glb]');
function parseGlb(input) {
  assert.equal(input.readUInt32LE(0), 0x46546c67);
  assert.equal(input.readUInt32LE(4), 2);
  const jsonLength = input.readUInt32LE(12);
  return {
    document: JSON.parse(input.subarray(20, 20 + jsonLength).toString()),
    binary: input.subarray(28 + jsonLength),
  };
}
const { document, binary } = parseGlb(await fs.readFile(source));
assert.equal(document.animations.length, 1, 'Export in Scene mode, without Split by Object');
let animation = document.animations[0];
const destination = process.argv[3]
  ? path.resolve(process.argv[3])
  : new URL('../public/mascot/bird-motion-16.glb', import.meta.url);
const replaceNames = process.argv
  .find((arg) => arg.startsWith('--replace-clips='))
  ?.slice('--replace-clips='.length)
  .split(',');
const replaceNodes = process.argv
  .find((arg) => arg.startsWith('--replace-nodes='))
  ?.slice('--replace-nodes='.length)
  .split(',');
if (replaceNodes) {
  assert.ok(replaceNames, '--replace-nodes requires --replace-clips');
  const channels = animation.channels.filter((channel) =>
    replaceNodes.includes(document.nodes[channel.target.node].name),
  );
  assert.deepEqual(
    new Set(channels.map((channel) => document.nodes[channel.target.node].name)),
    new Set(replaceNodes),
    'Each selected node must have native sampled tracks',
  );
  animation = {
    ...animation,
    samplers: channels.map((channel) => animation.samplers[channel.sampler]),
    channels: channels.map((channel, sampler) => ({ ...channel, sampler })),
  };
}
const output = replaceNames ? parseGlb(await fs.readFile(destination)) : { document, binary };
const chunks = [output.binary];
let byteLength = output.binary.length;
const widths = { SCALAR: 1, VEC3: 3, VEC4: 4 };

function readAccessor(index, glb = { document, binary }) {
  const accessor = glb.document.accessors[index];
  const view = glb.document.bufferViews[accessor.bufferView];
  assert.equal(accessor.componentType, 5126);
  assert.ok(!view.byteStride && !accessor.sparse);
  const size = widths[accessor.type];
  assert.ok(size);
  const start = (view.byteOffset ?? 0) + (accessor.byteOffset ?? 0);
  const values = Array.from({ length: accessor.count * size }, (_, i) =>
    glb.binary.readFloatLE(start + i * 4),
  );
  return { values, size, type: accessor.type };
}

function appendAccessor(values, type) {
  const data = Buffer.alloc(values.length * 4);
  values.forEach((value, i) => data.writeFloatLE(value, i * 4));
  const bufferView = output.document.bufferViews.length;
  output.document.bufferViews.push({
    buffer: 0,
    byteOffset: byteLength,
    byteLength: data.length,
  });
  chunks.push(data);
  byteLength += data.length;
  const index = output.document.accessors.length;
  output.document.accessors.push({
    bufferView,
    componentType: 5126,
    count: values.length / widths[type],
    type,
    ...(type === 'SCALAR' ? { min: [values[0]], max: [values.at(-1)] } : {}),
  });
  return index;
}

const business = process.argv.includes('--business');
if (business) {
  const bird = document.nodes.find((node) => node.name === 'Bird_Root.001');
  assert.ok(bird, 'Business scene is missing the animated bird root');
  bird.name = 'Bird_Root';
  const excluded = new Set(
    document.nodes.flatMap((node, index) =>
      ['Animation studio floor', '空物体'].includes(node.name) ? [index] : [],
    ),
  );
  document.scenes.forEach((scene) => {
    scene.nodes = scene.nodes.filter((index) => !excluded.has(index));
  });
  assert.ok(animation.channels.every((channel) => !excluded.has(channel.target.node)));
}
const ranges = [
  ['idle', 1, 121, 30],
  ['curious', 141, 241, 30],
  ['hop', 261, 360, 30],
  ...(business
    ? [
        ['capture', 546, 613, 24],
        ['send', 687, 879, 24],
        ['receive_text', 880, 1078, 24],
        ['receive_file', 1079, 1215, 24],
        ['success', 1344, 1386, 24],
        ['error', 1387, 1447, 24],
        ['paused', 1448, 1520, 24],
        ['offline', 1521, 1617, 24],
        ['resume', 1618, 1678, 30],
      ]
    : []),
];
const selectedRanges = replaceNames
  ? ranges.filter(([name]) => replaceNames.includes(name))
  : ranges;
if (replaceNames) {
  assert.deepEqual(new Set(selectedRanges.map(([name]) => name)), new Set(replaceNames));
  for (const name of replaceNames)
    assert.ok(
      output.document.animations.some((clip) => clip.name === name),
      `Unknown clip ${name}`,
    );
}
const channels = animation.channels.map((channel) => {
  if (!replaceNames) return channel;
  const name = document.nodes[channel.target.node].name;
  const matches = output.document.nodes.flatMap((node, index) =>
    node.name === name ? [index] : [],
  );
  assert.equal(matches.length, 1, `Animation node must map uniquely: ${name}`);
  return { ...channel, target: { ...channel.target, node: matches[0] } };
});
const clips = selectedRanges.map(([name, first, last, fps]) => ({
  name,
  extras: {
    source: replaceNames
      ? output.document.animations.find((clip) => clip.name === name).extras.source
      : `${path.parse(source).name}.blend`,
    frames: [first, last],
    fps,
  },
  channels,
  samplers: animation.samplers.map((sampler) => {
    assert.ok(
      ['LINEAR', 'STEP'].includes(sampler.interpolation ?? 'LINEAR'),
      'Use sampled animation export',
    );
    const times = readAccessor(sampler.input).values;
    const output = readAccessor(sampler.output);
    const frames = new Map(times.map((time, i) => [Math.round(time * 30), i]));
    const constant =
      times.length === 2 &&
      output.values
        .slice(0, output.size)
        .every((value, i) => Math.abs(value - output.values[output.size + i]) < 1e-6);
    const selected = constant
      ? [first, last]
      : Array.from({ length: last - first + 1 }, (_, i) => first + i);
    const values = selected.flatMap((frame) => {
      const index = constant ? 0 : frames.get(frame);
      assert.notEqual(index, undefined, `Missing baked frame ${frame}`);
      return output.values.slice(index * output.size, (index + 1) * output.size);
    });
    return {
      input: appendAccessor(
        selected.map((frame) => (frame - first) / fps),
        'SCALAR',
      ),
      output: appendAccessor(values, output.type),
      interpolation: sampler.interpolation ?? 'LINEAR',
    };
  }),
}));
if (replaceNames) {
  const signature = (channel) => `${channel.target.node}:${channel.target.path}`;
  output.document.animations = output.document.animations.map((original) => {
    const replacement = clips.find((clip) => clip.name === original.name);
    if (!replacement) return original;
    const originalTargets = new Set(original.channels.map(signature));
    assert.ok(replacement.channels.every((channel) => originalTargets.has(signature(channel))));
    return {
      ...replacement,
      channels: original.channels.map((channel) => {
        const updated = replacement.channels.find(
          (candidate) => signature(candidate) === signature(channel),
        );
        if (updated) return updated;
        if (
          replaceNodes &&
          !replaceNodes.includes(output.document.nodes[channel.target.node].name)
        ) {
          const samplerIndex = replacement.samplers.length;
          replacement.samplers.push(original.samplers[channel.sampler]);
          return { ...channel, sampler: samplerIndex };
        }
        // A limited Scene export omits tracks constant over its whole range.
        // Retain their existing samples only when Blender's static transform agrees.
        const sampler = original.samplers[channel.sampler];
        const baked = readAccessor(sampler.output, output);
        const nodeName = output.document.nodes[channel.target.node].name;
        const node = document.nodes.find((candidate) => candidate.name === nodeName);
        assert.ok(node && !node.matrix, `Missing sampled node: ${nodeName}`);
        const defaults = { translation: [0, 0, 0], rotation: [0, 0, 0, 1], scale: [1, 1, 1] };
        const pose = node[channel.target.path] ?? defaults[channel.target.path];
        assert.ok(
          pose && baked.values.every((value, i) => Math.abs(value - pose[i % baked.size]) < 1e-5),
          `Missing track is not an unchanged constant: ${nodeName}.${channel.target.path}`,
        );
        const samplerIndex = replacement.samplers.length;
        replacement.samplers.push(sampler);
        return { ...channel, sampler: samplerIndex };
      }),
    };
  });
} else {
  output.document.animations = clips;
}
output.document.buffers = [{ byteLength }];
const encoded = Buffer.from(JSON.stringify(output.document));
const json = Buffer.alloc(Math.ceil(encoded.length / 4) * 4, 0x20);
encoded.copy(json);
const header = Buffer.alloc(20);
header.writeUInt32LE(0x46546c67, 0);
header.writeUInt32LE(2, 4);
header.writeUInt32LE(28 + json.length + byteLength, 8);
header.writeUInt32LE(json.length, 12);
header.writeUInt32LE(0x4e4f534a, 16);
const binHeader = Buffer.alloc(8);
binHeader.writeUInt32LE(byteLength, 0);
binHeader.writeUInt32LE(0x004e4942, 4);
await fs.writeFile(destination, Buffer.concat([header, json, binHeader, ...chunks]));
console.log(
  `Packaged ${selectedRanges.map(([name]) => name).join(', ')} → ${destination.pathname ?? destination}`,
);
