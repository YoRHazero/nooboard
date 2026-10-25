import { expect, it, vi } from 'vitest';
const invoke = vi.hoisted(() => vi.fn());
vi.mock('@tauri-apps/api/core', () => ({
  invoke,
  Channel: class {
    onmessage = () => {};
  },
}));
vi.mock('@tauri-apps/plugin-os', () => ({ locale: vi.fn() }));
import { TauriBridge } from './tauri';
import { snapshotFixture, hostFixture } from '../../preview/scenarios/fixtures';
it('disconnects exactly the subscription it opened, including an aborted late handshake', async () => {
  let resolve!: (value: unknown) => void;
  invoke
    .mockImplementationOnce(
      () =>
        new Promise((done) => {
          resolve = done;
        }),
    )
    .mockResolvedValue(undefined);
  const abort = new AbortController();
  const receive = vi.fn();
  const opening = new TauriBridge().connect(receive, abort.signal);
  const args = invoke.mock.calls[0][1];
  abort.abort();
  resolve({ snapshot: snapshotFixture(), host: hostFixture(), diagnostic: false });
  const session = await opening;
  await session.close();
  expect(invoke).toHaveBeenLastCalledWith('disconnect', { subscription: args.subscription });
  expect(invoke).toHaveBeenCalledTimes(2);
  args.onFrame.onmessage({ type: 'stopped', data: { code: 'stopped' } });
  expect(receive).not.toHaveBeenCalled();
});
