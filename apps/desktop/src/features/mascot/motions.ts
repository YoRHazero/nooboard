export const motions = [
  'idle',
  'curious',
  'hop',
  'capture',
  'send',
  'receive_text',
  'receive_file',
  'success',
  'error',
  'paused',
  'offline',
  'resume',
] as const;
export type Clip = (typeof motions)[number];
export type Gesture = 'curious' | 'hop';

export const carriesProps = (clip: Clip | undefined) =>
  clip === 'send' || clip === 'receive_text' || clip === 'receive_file';
