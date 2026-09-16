import assert from 'node:assert/strict';
import { test } from 'node:test';
import { checkReleaseVersion } from './check_release_version.mjs';

test('accepts matching stable versions', () => {
  assert.deepEqual(checkReleaseVersion('v0.1.0', { tauri: '0.1.0', cargo: '0.1.0' }), {
    version: '0.1.0',
    prerelease: false,
  });
});

test('marks a matching beta as a prerelease', () => {
  assert.deepEqual(checkReleaseVersion('v0.2.0-beta.1', { tauri: '0.2.0-beta.1' }), {
    version: '0.2.0-beta.1',
    prerelease: true,
  });
});

test('fails when a manifest or lockfile has a different or missing version', () => {
  for (const other of ['0.0.9', '0.1.0-beta.1', undefined]) {
    assert.throws(
      () => checkReleaseVersion('v0.1.0', { tauri: '0.1.0', lockfile: other }),
      /lockfile:/,
    );
  }
  assert.throws(() => checkReleaseVersion('v0.1.0', {}), /No app versions/);
});

test('rejects branch names, malformed versions and output injection', () => {
  for (const tag of [
    undefined,
    'main',
    '0.1.0',
    'v1',
    'v01.2.3',
    'v1.2.3-beta.01',
    'v1.2.3-',
    'v1.2.3+metadata',
    'v1.2.3\nprerelease=false',
    'v1.2.3\n',
  ]) {
    assert.throws(() => checkReleaseVersion(tag, { tauri: '0.1.0' }), /Expected a version tag/);
  }
});
