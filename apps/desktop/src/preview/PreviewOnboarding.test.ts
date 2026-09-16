import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { PreviewClient } from './PreviewClient';
import { nearbySample } from './seed';
let client: PreviewClient;
beforeEach(() => {
  vi.useFakeTimers();
  client = new PreviewClient();
});
afterEach(() => {
  client.dispose();
  vi.useRealTimers();
});
it('discovery does not pair, code expiry is fixed across retries, and completion needs a correct code', async () => {
  await client.discover();
  expect(client.getSnapshot().onboarding?.nearby).toHaveLength(1);
  expect(client.getSnapshot().peers.some((p) => p.noobId === nearbySample.noobId)).toBe(false);
  await client.beginPairing('192.168.1.52:24817', nearbySample.noobId);
  const session = client.getSnapshot().onboarding!.session!;
  await client.submitPairingCode(session.id, '00000000');
  expect(client.getSnapshot().onboarding?.session).toMatchObject({
    stage: 'EnteringCode',
    attemptsLeft: 2,
    expiresAt: session.expiresAt,
  });
  await client.submitPairingCode(session.id, '48217396');
  expect(client.getSnapshot().onboarding?.session?.stage).toBe('Completed');
  expect(
    client.getSnapshot().peers.find((p) => p.noobId === nearbySample.noobId)?.settings.autoSend,
  ).toBe(false);
  await vi.advanceTimersByTimeAsync(120000);
  expect(client.getSnapshot().onboarding?.session?.stage).toBe('Completed');
});
it('expires an unused code and resetting cancels callbacks from the old session', async () => {
  await client.beginPairing('192.168.1.52:24817');
  const id = client.getSnapshot().onboarding!.session!.id;
  await vi.advanceTimersByTimeAsync(120000);
  expect(client.getSnapshot().onboarding?.session?.stage).toBe('Failed');
  await expect(client.submitPairingCode(id, '48217396')).rejects.toThrow();
  await client.reset();
  await vi.advanceTimersByTimeAsync(120000);
  expect(client.getSnapshot().onboarding).toBeUndefined();
});
