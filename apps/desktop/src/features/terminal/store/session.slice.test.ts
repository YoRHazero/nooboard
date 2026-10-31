import { expect, it } from 'vitest';
import { terminalFixture } from '../../../preview/terminal';
import { createTerminalStore } from './index';
import type { TerminalRunView } from '../../../shared/types';
const run = (
  id: string,
  kind: 'terminal' | 'command' | 'tunnel' = 'terminal',
): TerminalRunView => ({
  target: { kind, id },
  name: id,
  phase: 'running',
  readyMarkerReceived: false,
});
it('minimizes every instance type without stopping, then restores only on explicit selection', () => {
  const workspace = {
    ...terminalFixture(),
    runs: [run('shell'), run('command', 'command'), run('tunnel', 'tunnel')],
  };
  const ui = createTerminalStore();
  ui.getState().reconcile(workspace);
  for (const item of workspace.runs) ui.getState().collapse(item.target, workspace);
  expect(ui.getState().selected).toBeNull();
  expect(ui.getState().collapsed).toHaveLength(3);
  ui.getState().select(workspace.runs[0].target);
  expect(ui.getState().selected?.id).toBe('shell');
  expect(ui.getState().collapsed).toHaveLength(2);
});
it('keeps ready commands visible; ending an unselected instance never steals focus', () => {
  const workspace = {
    ...terminalFixture(),
    runs: [run('shell'), run('command', 'command'), run('other')],
  };
  const ui = createTerminalStore();
  ui.getState().reconcile(workspace);
  ui.getState().select(workspace.runs[0].target);
  workspace.runs[1].readyMarkerReceived = true;
  ui.getState().reconcile(workspace);
  expect(ui.getState().collapsed).toEqual([]);
  ui.getState().reconcile(workspace, workspace.runs[2].target);
  expect(ui.getState().selected?.id).toBe('shell');
});
it('ending the selected instance switches only to an unminimized instance, otherwise clears', () => {
  const workspace = { ...terminalFixture(), runs: [run('a'), run('b'), run('hidden')] };
  const ui = createTerminalStore();
  ui.getState().reconcile(workspace);
  ui.getState().collapse(workspace.runs[2].target, workspace);
  ui.getState().select(workspace.runs[0].target);
  ui.getState().reconcile(workspace, workspace.runs[0].target);
  expect(ui.getState().selected?.id).toBe('b');
  workspace.runs = workspace.runs.slice(1);
  ui.getState().reconcile(workspace, workspace.runs[0].target);
  expect(ui.getState().selected).toBeNull();
  expect(ui.getState().collapsed).toEqual(['terminal:hidden']);
});
it('keeps a just-opened selection until its first snapshot, then clears on authoritative removal', () => {
  const ui = createTerminalStore();
  const workspace = terminalFixture();
  const target = { kind: 'terminal' as const, id: 'new-run' };
  ui.getState().select(target);
  ui.getState().reconcile(workspace);
  expect(ui.getState().selected).toEqual(target);
  workspace.runs = [{ target, name: 'shell', phase: 'running', readyMarkerReceived: false }];
  ui.getState().reconcile(workspace);
  workspace.runs = [];
  ui.getState().reconcile(workspace);
  expect(ui.getState().selected).toBeNull();
});
