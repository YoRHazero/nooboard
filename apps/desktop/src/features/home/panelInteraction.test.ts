import { expect, it, vi } from 'vitest';
import { PanelInteraction } from './panelInteraction';

it('immediately follows the hovered object without requesting keyboard focus', () => {
  const panel = new PanelInteraction(vi.fn());
  for (const target of ['board', 'bird', 'mailbox', 'board'] as const) {
    panel.move(target);
    expect(panel.getSnapshot()).toEqual({ target, mode: 'preview', focusHeading: false });
  }
});

it('keeps a preview inside its panel and dismisses immediately over empty space', () => {
  const changed = vi.fn();
  const panel = new PanelInteraction(changed);
  panel.move('board');
  panel.move('board');
  panel.move('panel');
  expect(changed).toHaveBeenCalledTimes(1);
  expect(panel.getSnapshot()?.target).toBe('board');
  panel.move(null);
  expect(panel.getSnapshot()).toBeNull();
});

it('pins on activation and remains visible regardless of pointer movement', () => {
  const panel = new PanelInteraction(vi.fn());
  panel.move('board');
  panel.activate('board');
  for (const over of [null, 'bird', 'mailbox', 'panel'] as const) panel.move(over);
  expect(panel.getSnapshot()).toEqual({ target: 'board', mode: 'pinned', focusHeading: true });
});

it('clicking any stage object releases a pin without pinning another panel in the same click', () => {
  const panel = new PanelInteraction(vi.fn());
  for (const target of ['board', 'bird', 'mailbox'] as const) {
    panel.activate('board');
    panel.activate(target);
    expect(panel.getSnapshot()).toBeNull();
    panel.move(target);
    expect(panel.getSnapshot()).toEqual({ target, mode: 'preview', focusHeading: false });
  }
});

it('clicking inside a preview pins it without moving focus off the control', () => {
  const panel = new PanelInteraction(vi.fn());
  panel.move('bird');
  panel.pin();
  panel.pin();
  panel.move(null);
  expect(panel.getSnapshot()).toEqual({ target: 'bird', mode: 'pinned', focusHeading: false });
});

it('outside dismissal and close restore immediate hover behavior', () => {
  const panel = new PanelInteraction(vi.fn());
  panel.activate('board');
  panel.dismiss();
  expect(panel.getSnapshot()).toBeNull();
  panel.move('board');
  expect(panel.getSnapshot()?.mode).toBe('preview');
  panel.move('bird');
  expect(panel.getSnapshot()?.target).toBe('bird');
});

it('hides hover previews on window blur but preserves a pinned panel', () => {
  const panel = new PanelInteraction(vi.fn());
  panel.move('board');
  panel.cancelPreview();
  expect(panel.getSnapshot()).toBeNull();
  panel.activate('bird');
  panel.cancelPreview();
  expect(panel.getSnapshot()?.mode).toBe('pinned');
});
