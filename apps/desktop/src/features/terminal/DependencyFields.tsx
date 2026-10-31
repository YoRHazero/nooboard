import { ArrowUp, Plus, X } from 'lucide-react';
import type { Dispatch, SetStateAction } from 'react';
import type { TerminalDependency } from '../../shared/types';
import { useSnapshot } from '../../shared/queries/useBackendSnapshotQuery';
import { useI18n } from '../../shared/i18n/react';
import { Button, IconButton } from '../../shared/ui/controls';
import { Select, SelectItem } from '../../shared/ui/Select';
export function DependencyFields({
  dependencies,
  setDependencies,
}: {
  dependencies: TerminalDependency[];
  setDependencies: Dispatch<SetStateAction<TerminalDependency[]>>;
}) {
  const { t } = useI18n();
  const commands = useSnapshot((s) => s.terminal.configs.filter((c) => c.kind === 'command'));
  return (
    <section className="terminal-prerequisites">
      <h3>{t('terminal:dependencies')}</h3>
      <p className="muted small">{t('terminal:dependencyHint')}</p>
      {dependencies.map((dep, index) => (
        <div className="terminal-dependency-edit" key={index}>
          <span>{index + 1}.</span>
          <Select
            aria-label={t('terminal:command')}
            value={dep.commandId}
            onValueChange={(e) =>
              setDependencies((items) =>
                items.map((item, i) => (i === index ? { ...item, commandId: e } : item)),
              )
            }
          >
            {commands.map((c) => (
              <SelectItem key={c.id} value={c.id}>
                {c.data.name}
              </SelectItem>
            ))}
          </Select>
          <Select
            aria-label={t('terminal:reuse')}
            value={dep.reuse}
            onValueChange={(e) =>
              setDependencies((items) =>
                items.map((item, i) =>
                  i === index ? { ...item, reuse: e as TerminalDependency['reuse'] } : item,
                ),
              )
            }
          >
            <SelectItem value="shared">{t('terminal:shared')}</SelectItem>
            <SelectItem value="alwaysNew">{t('terminal:alwaysNew')}</SelectItem>
          </Select>
          <IconButton
            label={t('terminal:moveUp')}
            disabled={!index}
            onClick={() =>
              setDependencies((items) => {
                const next = [...items];
                [next[index - 1], next[index]] = [next[index], next[index - 1]];
                return next;
              })
            }
          >
            <ArrowUp size={14} />
          </IconButton>
          <IconButton
            label={t('terminal:remove')}
            onClick={() => setDependencies((items) => items.filter((_, i) => i !== index))}
          >
            <X size={14} />
          </IconButton>
        </div>
      ))}
      <Button
        disabled={!commands.length}
        onClick={() =>
          setDependencies((items) => [...items, { commandId: commands[0].id, reuse: 'shared' }])
        }
      >
        <Plus size={14} />
        {t('terminal:addDependency')}
      </Button>
      {!commands.length && <p className="muted small">{t('terminal:noCommands')}</p>}
    </section>
  );
}
