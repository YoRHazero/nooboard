import assert from 'node:assert/strict';
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { test } from 'node:test';
import { collectReleaseArtifacts } from './release_artifacts.mjs';
import { publishRelease } from './publish_release.mjs';

function installers(t) {
  const root = mkdtempSync(path.join(os.tmpdir(), 'nooboard-release-'));
  t.after(() => rmSync(root, { recursive: true, force: true }));
  const files = [
    'nooboard-windows-x64/nooboard_0.1.0_x64-setup.exe',
    'nooboard-windows-x64/nooboard_0.1.0_x64-setup.exe.sig',
    'nooboard-macos-arm64/nooboard_0.1.0_aarch64.dmg',
    'nooboard-macos-arm64/nooboard.app.tar.gz',
    'nooboard-macos-arm64/nooboard.app.tar.gz.sig',
    'nooboard-linux-x64/deb/nooboard_0.1.0_amd64.deb',
    'nooboard-linux-x64/appimage/nooboard_0.1.0_amd64.AppImage',
    'nooboard-linux-x64/appimage/nooboard_0.1.0_amd64.AppImage.sig',
  ].map((file) => path.join(root, file));
  for (const file of files) {
    mkdirSync(path.dirname(file), { recursive: true });
    writeFileSync(
      file,
      file.endsWith('.sig')
        ? Buffer.from(
            'untrusted comment: test fixture\nsignature\ntrusted comment: timestamp:0\nglobal signature\n',
          ).toString('base64')
        : `installer: ${path.basename(file)}`,
    );
  }
  return { root, files };
}

function githubStub(options = {}) {
  const state = {
    release: null,
    assets: [],
    calls: [],
    failUpload: 0,
    ...options,
  };
  let nextId = 10;
  let uploads = 0;
  const record = (name, params) => state.calls.push({ name, params });
  const api = {
    async listReleases(params) {
      record('listReleases', params);
      if (state.lookupError) throw state.lookupError;
      return { data: state.release ? [state.release] : [] };
    },
    async createRelease(params) {
      record('createRelease', params);
      state.release = { id: 1, ...params };
      return { data: state.release };
    },
    async listReleaseAssets(params) {
      record('listReleaseAssets', params);
      return { data: [...state.assets] };
    },
    async deleteReleaseAsset(params) {
      record('deleteReleaseAsset', params);
      state.assets = state.assets.filter((asset) => asset.id !== params.asset_id);
    },
    async uploadReleaseAsset(params) {
      record('uploadReleaseAsset', params);
      if (++uploads === state.failUpload) throw new Error('Upload interrupted');
      assert.ok(Buffer.isBuffer(params.data));
      assert.equal(params.headers['content-length'], params.data.length);
      const asset = {
        id: nextId++,
        name: params.name,
        size: params.data.length,
        state: state.incompleteUpload ? 'starter' : 'uploaded',
      };
      state.assets.push(asset);
      return { data: asset };
    },
    async updateRelease(params) {
      record('updateRelease', params);
      Object.assign(state.release, params, {
        html_url: 'https://example.test/release',
      });
      return { data: state.release };
    },
  };
  return {
    state,
    github: {
      rest: { repos: api },
      paginate: async (method, params) => (await method(params)).data,
    },
  };
}

function request(root, stub, tag = 'v0.1.0') {
  return {
    github: stub.github,
    repo: { owner: 'example', repo: 'nooboard' },
    tag,
    artifactRoot: root,
    body: 'Installers for all supported platforms.',
  };
}

test('collects all installers and updater signatures, including nested bundles', (t) => {
  const { root, files } = installers(t);
  const assets = collectReleaseArtifacts(root);
  assert.deepEqual(
    assets.map((asset) => asset.path),
    files,
  );
});

test('blocks publication before any API call if a platform or bundle is missing', async (t) => {
  for (const missing of ['nooboard-macos-arm64', 'nooboard-linux-x64/appimage']) {
    const { root } = installers(t);
    rmSync(path.join(root, missing), { recursive: true });
    const stub = githubStub();
    await assert.rejects(publishRelease(request(root, stub)), /ENOENT|expected exactly one/);
    assert.equal(stub.state.calls.length, 0);
  }
});

test('rejects empty, duplicate and unexpected installer files', (t) => {
  const empty = installers(t);
  writeFileSync(empty.files[0], '');
  assert.throws(() => collectReleaseArtifacts(empty.root), /non-empty/);

  const duplicate = installers(t);
  writeFileSync(path.join(duplicate.root, 'nooboard-macos-arm64/extra.dmg'), 'extra');
  assert.throws(() => collectReleaseArtifacts(duplicate.root), /exactly one/);

  const extra = installers(t);
  writeFileSync(path.join(extra.root, 'nooboard-windows-x64/extra.txt'), 'extra');
  assert.throws(() => collectReleaseArtifacts(extra.root), /unexpected files/);
});

test('creates a draft and publishes only after uploading and checking every installer', async (t) => {
  const { root } = installers(t);
  const stub = githubStub();
  assert.equal(await publishRelease(request(root, stub)), 'https://example.test/release');
  const { calls } = stub.state;
  const creation = calls.find((call) => call.name === 'createRelease');
  assert.equal(creation.params.draft, true);
  assert.equal(creation.params.tag_name, 'v0.1.0');
  assert.deepEqual(
    calls.slice(-11).map((call) => call.name),
    [...Array(9).fill('uploadReleaseAsset'), 'listReleaseAssets', 'updateRelease'],
  );
  assert.equal(calls.at(-1).params.draft, false);
  assert.equal(calls.at(-1).params.prerelease, false);
  assert.equal(calls.at(-1).params.make_latest, 'legacy');
});

test('publishes beta tags as prereleases without replacing the latest stable release', async (t) => {
  const { root } = installers(t);
  const stub = githubStub();
  await publishRelease(request(root, stub, 'v0.2.0-beta.1'));
  assert.equal(
    stub.state.calls.find((call) => call.name === 'createRelease').params.prerelease,
    true,
  );
  assert.equal(stub.state.calls.at(-1).params.prerelease, true);
  assert.equal(stub.state.calls.at(-1).params.make_latest, 'false');
});

test('an interrupted upload stays draft and a retry resumes the same release', async (t) => {
  const { root } = installers(t);
  const stub = githubStub({ failUpload: 3 });
  await assert.rejects(publishRelease(request(root, stub)), /Upload interrupted/);
  assert.equal(stub.state.release.draft, true);
  assert.equal(stub.state.assets.length, 2);
  assert.ok(!stub.state.calls.some((call) => call.name === 'updateRelease'));

  stub.state.failUpload = 0;
  await publishRelease(request(root, stub));
  assert.equal(stub.state.release.draft, false);
  assert.equal(stub.state.assets.length, 9);
  const count = (name) => stub.state.calls.filter((call) => call.name === name).length;
  assert.equal(count('createRelease'), 1);
  assert.equal(count('deleteReleaseAsset'), 2);
  assert.equal(count('updateRelease'), 1);
});

test('incomplete remote uploads cannot be published', async (t) => {
  const { root } = installers(t);
  const stub = githubStub({ incompleteUpload: true });
  await assert.rejects(publishRelease(request(root, stub)), /remains a draft/);
  assert.equal(stub.state.release.draft, true);
  assert.ok(!stub.state.calls.some((call) => call.name === 'updateRelease'));
});

test('never modifies a published release or a draft containing unrelated assets', async (t) => {
  const { root } = installers(t);
  for (const options of [
    { release: { id: 1, tag_name: 'v0.1.0', draft: false } },
    {
      release: { id: 1, tag_name: 'v0.1.0', draft: true },
      assets: [{ id: 9, name: 'unrelated.zip' }],
    },
  ]) {
    const stub = githubStub(options);
    await assert.rejects(
      publishRelease(request(root, stub)),
      /already published|unexpected assets/,
    );
    assert.ok(stub.state.calls.every((call) => call.name.startsWith('list')));
  }
});

test('lookup failures stop publication instead of being treated as a missing release', async (t) => {
  const { root } = installers(t);
  const stub = githubStub({ lookupError: new Error('API unavailable') });
  await assert.rejects(publishRelease(request(root, stub)), /API unavailable/);
  assert.deepEqual(
    stub.state.calls.map((call) => call.name),
    ['listReleases'],
  );
});

test('metadata points to the signed payloads of this exact release', async (t) => {
  const { root } = installers(t);
  const stub = githubStub();
  await publishRelease(request(root, stub));
  const uploads = stub.state.calls.filter((call) => call.name === 'uploadReleaseAsset');
  const manifest = JSON.parse(uploads.at(-1).params.data.toString());
  assert.equal(uploads.at(-1).params.name, 'latest.json');
  assert.equal(manifest.version, '0.1.0');
  assert.deepEqual(Object.keys(manifest.platforms).sort(), [
    'darwin-aarch64',
    'linux-x86_64',
    'windows-x86_64',
  ]);
  for (const [platform, entry] of Object.entries(manifest.platforms)) {
    assert.ok(
      entry.url.startsWith('https://github.com/example/nooboard/releases/download/v0.1.0/'),
    );
    const name = decodeURIComponent(entry.url.split('/').at(-1));
    const signature = uploads.find((call) => call.params.name === `${name}.sig`);
    assert.equal(entry.signature, signature.params.data.toString());
    if (platform === 'darwin-aarch64') assert.equal(name, 'nooboard.app.tar.gz');
  }
});

test('missing, mismatched and malformed signatures stop before any API mutation', async (t) => {
  for (const mutation of [
    (file) => rmSync(file),
    (file) => writeFileSync(file, ''),
    (file) => writeFileSync(file, 'not a signature'),
    (file) => {
      rmSync(file);
      writeFileSync(path.join(path.dirname(file), 'other.app.tar.gz.sig'), 'wrong');
    },
  ]) {
    const { root, files } = installers(t);
    mutation(files.find((file) => file.endsWith('.app.tar.gz.sig')));
    const stub = githubStub();
    await assert.rejects(publishRelease(request(root, stub)), /non-empty|signature/);
    assert.equal(stub.state.calls.length, 0);
  }
});
