import { decode } from '../client/snapshot/decode';
import { snapshotFixture, hostFixture } from './scenarios/fixtures';
export { examplePeer, examplePeers, nearbySample } from './scenarios/fixtures';
export function createSeed() {
  return decode(snapshotFixture(), hostFixture(), { theme: 'light', reducedMotion: false });
}
