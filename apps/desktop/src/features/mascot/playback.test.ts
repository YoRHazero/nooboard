import { expect, it, vi } from 'vitest';
import { Playback } from './playback';

it('coalesces concurrent performances while ignoring business receipts', () => {
  const play = vi.fn();
  const changed = vi.fn();
  const playback = new Playback(play, changed);
  playback.event({ type: 'sent', sequence: '1' });
  playback.event({ type: 'copied', sequence: '2' });
  playback.event({ type: 'received', sequence: '3' });
  playback.event({ type: 'applied', sequence: '1' });
  expect(play).toHaveBeenCalledTimes(1);
  expect(changed).toHaveBeenLastCalledWith({ active: 'send', activityId: '1' });
  playback.finish(1);
  expect(play).toHaveBeenLastCalledWith('idle', null);
  expect(play).toHaveBeenCalledTimes(2);
});

it('ignores stale animation completion after pause', () => {
  const play = vi.fn();
  const changed = vi.fn();
  const playback = new Playback(play, changed);
  playback.event({ type: 'sent', sequence: '1' });
  playback.setMode('paused');
  playback.finish(1);
  expect(play).toHaveBeenLastCalledWith('paused', null);
  expect(changed).toHaveBeenLastCalledWith({ active: null, activityId: null });
});
