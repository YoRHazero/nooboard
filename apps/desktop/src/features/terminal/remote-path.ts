export const joinRemote = (directory: string, name: string) =>
  `${directory.replace(/\/+$/, '')}/${name}`;
export const parentRemote = (path: string) =>
  path.replace(/\/+$/, '').replace(/\/[^/]*$/, '') || '/';
export const basenameRemote = (path: string) => path.replace(/\/+$/, '').split('/').pop() ?? '';
export function completionParts(input: string, root: string) {
  const path = input.startsWith('/') ? input : joinRemote(root, input);
  const slash = path.lastIndexOf('/');
  return { parent: path.slice(0, slash) || '/', prefix: path.slice(slash + 1) };
}
/** Selecting a directory already includes its descendants in recursive operations. */
export function topLevelPaths(paths: readonly string[]) {
  const unique = [...new Set(paths)];
  return unique.filter(
    (path) =>
      !unique.some((other) => other !== path && path.startsWith(`${other.replace(/\/+$/, '')}/`)),
  );
}
