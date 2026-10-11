export type {
  RemoteProfile,
  RemoteDirectory,
  RemoteEntry,
  RemoteTransfer,
  RemoteRequest,
  RemoteReply,
  TerminalFrame,
} from '../bridge/generated';
import type {
  RemoteSnapshot as SnapshotWire,
  RemoteWorkspace as WorkspaceWire,
  RemoteDirectory,
} from '../bridge/generated';
export type RemoteWorkspace = Omit<WorkspaceWire, 'directories'> & {
  directories: RemoteDirectory[];
};
export type RemoteSnapshot = Omit<SnapshotWire, 'workspaces'> & { workspaces: RemoteWorkspace[] };
/** Keep directory patches that arrive before the initial connect reply. */
export function coalesceRemote(snapshot: SnapshotWire, previous: SnapshotWire): SnapshotWire {
  return {
    ...snapshot,
    workspaces: snapshot.workspaces.map((w) => ({
      ...w,
      directories:
        w.directories ??
        previous.workspaces.find((old) => old.id === w.id && old.filesId === w.filesId)
          ?.directories,
    })),
  };
}
export function mergeRemote(snapshot: SnapshotWire, previous: RemoteSnapshot): RemoteSnapshot {
  return {
    ...snapshot,
    workspaces: snapshot.workspaces.map((w) => ({
      ...w,
      directories:
        w.directories ??
        previous.workspaces.find((old) => old.id === w.id && old.filesId === w.filesId)
          ?.directories ??
        [],
    })),
  };
}
export const emptyRemote = (): RemoteSnapshot => ({
  revision: '0',
  profiles: [],
  workspaces: [],
  challenges: [],
  transfers: [],
  downloadDirectory: null,
});
export interface FileDrop {
  type: 'over' | 'drop' | 'leave';
  paths: string[];
  x: number;
  y: number;
}

export const fileTarget = (workspace: RemoteWorkspace) => ({
  workspace: workspace.id,
  files: workspace.filesId,
  generation: workspace.generation,
});
