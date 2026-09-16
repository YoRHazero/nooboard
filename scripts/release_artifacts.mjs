import { readdirSync, statSync } from 'node:fs';
import path from 'node:path';

// Names match the build matrix; every requested installer must be present.
const requiredArtifacts = {
  'nooboard-windows-x64': ['-setup.exe'],
  'nooboard-macos-arm64': ['.dmg'],
  'nooboard-linux-x64': ['.deb', '.AppImage'],
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
