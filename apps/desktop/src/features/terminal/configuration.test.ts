import { expect, it } from 'vitest';
import { configParent, configUsage, LOCAL_TARGET } from './configuration';
import { terminalFixture } from '../../preview/terminal';
import type { TerminalConfigView } from '../../shared/types';
const ssh: TerminalConfigView = {
  id: 'ssh',
  kind: 'ssh',
  data: {
    name: 'host',
    address: { kind: 'host', value: 'example.test' },
    port: 22,
    username: 'demo',
    auth: 'password',
    passwordSaved: false,
  },
};
const command: TerminalConfigView = {
  id: 'cmd',
  kind: 'command',
  data: { name: 'prepare', command: 'echo ready', sshTarget: 'ssh' },
};
const terminal: TerminalConfigView = {
  id: 'session',
  kind: 'terminal',
  data: { name: 'shell', dependencies: [{ commandId: 'cmd', reuse: 'shared' }] },
};
it('groups a local session and remote command under their execution targets', () => {
  expect(configParent(terminal)).toBe(LOCAL_TARGET);
  expect(configParent(command)).toBe('ssh');
});
it('locks saved references, including a command referenced from a different execution target', () => {
  const workspace = { ...terminalFixture(), configs: [ssh, command, terminal] };
  expect(configUsage('ssh', workspace).referrers.map((c) => c.id)).toEqual(['cmd']);
  expect(configUsage('cmd', workspace).locked).toBe(true);
  expect(configUsage('session', workspace).locked).toBe(false);
});
it('locks running, stopping, and transitive starting plans, then unlocks after release', () => {
  const workspace = { ...terminalFixture(), configs: [ssh, command, terminal] };
  const pending = [{ target: { kind: 'terminal' as const, id: 'session' } }];
  expect(configUsage('session', workspace, pending).active).toBe(true);
  expect(configUsage('cmd', workspace, pending).active).toBe(true);
  expect(configUsage('ssh', workspace, pending).active).toBe(true);
  workspace.runs = [
    {
      target: { kind: 'terminal', id: 'run' },
      configId: 'session',
      name: 'shell',
      phase: 'stopping',
      readyMarkerReceived: false,
    },
  ];
  expect(configUsage('session', workspace).locked).toBe(true);
  workspace.runs = [];
  expect(configUsage('session', workspace).locked).toBe(false);
});
