import { describe, expect, it, vi } from 'vitest';
import { BackendSubscription } from '../../../app/backend-subscription';
import { PreviewBackend } from '../../../preview/backend';
import { useApplicationStore } from '../../../shared/store';
import { installUpdate } from './useInstallUpdateMutation';

describe('update installation', () => {
  it('has no indicator by default and clears it after a successful update', async () => {
    const backend = new PreviewBackend();
    const subscription = new BackendSubscription(backend);
    await subscription.start();
    expect(subscription.projection.getSnapshot().host.update).toBeNull();
    await subscription.dispose();

    const ready = new BackendSubscription(new PreviewBackend('ready'));
    await ready.start();
    const updateVersion = ready.projection.getSnapshot().host.update?.version;
    expect(updateVersion).toBeDefined();
    expect(updateVersion).not.toBe(ready.projection.getSnapshot().host.version);
    await installUpdate();
    expect(ready.projection.getSnapshot().host.version).toBe(updateVersion);
    expect(ready.projection.getSnapshot().host.update).toBeNull();
    await ready.dispose();
  });
  it('reattaches to the recovered Core session and keeps the update for retry on failure', async () => {
    const backend = new PreviewBackend('failed');
    const subscription = new BackendSubscription(backend);
    await subscription.start();
    const connect = vi.spyOn(backend, 'connect');
    await expect(installUpdate()).rejects.toMatchObject({ problem: { code: 'updateInstall' } });
    expect(connect).toHaveBeenCalledOnce();
    expect(useApplicationStore.getState().connection.phase).toBe('ready');
    expect(subscription.projection.getSnapshot().host.update?.installing).toBe(false);
    await subscription.dispose();
  });
});
