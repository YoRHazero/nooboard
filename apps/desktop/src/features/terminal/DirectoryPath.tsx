import { useEffect, useId, useState } from 'react';
import { ArrowRight, ArrowUp, Folder, RefreshCw } from 'lucide-react';
import { useI18n } from '../../shared/i18n/react';
import { IconButton } from '../../shared/ui/controls';
import { useRemoteDirectoryQuery } from './queries/useRemoteDirectoryQuery';
import { completionParts, joinRemote, parentRemote } from './remote-path';
export function DirectoryPath({
  id,
  root,
  loading,
  navigate,
  reload,
}: {
  id: string;
  root?: string;
  loading: boolean;
  navigate(path: string): void;
  reload(): void;
}) {
  const { t } = useI18n();
  const listId = useId();
  const [input, setInput] = useState(root ?? '');
  const [debounced, setDebounced] = useState(input);
  const [suggesting, setSuggesting] = useState(false);
  const [active, setActive] = useState(-1);
  useEffect(() => {
    if (root !== undefined) {
      setInput(root);
      setSuggesting(false);
    }
  }, [root]);
  useEffect(() => {
    const timer = setTimeout(() => setDebounced(input), 180);
    return () => clearTimeout(timer);
  }, [input]);
  const { parent, prefix } = completionParts(debounced, root ?? '/');
  const query = useRemoteDirectoryQuery(id, parent, {
    enabled: suggesting && !!root,
    staleTime: 15_000,
  });
  const candidates =
    debounced === input
      ? (query.data?.entries ?? [])
          .filter((entry) => entry.kind === 'directory' && entry.name.startsWith(prefix))
          .sort((a, b) => a.name.localeCompare(b.name))
          .map((entry) => joinRemote(query.data!.path, entry.name))
      : [];
  const choose = (path: string) => {
    setInput(`${path}/`);
    setSuggesting(false);
    setActive(-1);
  };
  const go = () => {
    if (!input.trim() || loading) return;
    setSuggesting(false);
    navigate(input.startsWith('/') ? input : joinRemote(root ?? '/', input));
  };
  return (
    <form
      className="terminal-path"
      onSubmit={(event) => {
        event.preventDefault();
        go();
      }}
    >
      <div className="terminal-path-input">
        <input
          role="combobox"
          aria-label={t('terminal:path')}
          aria-autocomplete="list"
          aria-expanded={suggesting && candidates.length > 0}
          aria-controls={listId}
          aria-activedescendant={
            active >= 0 && candidates[active] ? `${listId}-${active}` : undefined
          }
          value={input}
          onChange={(event) => {
            setInput(event.target.value);
            setSuggesting(true);
            setActive(-1);
          }}
          onBlur={() => setSuggesting(false)}
          onKeyDown={(event) => {
            if (event.key === 'Escape') {
              setSuggesting(false);
              setActive(-1);
            }
            if ((event.key === 'ArrowDown' || event.key === 'ArrowUp') && candidates.length) {
              event.preventDefault();
              setSuggesting(true);
              setActive(
                (i) =>
                  (i + (event.key === 'ArrowDown' ? 1 : -1) + candidates.length) %
                  candidates.length,
              );
            }
            if (event.key === 'Tab' && suggesting && candidates.length) {
              event.preventDefault();
              choose(candidates[Math.max(0, active)]);
            }
            if (event.key === 'Enter' && suggesting && active >= 0 && candidates[active]) {
              event.preventDefault();
              choose(candidates[active]);
            }
          }}
        />
        {suggesting && candidates.length > 0 && (
          <div className="path-suggestions" id={listId} role="listbox">
            {candidates.map((path, index) => (
              <button
                type="button"
                role="option"
                aria-selected={active === index}
                id={`${listId}-${index}`}
                key={path}
                onMouseDown={(event) => event.preventDefault()}
                onClick={() => choose(path)}
              >
                <Folder size={13} />
                <span>{path}</span>
              </button>
            ))}
          </div>
        )}
      </div>
      <IconButton type="submit" label={t('terminal:go')} disabled={loading || !input.trim()}>
        <ArrowRight size={15} />
      </IconButton>
      <IconButton
        label={t('terminal:up')}
        disabled={loading || !root || root === '/'}
        onClick={() => navigate(parentRemote(root!))}
      >
        <ArrowUp size={15} />
      </IconButton>
      <IconButton label={t('terminal:refresh')} disabled={loading} onClick={reload}>
        <RefreshCw size={14} className={loading ? 'is-spinning' : ''} />
      </IconButton>
    </form>
  );
}
