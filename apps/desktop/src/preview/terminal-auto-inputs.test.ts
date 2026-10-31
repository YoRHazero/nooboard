import { expect, it } from 'vitest';
import { PreviewTerminal } from './terminal';
import type { TerminalAutoInputEdit, TerminalWorkspaceView } from '../shared/contracts.generated';

it('keeps saved rule order and exposes only keys after saving, copying and deleting values', async () => {
  let current: TerminalWorkspaceView | undefined;
  const preview = new PreviewTerminal(
    (workspace) => {
      current = workspace;
    },
    () => {},
  );
  const save = (autoInputs: TerminalAutoInputEdit[]) =>
    preview.request({
      type: 'saveConfig',
      data: {
        id: 'auto-input-test',
        config: {
          kind: 'command',
          data: { name: 'test', command: 'demo', autoInputs },
        },
      },
    });
  const keys = () => {
    const config = current!.configs.find((item) => item.id === 'auto-input-test');
    if (config?.kind !== 'command') throw new Error('Missing command');
    return config.data.autoInputs;
  };
  try {
    await save([
      { prefix: 'Zulu:', value: 'first-secret' },
      { prefix: 'Alpha:', value: '' },
    ]);
    expect(keys()).toEqual(['Zulu:', 'Alpha:']);
    await save([
      { prefix: 'New:', value: 'new-secret' },
      { prefix: 'Alpha:', value: null },
      { prefix: 'Zulu:', value: 'replacement-secret' },
    ]);
    expect(keys()).toEqual(['Zulu:', 'Alpha:', 'New:']);
    expect(JSON.stringify(current)).not.toContain('secret');
    await preview.request({
      type: 'copyConfig',
      data: { target: { kind: 'command', id: 'auto-input-test' } },
    });
    const copy = current!.configs.at(-1);
    expect(copy?.kind === 'command' && copy.data.autoInputs).toEqual(keys());
    await save([{ prefix: 'Alpha:', value: null }]);
    expect(keys()).toEqual(['Alpha:']);
    await expect(save([{ prefix: 'Missing:', value: null }])).rejects.toBeDefined();
    await expect(
      save([
        { prefix: 'Repeat:', value: '' },
        { prefix: 'Repeat:', value: '' },
      ]),
    ).rejects.toBeDefined();
  } finally {
    preview.dispose();
  }
});
