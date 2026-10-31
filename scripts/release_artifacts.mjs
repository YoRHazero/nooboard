import { readFileSync, readdirSync, statSync, writeFileSync } from 'node:fs';
import path from 'node:path';

// Names match the build matrix; every requested installer must be present.
const requiredArtifacts = {
  'nooboard-windows-x64': ['-setup.exe', '-setup.exe.sig'],
  'nooboard-macos-arm64': ['.dmg', '.app.tar.gz', '.app.tar.gz.sig'],
  'nooboard-linux-x64': ['.deb', '.AppImage', '.AppImage.sig'],
};

function filesIn(directory) {
  return readdirSync(directory, { withFileTypes: true }).flatMap((entry) => {
    const file = path.join(directory, entry.name);
    if (entry.isDirectory()) return filesIn(file);
    if (!entry.isFile()) throw new Error(`Installer must be a regular file: ${file}`);
    return [{ path: file, name: entry.name, size: statSync(file).size }];
  });
}

export function collectReleaseArtifacts(root) {
  const assets = [];
  for (const [artifact, suffixes] of Object.entries(requiredArtifacts)) {
    const files = filesIn(path.join(root, artifact));
    for (const suffix of suffixes) {
      const matches = files.filter((file) => file.name.endsWith(suffix));
      if (matches.length !== 1 || matches[0].size === 0) {
        throw new Error(`${artifact}: expected exactly one non-empty ${suffix} installer.`);
      }
      assets.push(matches[0]);
    }
    if (files.length !== suffixes.length) {
      throw new Error(`${artifact}: unexpected files in installer artifact.`);
    }
  }
  if (new Set(assets.map((asset) => asset.name)).size !== assets.length) {
    throw new Error('Installer filenames must be unique across platforms.');
  }
  return assets;
}

/** Only publish metadata after every platform has a signed updater payload. */
export function createUpdateManifest(root, assets, repo, tag) {
  const platforms = {};
  for (const [platform, suffix] of Object.entries({
    'windows-x86_64': '-setup.exe',
    'darwin-aarch64': '.app.tar.gz',
    'linux-x86_64': '.AppImage',
  })) {
    const payload = assets.find((asset) => asset.name.endsWith(suffix));
    const signatureFile = assets.find((asset) => asset.name === `${payload?.name}.sig`);
    if (!payload || !signatureFile) throw new Error(`Missing updater signature for ${platform}`);
    const signature = readFileSync(signatureFile.path, 'utf8').trim();
    if (
      !/^[A-Za-z0-9+/]+={0,2}$/.test(signature) ||
      !Buffer.from(signature, 'base64').toString('utf8').startsWith('untrusted comment: ')
    )
      throw new Error(`Invalid updater signature file: ${signatureFile.name}`);
    platforms[platform] = {
      signature,
      url: `https://github.com/${repo.owner}/${repo.repo}/releases/download/${encodeURIComponent(tag)}/${encodeURIComponent(payload.name)}`,
    };
  }
  const manifest = { version: tag.slice(1), platforms };
  const file = path.join(root, 'latest.json');
  writeFileSync(file, `${JSON.stringify(manifest, null, 2)}\n`);
  return { name: 'latest.json', path: file, size: statSync(file).size };
}
