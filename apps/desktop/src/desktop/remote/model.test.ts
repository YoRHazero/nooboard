import { expect, it } from 'vitest';
import { emptyRemote, mergeRemote, coalesceRemote, type RemoteWorkspace } from './model';
const workspace: RemoteWorkspace = {
  id: 'workspace',
  deviceId: 'device',
  profileId: 'profile',
  username: 'tester',
  port: 22,
  terminalId: 'terminal',
  filesId: 'files',
  terminal: { state: 'Ready' },
  files: { state: 'Ready' },
  generation: '1',
  root: '/home',
  directories: [{ path: '/home', state: 'Complete', entries: [] }],
};
it('retains directory identity on progress-only frames and clears it for a replaced file connection', () => {
  const previous = { ...emptyRemote(), workspaces: [workspace] };
  const { directories, ...wire } = workspace;
  const next = mergeRemote({ ...previous, revision: '2', workspaces: [wire] }, previous);
  expect(next.workspaces[0].directories).toBe(directories);
  const replaced = mergeRemote(
    { ...previous, workspaces: [{ ...wire, filesId: 'new-files' }] },
    next,
  );
  expect(replaced.workspaces[0].directories).toEqual([]);
  expect(
    mergeRemote({ ...previous, workspaces: [{ ...wire, directories: [] }] }, next).workspaces[0]
      .directories,
  ).toEqual([]);
});

it('coalesces early partial frames without discarding a directory update or the initial cache', () => {
  const { directories, ...wire } = workspace;
  const initial = { ...emptyRemote(), workspaces: [workspace] };
  const updated = [{ ...directories[0], path: '/updated' }];
  const changed = { ...initial, workspaces: [{ ...wire, directories: updated }] };
  const progress = { ...initial, workspaces: [wire] };
  const pending = coalesceRemote(progress, changed);
  expect(mergeRemote(pending, initial).workspaces[0].directories).toBe(updated);
  expect(mergeRemote(coalesceRemote(progress, progress), initial).workspaces[0].directories).toBe(
    directories,
  );
});
