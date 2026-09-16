import { problem, fail } from '../i18n/errors';
import type { DesktopSnapshot, Onboarding, Peer, PairingSession } from '../api/contracts';
import { validAddress } from '../api/devices';
import { nearbySample, examplePeers } from './seed';

/** Simulates pairing only in the explicitly labelled browser preview. */
export class PreviewOnboarding {
  private candidate: Omit<Peer, 'online' | 'accepting'> | undefined;
  private timer: ReturnType<typeof setTimeout> | undefined;
  constructor(
    private snapshot: () => DesktopSnapshot,
    private publish: (value: Onboarding) => void,
    private savePeer: (peer: Omit<Peer, 'online' | 'accepting'>) => Promise<void>,
  ) {}
  private state(): Onboarding {
    return (
      this.snapshot().onboarding ?? {
        nearby: [],
        error: null,
        session: null,
      }
    );
  }
  private session(id: string) {
    const session = this.state().session;
    if (!session || session.id !== id) throw fail('pairingEnded');
    return session;
  }
  async discover() {
    this.publish({
      ...this.state(),
      nearby: [
        {
          key: 'preview-nearby',
          noobId: nearbySample.noobId,
          deviceName: nearbySample.deviceName,
          addresses: ['192.168.1.52:24817'],
          syncPort: 24816,
        },
      ],
    });
  }
  async begin(address: string, expected?: string) {
    if (!validAddress(address)) throw fail('invalidAddress');
    const old = this.state().session;
    if (old && !['Completed', 'Failed'].includes(old.stage)) throw fail('pairingBusy');
    if (expected === this.snapshot().localDevice.noobId) throw fail('selfPairing');
    const identity = [nearbySample, ...examplePeers].find(
      (p) => p.noobId === (expected ?? nearbySample.noobId),
    );
    if (!identity) throw fail('pairingIdentityChanged');
    this.dispose();
    this.candidate = {
      ...identity,
      settings: { address: address.replace(/:\d+$/, ':24816'), autoSend: false },
    };
    const session: PairingSession = {
      id: crypto.randomUUID(),
      incoming: false,
      deviceName: identity.deviceName,
      noobId: identity.noobId,
      stage: 'EnteringCode',
      code: null,
      expiresAt: Date.now() + 120000,
      attemptsLeft: 3,
      error: null,
    };
    this.publish({ ...this.state(), session });
    this.timer = setTimeout(() => {
      if (this.state().session?.id === session.id)
        this.publish({
          ...this.state(),
          session: {
            ...this.session(session.id),
            stage: 'Failed',
            code: null,
            error: problem('pairingTimeout'),
          },
        });
    }, 120000);
  }
  async accept(id: string) {
    this.session(id);
    throw fail('previewPairingDirection');
  }
  async submit(id: string, code: string) {
    const session = this.session(id);
    if (session.stage !== 'EnteringCode' || Date.now() >= session.expiresAt)
      throw fail('pairingTimeout');
    if (!/^\d{8}$/.test(code)) throw fail('pairingCodeFormat');
    if (code !== '48217396') {
      const attemptsLeft = session.attemptsLeft - 1;
      this.publish({
        ...this.state(),
        session: {
          ...session,
          attemptsLeft,
          stage: attemptsLeft ? 'EnteringCode' : 'Failed',
          error: attemptsLeft
            ? problem('pairingCodeRemaining', { count: attemptsLeft })
            : problem('pairingAttempts'),
        },
      });
      if (!attemptsLeft) this.dispose();
      return;
    }
    if (!this.candidate) throw fail('pairingEnded');
    await this.savePeer(this.candidate);
    this.dispose();
    this.publish({ ...this.state(), session: { ...session, stage: 'Completed', error: null } });
  }
  async dismiss(id: string) {
    this.session(id);
    this.dispose();
    this.publish({ ...this.state(), session: null });
  }
  dispose() {
    this.candidate = undefined;
    clearTimeout(this.timer);
    this.timer = undefined;
  }
}
