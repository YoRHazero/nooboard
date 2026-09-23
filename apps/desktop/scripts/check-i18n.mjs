import { readFile, readdir } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';
import path from 'node:path';
import assert from 'node:assert/strict';
import { parse } from '@babel/parser';
const root = fileURLToPath(new URL('../src/', import.meta.url));
const read = (file) => readFile(file, 'utf8');
const files = async (dir) =>
  (
    await Promise.all(
      (await readdir(dir, { withFileTypes: true })).map((entry) =>
        entry.isDirectory() ? files(path.join(dir, entry.name)) : path.join(dir, entry.name),
      ),
    )
  ).flat();
const placeholders = (text) =>
  [...text.matchAll(/{{\s*([^}]+)\s*}}/g)].map((match) => match[1].trim()).sort();
const locales = path.join(root, 'i18n/locales');
const english = await readdir(path.join(locales, 'en'));
assert.deepEqual((await readdir(path.join(locales, 'zh-CN'))).sort(), english.sort());
let count = 0;
for (const namespace of english) {
  const en = JSON.parse(await read(path.join(locales, 'en', namespace)));
  const zh = JSON.parse(await read(path.join(locales, 'zh-CN', namespace)));
  assert.deepEqual(Object.keys(en).sort(), Object.keys(zh).sort(), `${namespace}: keys differ`);
  for (const key of Object.keys(en)) {
    assert(en[key].trim() && zh[key].trim(), `${namespace}:${key}: empty translation`);
    assert.deepEqual(
      placeholders(en[key]),
      placeholders(zh[key]),
      `${namespace}:${key}: interpolation differs`,
    );
    assert(!/\$\{\d+\}/.test(en[key] + zh[key]), `${namespace}:${key}: unconverted interpolation`);
    if (key.endsWith('_one'))
      assert(Object.hasOwn(en, key.replace(/_one$/, '_other')), `${key}: missing plural`);
    count++;
  }
}
// User-authored sample data is intentionally untranslated. UI labels belong in resources.
function visit(node, file) {
  if (!node || typeof node !== 'object') return;
  const text = ['StringLiteral', 'JSXText', 'TemplateElement'].includes(node.type)
    ? node.type === 'TemplateElement'
      ? node.value.cooked
      : node.value
    : '';
  assert(
    !/\p{Script=Han}/u.test(text ?? ''),
    `${path.relative(root, file)}:${node.loc?.start.line}: hardcoded UI text`,
  );
  for (const [key, value] of Object.entries(node)) {
    if (['comments', 'loc', 'leadingComments', 'trailingComments', 'innerComments'].includes(key))
      continue;
    if (Array.isArray(value)) value.forEach((child) => visit(child, file));
    else if (value && typeof value === 'object') visit(value, file);
  }
}
for (const file of await files(root)) {
  if (
    !/\.tsx?$/.test(file) ||
    file.includes('.test.') ||
    file.startsWith(path.join(root, 'i18n')) ||
    file === path.join(root, 'preview/scenarios/fixtures.ts')
  )
    continue;
  visit(parse(await read(file), { sourceType: 'module', plugins: ['typescript', 'jsx'] }), file);
}
console.log(`i18n: ${count} bilingual messages checked; UI sources contain no hardcoded Chinese.`);
