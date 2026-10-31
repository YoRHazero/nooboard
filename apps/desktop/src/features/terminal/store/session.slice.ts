import type { StateCreator } from 'zustand';
import type { TerminalTarget, TerminalEnded, TerminalWorkspaceView } from '../../../shared/types';
export interface TerminalSessionState {
  selected: TerminalTarget | null;
  collapsed: readonly string[];
  pending: readonly {
    subscription: string;
    target: TerminalTarget | null;
    name?: string;
    attaching?: boolean;
  }[];
  notice: TerminalEnded | 'lagged' | null;
}

export interface SessionSlice extends TerminalSessionState {
  selectionIntent: number;
  select(target: TerminalTarget | null): void;
  clearNotice(): void;
  observedRuns: readonly string[];
  collapse(target: TerminalTarget, workspace: TerminalWorkspaceView): void;
  reconcile(
    workspace: Pick<TerminalWorkspaceView, 'runs' | 'dependencies'>,
    ended?: TerminalTarget,
  ): void;
  resetSession(): void;
}
export const createSessionSlice: StateCreator<SessionSlice, [], [], SessionSlice> = (set, get) => ({
  selected: null,
  collapsed: [],
  pending: [],
  notice: null,
  selectionIntent: 0,
  observedRuns: [],
  select: (target) =>
    set((state) => ({
      selectionIntent: state.selectionIntent + 1,
      selected: target,
      collapsed: target
        ? state.collapsed.filter((key) => key !== `${target.kind}:${target.id}`)
        : state.collapsed,
    })),
  clearNotice: () => set({ notice: null }),
  collapse: (target, workspace) => {
    const key = `${target.kind}:${target.id}`;
    if (!workspace.runs.some((run) => `${run.target.kind}:${run.target.id}` === key)) return;
    set((state) => ({
      collapsed: [...new Set([...state.collapsed, key])],
      selectionIntent: state.selectionIntent + 1,
    }));
    get().reconcile(workspace);
  },
  reconcile: ({ runs, dependencies }, ended) => {
    const key = (target: TerminalTarget) => `${target.kind}:${target.id}`;
    const state = get();
    const currentRuns = runs.filter((run) => !ended || key(run.target) !== key(ended));
    const current = new Set(currentRuns.map((run) => key(run.target)));
    const collapsed = new Set(state.collapsed.filter((value) => current.has(value)));
    let selected = state.selected;
    if (
      selected &&
      !current.has(key(selected)) &&
      (state.observedRuns.includes(key(selected)) || (ended && key(selected) === key(ended)))
    )
      selected = null;
    if (selected && collapsed.has(key(selected))) {
      const owner = dependencies.find((dep) => dep.runId === selected!.id)?.owner;
      selected = owner && current.has(key(owner)) && !collapsed.has(key(owner)) ? owner : null;
    }
    if (!selected)
      selected =
        [...currentRuns].reverse().find((run) => !collapsed.has(key(run.target)))?.target ?? null;
    const same = (values: readonly string[], other: Set<string>) =>
      values.length === other.size && values.every((value) => other.has(value));
    if (
      (selected ? key(selected) : null) !== (state.selected ? key(state.selected) : null) ||
      !same(state.collapsed, collapsed) ||
      !same(state.observedRuns, current)
    ) {
      set({
        selected,
        collapsed: [...collapsed],
        observedRuns: [...current],
      });
    }
  },
  resetSession: () =>
    set({
      selected: null,
      collapsed: [],
      pending: [],
      notice: null,
      observedRuns: [],
      selectionIntent: 0,
    }),
});
