import { execFileSync } from 'node:child_process';
import { copyFile, mkdtemp, mkdir, rm } from 'node:fs/promises';
import { createRequire } from 'node:module';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const desktop = fileURLToPath(new URL('..', import.meta.url));
const source = path.join(desktop, 'assets/brand/q-bird.png');
const cli = createRequire(import.meta.url).resolve('@tauri-apps/cli/tauri.js');
const temporary = await mkdtemp(path.join(tmpdir(), 'nooboard-icons-'));

// Resize and encode the approved artwork; do not redraw or crop the character.
function generate(output, sizes = []) {
  execFileSync(
    process.execPath,
    [cli, 'icon', source, '--output', output, ...sizes.flatMap((size) => ['--png', String(size)])],
    { cwd: desktop, stdio: 'inherit' },
  );
}

try {
  const native = path.join(temporary, 'native');
  const ui = path.join(temporary, 'ui');
  generate(native);
  generate(ui, [64, 256]);
  const icons = path.join(desktop, 'src-tauri/icons');
  await mkdir(icons, { recursive: true });
  for (const name of [
    '32x32.png',
    '128x128.png',
    '128x128@2x.png',
    'icon.png',
    'icon.icns',
    'icon.ico',
  ]) {
    await copyFile(path.join(native, name), path.join(icons, name));
  }
  await copyFile(path.join(ui, '64x64.png'), path.join(icons, '64x64.png'));
  await copyFile(path.join(ui, '256x256.png'), path.join(desktop, 'public/mascot/icon.png'));
  console.log(
    'Updated desktop bundle icons and the shared UI mascot from assets/brand/q-bird.png.',
  );
} finally {
  await rm(temporary, { recursive: true, force: true });
}
