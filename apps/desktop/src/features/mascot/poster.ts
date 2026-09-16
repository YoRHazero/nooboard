import type { TargetLayout } from './targets';

// Bounds in the 1200 × 700 fallback render, fitted with the image's contain sizing.
const bounds: TargetLayout = {
  board: { left: 5, top: 51, width: 12, height: 30 },
  bird: { left: 10, top: 18, width: 32, height: 61 },
  mailbox: { left: 67, top: 39, width: 20, height: 42 },
};

export function posterTargets(width: number, height: number): TargetLayout {
  const scale = Math.min(width / 1200, height / 700);
  const imageWidth = 1200 * scale;
  const imageHeight = 700 * scale;
  return Object.fromEntries(
    Object.entries(bounds).map(([key, rect]) => [
      key,
      {
        left: (((width - imageWidth) / 2 + (rect.left / 100) * imageWidth) / width) * 100,
        top: (((height - imageHeight) / 2 + (rect.top / 100) * imageHeight) / height) * 100,
        width: (rect.width * imageWidth) / width,
        height: (rect.height * imageHeight) / height,
      },
    ]),
  ) as TargetLayout;
}
