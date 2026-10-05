import { expect, it, vi } from 'vitest';
import { Playback } from './playback';

function setup() {
  const play = vi.fn();
  const changed = vi.fn();
  const playback = new Playback(play, changed);
  const finish = () => playback.finish(play.mock.lastCall![1]);
  return { play, changed, playback, finish };
}

it('groups concurrent activities without queuing or celebrating an obsolete receipt', () => {
  const { play, changed, playback, finish } = setup();
  playback.event({ type: 'sent', sequence: '1' });
  playback.event({ type: 'copied', sequence: '2' });
  playback.event({ type: 'received', sequence: '3' });
  playback.event({ type: 'applied', sequence: '1' });
  expect(play).toHaveBeenCalledTimes(1);
  expect(changed).toHaveBeenLastCalledWith({ active: 'send', activityId: '1' });
  finish();
  expect(play).toHaveBeenLastCalledWith('idle', null);
  expect(play).toHaveBeenCalledTimes(2);
});

it.each(['applied', 'rejected'] as const)('waits until delivery ends before showing %s', (type) => {
  const { play, changed, playback, finish } = setup();
  playback.event({ type: 'sent', sequence: '1' });
  playback.event({ type, sequence: '1' });
  expect(play).toHaveBeenCalledTimes(1);
  finish();
  expect(changed).toHaveBeenLastCalledWith({
    active: type === 'applied' ? 'success' : 'error',
    activityId: '1',
  });
  finish();
  expect(play).toHaveBeenLastCalledWith('idle', null);
});

it('correlates a delayed receipt after the bird has returned to idle', () => {
  const { play, playback, finish } = setup();
  playback.event({ type: 'sent', sequence: '1' });
  finish();
  playback.event({ type: 'applied', sequence: 'unrelated' });
  expect(play).toHaveBeenLastCalledWith('idle', null);
  playback.event({ type: 'applied', sequence: '1' });
  expect(play).toHaveBeenLastCalledWith('success', expect.any(Number));
});

it('waits for file availability and correlates outgoing file receipts by task, not activity ID', () => {
  const { play, playback, finish } = setup();
  playback.event({
    type: 'received',
    sequence: '1',
    contentTask: 'download',
    contentNode: 'started',
    contentStage: 'Receiving',
    contentKind: 'Image',
  });
  expect(play).not.toHaveBeenCalled();
  playback.event({
    type: 'received',
    sequence: '2',
    contentTask: 'download',
    contentNode: 'finished',
    contentStage: 'Saved',
    contentKind: 'Image',
  });
  expect(play).toHaveBeenLastCalledWith('receive_file', expect.any(Number));
  finish();
  playback.event({ type: 'sent', sequence: '3', contentTask: 'upload', contentNode: 'started' });
  expect(play).toHaveBeenLastCalledWith('idle', null);
  playback.event({
    type: 'applied',
    sequence: '4',
    contentTask: 'upload',
    contentNode: 'finished',
  });
  expect(play).toHaveBeenLastCalledWith('success', expect.any(Number));
});

it('settles carried props before sleeping, suppresses activity during pause, and wakes once', () => {
  const { play, changed, playback, finish } = setup();
  playback.setMode('idle');
  playback.event({ type: 'sent', sequence: '1' });
  playback.setMode('paused');
  expect(play).toHaveBeenLastCalledWith('send', expect.any(Number));
  playback.event({ type: 'applied', sequence: '1' });
  playback.event({ type: 'received', sequence: '2' });
  finish();
  expect(play).toHaveBeenLastCalledWith('paused', null);
  expect(changed).toHaveBeenLastCalledWith({ active: null, activityId: null });
  playback.setMode('idle');
  expect(play).toHaveBeenLastCalledWith('resume', expect.any(Number));
  finish();
  expect(play).toHaveBeenLastCalledWith('idle', null);
});

it('lets business actions interrupt gestures and ignores stale completions after hiding or reset', () => {
  const { play, playback, finish } = setup();
  playback.gesture('curious');
  const gestureSerial = play.mock.lastCall![1];
  playback.event({ type: 'received', sequence: '1' });
  playback.finish(gestureSerial);
  expect(play).toHaveBeenLastCalledWith('receive_text', expect.any(Number));
  playback.gesture('hop');
  const carryingSerial = play.mock.lastCall![1];
  playback.reset();
  playback.finish(carryingSerial);
  expect(play).toHaveBeenLastCalledWith('idle', null);
  playback.setMode('paused', false);
  playback.setMode('offline', false);
  expect(play).toHaveBeenLastCalledWith('offline', null);
  playback.event({ type: 'copied', sequence: '2' });
  expect(play).toHaveBeenLastCalledWith('offline', null);
  playback.setMode('idle');
  finish();
  playback.gesture('hop');
  expect(play).toHaveBeenLastCalledWith('hop', expect.any(Number));
});

it('does not treat cancellation as a failure', () => {
  const { play, playback } = setup();
  playback.event({
    type: 'rejected',
    sequence: '1',
    contentTask: 'task',
    contentStage: 'Cancelled',
  });
  expect(play).not.toHaveBeenCalled();
});

it('discards old negative receipts too and only shows a file failure tied to the current transfer', () => {
  const { play, playback, finish } = setup();
  playback.event({ type: 'sent', sequence: '1' });
  playback.event({ type: 'copied', sequence: '2' });
  finish();
  playback.event({ type: 'rejected', sequence: '1' });
  expect(play).toHaveBeenLastCalledWith('idle', null);
  playback.event({
    type: 'received',
    sequence: '3',
    contentTask: 'download',
    contentNode: 'started',
  });
  playback.event({
    type: 'rejected',
    sequence: '4',
    contentTask: 'download',
    contentNode: 'finished',
    contentStage: 'Failed',
  });
  expect(play).toHaveBeenLastCalledWith('error', expect.any(Number));
});

it('lets the first real event interrupt waking after reconnecting', () => {
  const { play, playback } = setup();
  playback.setMode('offline');
  playback.setMode('idle');
  expect(play).toHaveBeenLastCalledWith('resume', expect.any(Number));
  playback.event({ type: 'received', sequence: '1' });
  expect(play).toHaveBeenLastCalledWith('receive_text', expect.any(Number));
});
