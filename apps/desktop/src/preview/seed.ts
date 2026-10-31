import { decode } from '../shared/decode-snapshot';
import { snapshotFixture, hostFixture } from './scenarios/fixtures';
export { examplePeer, examplePeers, nearbySample } from './scenarios/fixtures';
export function createSeed() {
  return decode(snapshotFixture(), hostFixture());
}
