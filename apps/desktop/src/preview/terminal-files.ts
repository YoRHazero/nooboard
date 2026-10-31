import type { TerminalDirectoryEntry } from '../shared/contracts.generated';
import { fail } from '../shared/i18n/errors';
export function canonicalPreviewPath(path: string) {
  const parts: string[] = [];
  for (const part of path.split('/')) {
    if (part === '..') parts.pop();
    else if (part && part !== '.') parts.push(part);
  }
  return `/${parts.join('/')}`;
}
/** Deterministic in-memory files for preview interactions; no host filesystem access. */
export class PreviewTerminalFiles {
  private directories = new Map<string, TerminalDirectoryEntry[]>([
    [
      '/',
      [
        { name: 'home', kind: 'directory' },
        { name: 'tmp', kind: 'directory' },
      ],
    ],
    ['/tmp', []],
    ['/home', [{ name: 'demo', kind: 'directory' }]],
    [
      '/home/demo',
      [
        { name: 'projects', kind: 'directory' },
        { name: 'pictures', kind: 'directory' },
        { name: 'README.md', kind: 'file', size: '2048' },
        { name: 'notes.txt', kind: 'file', size: '256' },
      ],
    ],
    [
      '/home/demo/projects',
      [
        { name: 'nooboard', kind: 'directory' },
        { name: 'hello.txt', kind: 'file', size: '128' },
      ],
    ],
    [
      '/home/demo/projects/nooboard',
      [
        { name: 'src', kind: 'directory' },
        { name: 'package.json', kind: 'file', size: '1024' },
      ],
    ],
    ['/home/demo/projects/nooboard/src', [{ name: 'main.ts', kind: 'file', size: '512' }]],
    ['/home/demo/pictures', []],
  ]);
  browse(input = '/home/demo') {
    const path = canonicalPreviewPath(input);
    const entries = this.directories.get(path);
    if (!entries) throw fail('terminalDirectoryNotFound');
    return { path, entries: structuredClone(entries) };
  }
  private location(input: string) {
    const path = canonicalPreviewPath(input);
    const index = path.lastIndexOf('/');
    return { path, parent: path.slice(0, index) || '/', name: path.slice(index + 1) };
  }
  add(input: string, directory: boolean) {
    const { path, parent, name } = this.location(input);
    const entries = this.directories.get(parent);
    if (!entries || !name || entries.some((entry) => entry.name === name))
      throw fail('terminalFileExists');
    entries.push(directory ? { name, kind: 'directory' } : { name, kind: 'file', size: '8192' });
    if (directory) this.directories.set(path, []);
  }
  remove(input: string) {
    const { path, parent, name } = this.location(input);
    const entries = this.directories.get(parent);
    if (!entries?.some((entry) => entry.name === name)) throw fail('terminalDirectoryNotFound');
    this.directories.set(
      parent,
      entries.filter((entry) => entry.name !== name),
    );
    for (const key of this.directories.keys())
      if (key === path || key.startsWith(`${path}/`)) this.directories.delete(key);
  }
  rename(source: string, destination: string) {
    const from = this.location(source),
      to = this.location(destination);
    const entry = this.directories.get(from.parent)?.find((entry) => entry.name === from.name);
    if (!entry || !this.directories.has(to.parent)) throw fail('terminalDirectoryNotFound');
    if (this.directories.get(to.parent)!.some((entry) => entry.name === to.name))
      throw fail('terminalFileExists');
    this.directories.set(
      from.parent,
      this.directories.get(from.parent)!.filter((entry) => entry.name !== from.name),
    );
    this.directories.get(to.parent)!.push({ ...entry, name: to.name });
    const nested = [...this.directories].filter(
      ([path]) => path === from.path || path.startsWith(`${from.path}/`),
    );
    nested.forEach(([path]) => this.directories.delete(path));
    nested.forEach(([path, entries]) =>
      this.directories.set(to.path + path.slice(from.path.length), entries),
    );
  }
}
