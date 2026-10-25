import { Select, SelectItem } from '../../shared/ui/Select';
import { useCallback, useEffect, useRef, useState } from 'react';
import { ArrowUp, File, Folder, RefreshCw, X } from 'lucide-react';
import type { TerminalConflict, TerminalDirectory } from '../../client/api';
import { useCommand, useDesktop, useSnapshot } from '../../client/react';
import { useI18n } from '../../shared/i18n/react';
import { errorText } from '../../shared/i18n/errors';
import { Button, IconButton } from '../../shared/ui/controls';
import { Dialog } from '../../shared/ui/Dialog';
const join = (directory: string, name: string) => `${directory.replace(/\/$/, '')}/${name}`;
export function RemoteFiles({ id, onClose }: { id: string; onClose: () => void }) {
  const { terminal } = useDesktop();
  const { t } = useI18n();
  const files = useSnapshot((s) => s.terminal.files);
  const { execute, pending, error } = useCommand();
  const [directory, setDirectory] = useState<TerminalDirectory | null>(null);
  const [path, setPath] = useState('');
  const [selected, setSelected] = useState<string[]>([]);
  const [conflict, setConflict] = useState<TerminalConflict>('fail');
  const [action, setAction] = useState<'mkdir' | 'rename' | 'deleteFiles' | null>(null);
  const [name, setName] = useState('');
  const generation = useRef(0);
  const browse = useCallback(
    (path?: string) => {
      const current = ++generation.current;
      return execute(async () => {
        const reply = await terminal.request({ type: 'browse', data: { id, path } });
        if (current === generation.current && reply.type === 'terminalDirectory') {
          setDirectory(reply.data);
          setPath(reply.data.path);
          setSelected([]);
        }
      });
    },
    [terminal, id, execute],
  );
  useEffect(() => {
    void browse();
    return () => {
      generation.current++;
    };
  }, [browse]);
  const paths = selected.map((name) => join(directory?.path ?? '', name));
  const apply = () =>
    execute(async () => {
      if (!action || !directory) return;
      if (action === 'mkdir')
        await terminal.request({ type: action, data: { id, path: join(directory.path, name) } });
      else if (action === 'rename')
        await terminal.request({
          type: action,
          data: { id, source: paths[0], destination: join(directory.path, name) },
        });
      else await terminal.request({ type: action, data: { id, paths } });
      setAction(null);
      await browse(directory.path);
    });
  return (
    <aside className="terminal-files" aria-label={t('terminal:files')}>
      <header>
        <h2>{t('terminal:files')}</h2>
        <IconButton label={t('common:close')} onClick={onClose}>
          <X size={16} />
        </IconButton>
      </header>
      <p className="muted small">{t('terminal:filesIndependent')}</p>
      <form
        className="terminal-path"
        onSubmit={(e) => {
          e.preventDefault();
          void browse(path);
        }}
      >
        <input
          aria-label={t('terminal:path')}
          value={path}
          onChange={(e) => setPath(e.target.value)}
        />
        <Button type="submit" disabled={pending}>
          {t('terminal:go')}
        </Button>
      </form>
      <div className="terminal-file-actions">
        <IconButton
          label={t('terminal:up')}
          disabled={!directory || pending}
          onClick={() => browse(directory!.path.replace(/\/[^/]+\/?$/, '') || '/')}
        >
          <ArrowUp size={16} />
        </IconButton>
        <IconButton
          label={t('terminal:refresh')}
          disabled={pending}
          onClick={() => browse(directory?.path)}
        >
          <RefreshCw size={15} />
        </IconButton>
        <Button
          disabled={!directory || pending}
          onClick={() => {
            setName('');
            setAction('mkdir');
          }}
        >
          {t('terminal:mkdir')}
        </Button>
        <Button
          disabled={!directory || pending}
          onClick={() =>
            execute(() =>
              terminal.request({
                type: 'upload',
                data: { id, directory: directory!.path, conflict, folder: false },
              }),
            )
          }
        >
          {t('terminal:upload')}
        </Button>
      </div>
      <Button
        disabled={!directory || pending}
        onClick={() =>
          execute(() =>
            terminal.request({
              type: 'upload',
              data: { id, directory: directory!.path, conflict, folder: true },
            }),
          )
        }
      >
        {t('terminal:uploadFolder')}
      </Button>
      <label className="form-field terminal-conflict">
        <span>{t('terminal:conflict')}</span>
        <Select
          aria-label={t('terminal:conflict')}
          value={conflict}
          onValueChange={(e) => setConflict(e as TerminalConflict)}
        >
          {(['fail', 'skip', 'overwrite'] as const).map((kind) => (
            <SelectItem key={kind} value={kind}>
              {t(`terminal:${kind}`)}
            </SelectItem>
          ))}
        </Select>
      </label>
      <div className="terminal-file-list">
        {directory?.entries.map((entry) => (
          <div key={entry.name} className="terminal-file-row">
            <input
              type="checkbox"
              aria-label={t('terminal:selectFile', { name: entry.name })}
              checked={selected.includes(entry.name)}
              onChange={(e) =>
                setSelected((items) =>
                  e.target.checked
                    ? [...items, entry.name]
                    : items.filter((name) => name !== entry.name),
                )
              }
            />
            {entry.kind === 'directory' ? <Folder size={15} /> : <File size={15} />}
            <button
              disabled={entry.kind !== 'directory' || pending}
              title={entry.kind === 'link' ? t('terminal:linkHint') : entry.name}
              onClick={() => browse(join(directory!.path, entry.name))}
            >
              {entry.name}
            </button>
            <span className="muted small">{entry.size ?? ''}</span>
          </div>
        ))}
        {directory && !directory.entries.length && (
          <p className="muted small">{t('terminal:emptyDirectory')}</p>
        )}
      </div>
      <div className="terminal-file-actions">
        <Button
          disabled={!selected.length || pending}
          onClick={() =>
            execute(() => terminal.request({ type: 'download', data: { id, paths, conflict } }))
          }
        >
          {t('terminal:download')}
        </Button>
        <Button
          disabled={selected.length !== 1 || pending}
          onClick={() => {
            setName(selected[0]);
            setAction('rename');
          }}
        >
          {t('terminal:rename')}
        </Button>
        <Button
          variant="quiet"
          disabled={!selected.length || pending}
          onClick={() => setAction('deleteFiles')}
        >
          {t('terminal:deleteFiles')}
        </Button>
      </div>
      {files
        .filter((file) => file.terminalId === id)
        .map((file) => (
          <div className="terminal-file-progress" key={file.id}>
            <span>
              {file.operation} · {t(`terminal:${file.phase}`)}
              <br />
              {t('terminal:fileProgress', {
                completed: file.completed,
                total: file.total ?? '—',
                bytes: file.bytes,
              })}
            </span>
            <IconButton
              label={t('terminal:stop')}
              disabled={file.phase === 'stopping'}
              onClick={() =>
                execute(() => terminal.request({ type: 'stopFiles', data: { id: file.id } }))
              }
            >
              <X size={14} />
            </IconButton>
          </div>
        ))}
      {error && (
        <p className="form-error" role="alert">
          {errorText(error)}
        </p>
      )}
      {action && (
        <Dialog title={t(`terminal:${action}`)} onClose={() => setAction(null)}>
          <form
            className="terminal-form"
            onSubmit={(e) => {
              e.preventDefault();
              if (!pending) void apply();
            }}
          >
            {action === 'deleteFiles' ? (
              <p>{t('terminal:deleteFilesQuestion', { count: selected.length })}</p>
            ) : (
              <label className="form-field">
                <span>{t('terminal:fileName')}</span>
                <input
                  required
                  autoFocus
                  data-autofocus
                  pattern="[^/\\\\]+"
                  value={name}
                  onChange={(e) => setName(e.target.value)}
                />
              </label>
            )}
            {error && (
              <p className="form-error" role="alert">
                {errorText(error)}
              </p>
            )}
            <div className="dialog-actions">
              <Button onClick={() => setAction(null)}>{t('common:cancel')}</Button>
              <Button
                variant={action === 'deleteFiles' ? 'danger' : 'primary'}
                type="submit"
                disabled={pending}
              >
                {t(`terminal:${action}`)}
              </Button>
            </div>
          </form>
        </Dialog>
      )}
    </aside>
  );
}
