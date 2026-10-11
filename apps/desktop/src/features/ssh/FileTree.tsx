import { useEffect, useRef, useState } from 'react';
import {
  ArrowRight,
  ChevronDown,
  ChevronRight,
  Download,
  File,
  Folder,
  FolderOpen,
  Home,
  Link,
  RefreshCw,
  Upload,
} from 'lucide-react';
import {
  useDesktop,
  fileTarget,
  useCommand,
  useRemote,
  type RemoteWorkspace,
  type RemoteEntry,
} from '../../desktop/api';
import { useI18n } from '../../i18n/react';
import { Button, IconButton } from '../../ui/controls';
import { errorText, toProblem } from '../../i18n/errors';
export function FileTree({
  workspace,
  selectedText,
}: {
  workspace: RemoteWorkspace;
  selectedText: string;
}) {
  const desktop = useDesktop();
  const remote = useRemote();
  const { t } = useI18n();
  const { execute, pending } = useCommand();
  const [draft, setDraft] = useState(workspace.root ?? '');
  const [expanded, setExpanded] = useState<Set<string>>(new Set());
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [destination, setDestination] = useState<string>();
  const [drop, setDrop] = useState<string>();
  const ref = useRef<HTMLElement>(null);
  const current = useRef(workspace);
  current.current = workspace;
  const root = workspace.root;
  const ready = workspace.files.state === 'Ready';
  useEffect(() => {
    setDraft(root ?? '');
    setExpanded(new Set());
    setSelected(new Set());
    setDestination(undefined);
  }, [root, workspace.filesId, workspace.generation]);
  useEffect(() => {
    let disposed = false;
    let cleanup: (() => void) | undefined;
    void desktop
      .onFileDrop((event) => {
        if (event.type === 'leave') {
          setDrop(undefined);
          return;
        }
        const node = document
          .elementFromPoint(event.x, event.y)
          ?.closest<HTMLElement>('[data-ssh-drop]');
        const path = node && ref.current?.contains(node) ? node.dataset.sshDrop : undefined;
        setDrop(path || undefined);
        if (event.type === 'drop') {
          setDrop(undefined);
          if (path && current.current.files.state === 'Ready')
            void execute(() =>
              desktop.remote({
                type: 'upload',
                data: { target: fileTarget(current.current), directory: path, paths: event.paths },
              }),
            );
        }
      })
      .then((unsubscribe) => {
        if (disposed) unsubscribe();
        else cleanup = unsubscribe;
      })
      .catch(() => {});
    return () => {
      disposed = true;
      cleanup?.();
    };
  }, [desktop, execute]);
  const refreshing = useRef(new Set<string>());
  useEffect(() => {
    for (const directory of workspace.directories)
      if (
        directory.state === 'Stale' &&
        expanded.has(directory.path) &&
        !refreshing.current.has(directory.path)
      ) {
        refreshing.current.add(directory.path);
        void execute(() =>
          desktop.remote({
            type: 'list',
            data: { target: fileTarget(workspace), path: directory.path, more: false },
          }),
        ).finally(() => refreshing.current.delete(directory.path));
      }
  }, [workspace, expanded, desktop, execute]);
  const navigate = (path: string) =>
    execute(() =>
      desktop.remote({ type: 'navigate', data: { target: fileTarget(workspace), path } }),
    );
  const toggle = (path: string) => {
    const next = new Set(expanded);
    if (next.has(path)) {
      for (const open of expanded) {
        if (open === path || open.startsWith(path.replace(/\/$/, '') + '/')) {
          next.delete(open);
          void execute(() =>
            desktop.remote({
              type: 'collapse',
              data: { target: fileTarget(workspace), path: open },
            }),
          );
        }
      }
    } else {
      next.add(path);
      const d = workspace.directories.find((d) => d.path === path);
      if (!d || d.state !== 'Complete')
        void execute(() =>
          desktop.remote({
            type: 'list',
            data: { target: fileTarget(workspace), path, more: false },
          }),
        );
    }
    setExpanded(next);
    setDestination(path);
  };
  const children = (path: string, depth: number) => {
    if (!expanded.has(path)) return null;
    const directory = workspace.directories.find((d) => d.path === path);
    return (
      <div role="group">
        {directory?.entries.map((entry) => row(entry, depth))}
        {(!directory || directory.state === 'Loading') && (
          <p className="ssh-tree-note" role="status">
            {t('ssh:loading')}
          </p>
        )}
        {directory?.state === 'Complete' && !directory.entries.length && (
          <p className="ssh-tree-note">{t('ssh:emptyDirectory')}</p>
        )}
        {directory?.state === 'Error' && (
          <div className="ssh-tree-note ssh-error">
            {errorText(toProblem(directory.error))}
            <Button
              variant="quiet"
              onClick={() =>
                void execute(() =>
                  desktop.remote({
                    type: 'list',
                    data: { target: fileTarget(workspace), path, more: false },
                  }),
                )
              }
            >
              {t('ssh:retry')}
            </Button>
          </div>
        )}
        {directory?.state === 'Partial' && (
          <Button
            className="ssh-tree-more"
            variant="quiet"
            onClick={() =>
              void execute(() =>
                desktop.remote({
                  type: 'list',
                  data: { target: fileTarget(workspace), path, more: true },
                }),
              )
            }
          >
            {t('ssh:loadMore')}
          </Button>
        )}
      </div>
    );
  };
  const row = (entry: RemoteEntry, depth: number) => (
    <div
      key={entry.path}
      role="treeitem"
      aria-expanded={entry.kind === 'Directory' ? expanded.has(entry.path) : undefined}
      aria-selected={selected.has(entry.path)}
    >
      <div
        className={`ssh-tree-row ${selected.has(entry.path) ? 'is-selected' : ''} ${drop === entry.path ? 'is-drop' : ''}`}
        style={{ paddingLeft: 10 + depth * 16 }}
        data-ssh-drop={entry.kind === 'Directory' ? entry.path : ''}
      >
        {entry.kind === 'Directory' ? (
          <button className="ssh-tree-name" onClick={() => toggle(entry.path)} title={entry.path}>
            {expanded.has(entry.path) ? <ChevronDown size={13} /> : <ChevronRight size={13} />}
            <Folder size={15} />
            <span>{entry.name}</span>
          </button>
        ) : (
          <>
            <input
              type="checkbox"
              aria-label={t('ssh:selectFile', { name: entry.name })}
              disabled={entry.kind !== 'File'}
              checked={selected.has(entry.path)}
              onChange={(e) => {
                const next = new Set(selected);
                if (e.target.checked) next.add(entry.path);
                else next.delete(entry.path);
                setSelected(next);
              }}
            />
            <button
              className="ssh-tree-name"
              title={entry.path}
              onClick={() => {
                if (entry.kind === 'Symlink') void navigate(entry.path);
                else if (entry.kind === 'File') {
                  const next = new Set(selected);
                  if (next.has(entry.path)) next.delete(entry.path);
                  else next.add(entry.path);
                  setSelected(next);
                }
              }}
            >
              {entry.kind === 'Symlink' ? <Link size={14} /> : <File size={14} />}
              <span>{entry.name}</span>
            </button>
          </>
        )}
      </div>
      {entry.kind === 'Directory' && children(entry.path, depth + 1)}
    </div>
  );
  const target = destination ?? root;
  return (
    <aside ref={ref} className="ssh-files" aria-label={t('ssh:files')}>
      <header>
        <strong>{t('ssh:files')}</strong>
        <span className={`ssh-status ${ready ? 'is-ready' : ''}`}>
          SFTP · {t(`ssh:phase${workspace.files.state}`)}
        </span>
      </header>
      <form
        className="ssh-path"
        onSubmit={(e) => {
          e.preventDefault();
          void navigate(draft);
        }}
      >
        <input
          aria-label={t('ssh:path')}
          value={draft}
          disabled={!ready}
          placeholder={t('ssh:pathPlaceholder')}
          onChange={(e) => setDraft(e.target.value)}
        />
        <IconButton
          label={t('ssh:go')}
          disabled={!ready || pending}
          onClick={() => void navigate(draft)}
        >
          <ArrowRight size={15} />
        </IconButton>
      </form>
      <div className="ssh-file-tools">
        <IconButton
          label={t('ssh:initialDirectory')}
          disabled={!ready || !workspace.initialDirectory}
          onClick={() => void navigate(workspace.initialDirectory!)}
        >
          <Home size={15} />
        </IconButton>
        <IconButton
          label={t('ssh:refresh')}
          disabled={!ready || !root}
          onClick={() => {
            setExpanded((e) => new Set(e).add(root!));
            void execute(() =>
              desktop.remote({
                type: 'list',
                data: { target: fileTarget(workspace), path: root!, more: false },
              }),
            );
          }}
        >
          <RefreshCw size={14} />
        </IconButton>
        <Button
          variant="quiet"
          disabled={!ready || !selectedText.trim() || selectedText.includes('\n')}
          onClick={() => void navigate(selectedText.trim())}
        >
          {t('ssh:openSelection')}
        </Button>
      </div>
      <div
        className="ssh-tree"
        role="tree"
        aria-label={t('ssh:files')}
        aria-multiselectable="true"
        data-ssh-drop={ready ? (root ?? '') : ''}
      >
        {root && (
          <div role="treeitem" aria-expanded={expanded.has(root)}>
            <button
              className={`ssh-tree-root ${drop === root ? 'is-drop' : ''}`}
              data-ssh-drop={root}
              onClick={() => toggle(root)}
            >
              {expanded.has(root) ? <ChevronDown size={14} /> : <ChevronRight size={14} />}
              <FolderOpen size={16} />
              <span title={root}>{root}</span>
            </button>
            {children(root, 1)}
          </div>
        )}
        {!root && (
          <p className="ssh-tree-note">
            {workspace.files.error
              ? errorText(toProblem(workspace.files.error))
              : t('ssh:waitingFiles')}
          </p>
        )}
      </div>
      <div className="ssh-file-actions">
        <p title={drop ?? target ?? undefined}>
          {t(drop ? 'ssh:dropTarget' : 'ssh:uploadTarget')}{' '}
          <span className="mono">{drop ?? target ?? '—'}</span>
        </p>
        <div>
          <Button
            disabled={!ready || !target}
            onClick={() =>
              void execute(() =>
                desktop.remote({
                  type: 'selectUpload',
                  data: { target: fileTarget(workspace), directory: target! },
                }),
              )
            }
          >
            <Upload size={14} />
            {t('ssh:upload')}
          </Button>
          <Button
            disabled={!ready || !selected.size || !remote.downloadDirectory}
            onClick={() =>
              void execute(() =>
                desktop.remote({
                  type: 'download',
                  data: { target: fileTarget(workspace), paths: [...selected] },
                }),
              )
            }
          >
            <Download size={14} />
            {t('ssh:download', { count: selected.size })}
          </Button>
        </div>
      </div>
      <footer>
        <span>{t('ssh:downloadDirectory')}</span>
        <button
          title={remote.downloadDirectory ?? undefined}
          onClick={() => void execute(() => desktop.remote({ type: 'selectDownloadDirectory' }))}
        >
          <FolderOpen size={14} />
          <span>{remote.downloadDirectory ?? t('ssh:chooseDirectory')}</span>
        </button>
      </footer>
    </aside>
  );
}
