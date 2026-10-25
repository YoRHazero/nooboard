import fs from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { parse } from '@babel/parser';
const root = fileURLToPath(new URL('../src', import.meta.url));
async function scan(directory) {
  for (const entry of await fs.readdir(directory, { withFileTypes: true })) {
    const file = path.join(directory, entry.name);
    if (entry.isDirectory()) {
      await scan(file);
      continue;
    }
    if (!/\.tsx?$/.test(file) || /\.test\.tsx?$/.test(file) || file.endsWith('routeTree.gen.ts'))
      continue;
    const relative = path.relative(root, file).replaceAll('\\', '/');
    const tree = parse(await fs.readFile(file, 'utf8'), {
      sourceType: 'module',
      plugins: ['typescript', 'jsx'],
    });
    const imports = [];
    function visit(node) {
      if (!node || typeof node !== 'object') return;
      if (
        [
          'ImportDeclaration',
          'ExportNamedDeclaration',
          'ExportAllDeclaration',
          'ImportExpression',
        ].includes(node.type) &&
        node.source
      )
        imports.push(node.source.value);
      if (node.type === 'CallExpression' && node.callee.type === 'Import')
        imports.push(node.arguments[0]?.value);
      for (const value of Object.values(node))
        if (Array.isArray(value)) value.forEach(visit);
        else if (value && typeof value === 'object') visit(value);
    }
    visit(tree);
    for (const imported of imports) {
      if (typeof imported !== 'string') throw new Error(`Nonliteral import at ${relative}`);
      const target = imported.startsWith('.')
        ? path.relative(root, path.resolve(path.dirname(file), imported)).replaceAll('\\', '/')
        : imported;
      const assert = (valid, why) => {
        if (!valid) throw new Error(`${relative} -> ${imported}: ${why}`);
      };
      if (relative.startsWith('client/'))
        assert(
          !/^(app|pages|features|preview)\//.test(target),
          'client cannot depend on composition, pages or features',
        );
      if (/^(features|pages|app)\//.test(relative) && target.startsWith('client/'))
        assert(
          target === 'client/api' || target === 'client/react' || target === 'client/create',
          'use a public client entry',
        );
      if (/^(features|pages)\//.test(relative))
        assert(
          !/^(app|preview)\/|^@tauri-apps\//.test(target),
          'features cannot call IPC or select adapters',
        );
      if (relative.startsWith('features/'))
        assert(!target.startsWith('pages/'), 'features cannot depend on pages');
      if (/^(features|pages)\//.test(relative) && target.startsWith('features/')) {
        const sameFeature =
          relative.startsWith('features/') && relative.split('/')[1] === target.split('/')[1];
        assert(sameFeature || /\/api$/.test(target), 'cross-feature imports use the public api');
      }
      if (/^shared\//.test(relative))
        assert(
          !/^(app|pages|features|client|preview)\//.test(target),
          'shared utilities cannot depend on application layers',
        );
      if (imported.startsWith('@tauri-apps/'))
        assert(
          relative.startsWith('client/bridge/') || relative === 'app/bootstrap.tsx',
          'platform imports belong to the bridge',
        );
    }
  }
}
await scan(root);
const native = path.resolve(root, '../src-tauri');
const build = await fs.readFile(path.join(native, 'build.rs'), 'utf8');
const commands = [...build.matchAll(/"(connect|disconnect|request)"/g)].map((match) => match[1]);
if (commands.join(',') !== 'connect,disconnect,request')
  throw new Error('IPC build manifest must register the three facade commands');
const capability = JSON.parse(
  await fs.readFile(path.join(native, 'capabilities/main.json'), 'utf8'),
);
const expected = [
  'core:default',
  'os:allow-locale',
  ...commands.map((command) => `allow-${command}`),
];
if (JSON.stringify(capability.permissions) !== JSON.stringify(expected))
  throw new Error('IPC command permissions drifted from the facade');
console.log('Desktop module boundaries and IPC permissions passed.');
