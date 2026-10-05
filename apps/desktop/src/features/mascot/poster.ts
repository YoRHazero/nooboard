import type { TargetLayout } from './targets';

// Bounds in the 1200 × 700 fallback render, fitted with the image's contain sizing.
const bounds: TargetLayout = {
  board: { left: 3.8417, top: 49.7485, width: 14.832, height: 28.2832 },
  bird: { left: 6.6146, top: 21.1252, width: 39.9508, height: 57.3499 },
  mailbox: { left: 64.5655, top: 37.915, width: 25.0582, height: 40.2942 },
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
