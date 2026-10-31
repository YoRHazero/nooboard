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
    if (relative.startsWith('pages/')) {
      if (relative.split('/').length !== 2) throw new Error(`Pages stay flat: ${relative}`);
      for (const node of tree.program.body) {
        if (node.type === 'ImportDeclaration') {
          for (const specifier of node.specifiers) {
            if (/^use[A-Z]/.test(specifier.local.name))
              throw new Error(`Pages cannot import state hooks: ${relative}`);
          }
        }
      }
    }
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
      if (relative.startsWith('pages/')) {
        assert(!/^(app|preview)\//.test(target), 'pages only compose feature regions');
        assert(
          !target.startsWith('shared/backend') &&
            !target.startsWith('shared/queries/') &&
            !target.startsWith('shared/store/'),
          'pages do not access backend or state',
        );
        assert(
          !['zustand', '@tanstack/react-query'].includes(imported),
          'pages do not own state or queries',
        );
      }
      if (relative.startsWith('features/'))
        assert(
          !/^(app|pages|preview)\//.test(target),
          'features cannot depend on composition, pages or preview',
        );
      if (/^(features|pages)\//.test(relative) && target.startsWith('features/')) {
        const sameFeature =
          relative.startsWith('features/') && relative.split('/')[1] === target.split('/')[1];
        assert(
          sameFeature ||
            /^features\/[^/]+(?:\/index)?$/.test(target) ||
            /^features\/[^/]+\/queries\/use[A-Z]\w+$/.test(target),
          'cross-feature imports use index.ts or a public query/mutation hook',
        );
      }
      if (relative.startsWith('shared/'))
        assert(
          !/^(app|pages|features|preview)\//.test(target),
          'shared cannot depend on application or feature modules',
        );
      if (imported.startsWith('@tauri-apps/'))
        assert(
          ['shared/backend.ts', 'shared/native-menu.ts', 'app/bootstrap.tsx'].includes(relative),
          'platform imports belong to the backend boundary',
        );
      assert(!target.startsWith('client/'), 'the former client facade has been retired');
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
