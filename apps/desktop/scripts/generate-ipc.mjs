import fs from 'node:fs/promises';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import path from 'node:path';
import prettier from 'prettier';

const root = fileURLToPath(new URL('..', import.meta.url));
const check = process.argv.includes('--check');
if (!check) {
  const result = spawnSync(
    'cargo',
    [
      'test',
      '-p',
      'nooboard-desktop',
      '--locked',
      '--no-default-features',
      'ipc_schema_matches_rust_contract',
    ],
    {
      cwd: root,
      stdio: 'inherit',
      env: { ...process.env, NOOBOARD_UPDATE_IPC: '1' },
    },
  );
  if (result.status !== 0) process.exit(result.status ?? 1);
}
const schema = JSON.parse(await fs.readFile(path.join(root, 'ipc/schema.json'), 'utf8'));
// JSON Schema is emitted by schemars from the actual Rust DTOs. Fail on an
// unsupported shape rather than silently weakening the protocol to `any`.
function type(value) {
  if (value.$ref) return value.$ref.split('/').at(-1);
  if (value.enum) return value.enum.map((v) => JSON.stringify(v)).join(' | ');
  if ('const' in value) return JSON.stringify(value.const);
  if (value.anyOf || value.oneOf) return (value.anyOf ?? value.oneOf).map(type).join(' | ');
  if (value.allOf) return value.allOf.map((v) => `(${type(v)})`).join(' & ');
  if (Array.isArray(value.type))
    return value.type.map((t) => type({ ...value, type: t })).join(' | ');
  switch (value.type) {
    case 'null':
      return 'null';
    case 'boolean':
      return 'boolean';
    case 'string':
      return 'string';
    case 'integer':
    case 'number':
      return 'number';
    case 'array':
      return `Array<${type(value.items)}>`;
    case 'object': {
      const fields = Object.entries(value.properties ?? {}).map(
        ([key, v]) =>
          `${JSON.stringify(key)}${value.required?.includes(key) ? '' : '?'}: ${type(v)};`,
      );
      if (value.additionalProperties && typeof value.additionalProperties === 'object')
        fields.push(`[key: string]: ${type(value.additionalProperties)};`);
      if (!fields.length) return 'Record<string, never>';
      return `{ ${fields.join(' ')} }`;
    }
    default:
      throw new Error(`Unsupported IPC schema: ${JSON.stringify(value)}`);
  }
}
const text =
  '// Generated from Rust IPC DTOs. Run npm run ipc:generate; do not edit.\n' +
  Object.entries(schema.definitions)
    .map(([name, value]) => `export type ${name} = ${type(value)};`)
    .join('\n');
const target = path.join(root, 'src/desktop/bridge/generated.ts');
const config = await prettier.resolveConfig(target);
const formatted = await prettier.format(text, { ...config, parser: 'typescript' });
if (check) {
  if ((await fs.readFile(target, 'utf8')) !== formatted)
    throw new Error('IPC types drifted; run npm run ipc:generate');
  console.log('IPC TypeScript contract matches the Rust schema.');
} else {
  await fs.mkdir(path.dirname(target), { recursive: true });
  await fs.writeFile(target, formatted);
}
