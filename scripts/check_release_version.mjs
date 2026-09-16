import { execFileSync } from 'node:child_process';
import { appendFileSync, readFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = fileURLToPath(new URL('../', import.meta.url));
const json = (file) => JSON.parse(readFileSync(path.join(root, file), 'utf8'));

/** Publishing a tag must never silently label a different app version. */
export function checkReleaseVersion(tag, versions) {
  const match =
    /^v(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)(?:-([0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*))?$/.exec(
      tag ?? '',
    );
  if (!match || match[0] !== tag || match[4]?.split('.').some((part) => /^0\d+$/.test(part))) {
    throw new Error('Expected a version tag such as v0.1.0 or v0.1.0-beta.1.');
  }
  const version = tag.slice(1);
  if (!Object.keys(versions).length) throw new Error('No app versions were found.');
  const mismatches = Object.entries(versions).filter(([, value]) => value !== version);
  if (mismatches.length) {
    throw new Error(
      `Tag ${tag} requires version ${version}:\n` +
        mismatches.map(([name, value]) => `  ${name}: ${value ?? '(missing)'}`).join('\n'),
    );
  }
  return { version, prerelease: Boolean(match[4]) };
}

function main() {
  const tag = process.argv[2] ?? process.env.RELEASE_TAG;
  const metadata = JSON.parse(
    execFileSync('cargo', ['metadata', '--no-deps', '--format-version', '1', '--locked'], {
      cwd: root,
      encoding: 'utf8',
    }),
  );
  const lock = json('apps/desktop/package-lock.json');
  const versions = {
    'apps/desktop/package.json': json('apps/desktop/package.json').version,
    'apps/desktop/package-lock.json': lock.version,
    'apps/desktop/package-lock.json (root package)': lock.packages?.['']?.version,
    'apps/desktop/src-tauri/tauri.conf.json': json('apps/desktop/src-tauri/tauri.conf.json')
      .version,
    ...Object.fromEntries(
      metadata.packages
        .filter((pkg) => metadata.workspace_members.includes(pkg.id))
        .map((pkg) => [pkg.name, pkg.version]),
    ),
  };
  const result = checkReleaseVersion(tag, versions);
  if (process.env.GITHUB_OUTPUT) {
    appendFileSync(
      process.env.GITHUB_OUTPUT,
      `version=${result.version}\nprerelease=${result.prerelease}\n`,
    );
  }
  console.log(
    `Release ${tag}: all app versions match (${result.prerelease ? 'prerelease' : 'stable'}).`,
  );
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  try {
    main();
  } catch (error) {
    console.error(error.message);
    process.exitCode = 1;
  }
}
