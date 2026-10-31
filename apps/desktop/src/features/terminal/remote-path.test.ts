import { expect, it } from 'vitest';
import { completionParts, joinRemote, parentRemote, topLevelPaths } from './remote-path';
import { createTerminalStore } from './store';
it('completes only the typed parent, supporting absolute and relative directory names', () => {
  expect(completionParts('/home/demo/pr', '/elsewhere')).toEqual({
    parent: '/home/demo',
    prefix: 'pr',
  });
  expect(completionParts('projects/', '/home/demo')).toEqual({
    parent: '/home/demo/projects',
    prefix: '',
  });
  expect(completionParts('/', '/home/demo')).toEqual({ parent: '/', prefix: '' });
  expect(joinRemote('/', 'test')).toBe('/test');
  expect(parentRemote('/home/')).toBe('/');
});
it('avoids duplicate recursive downloads/deletions without confusing similarly named siblings', () => {
  expect(topLevelPaths(['/a', '/a/child', '/ab', '/a', '/ab/child'])).toEqual(['/a', '/ab']);
});
it('keeps each SSH browser location separate and retains open preference across selection', () => {
  const ui = createTerminalStore();
  ui.getState().toggleFiles();
  ui.getState().updateFileView('one', {
    path: '/one',
    expanded: ['/one/sub'],
    selected: ['/one/file'],
    selecting: true,
  });
  ui.getState().updateFileView('two', { path: '/two' });
  ui.getState().select({ kind: 'terminal', id: 'local' });
  expect(ui.getState().filesOpen).toBe(true);
  expect(ui.getState().fileViews.one.path).toBe('/one');
  expect(ui.getState().fileViews.two.selected).toEqual([]);
  ui.getState().forgetFileView('one');
  expect(ui.getState().fileViews.one).toBeUndefined();
  expect(ui.getState().fileViews.two.path).toBe('/two');
});
