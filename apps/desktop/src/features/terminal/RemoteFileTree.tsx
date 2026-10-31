import { useEffect } from 'react';
import {
  ChevronRight,
  File,
  Folder,
  FolderOpen,
  FolderPlus,
  Download,
  Pencil,
  Trash2,
} from 'lucide-react';
import { toast } from 'sonner';
import type { TerminalDirectory } from '../../shared/types';
import { useI18n } from '../../shared/i18n/react';
import { errorText, toProblem } from '../../shared/i18n/errors';
import { ContextMenu } from '../../shared/ui/Menu';
import { useRemoteDirectoryQuery } from './queries/useRemoteDirectoryQuery';
import { joinRemote } from './remote-path';
import type { FileView, FolderClickMode } from './store/files.slice';
export type FileAction =
  | { kind: 'mkdir'; directory: string }
  | { kind: 'rename'; path: string }
  | { kind: 'deleteFiles'; paths: string[] };
interface Props {
  id: string;
  directory: TerminalDirectory;
  view: FileView;
  mode: FolderClickMode;
  navigate(path: string): void;
  update(view: Partial<FileView>): void;
  action(action: FileAction): void;
  download(paths: string[]): void;
}
function DirectoryBranch({ path, ...props }: Omit<Props, 'directory'> & { path: string }) {
  const query = useRemoteDirectoryQuery(props.id, path);
  const { t } = useI18n();
  useEffect(() => {
    if (query.error)
      toast.error(errorText(toProblem(query.error)), {
        id: `browse:${props.id}:${path}`,
        duration: Infinity,
      });
  }, [query.error, props.id, path]);
  return query.data ? (
    <RemoteFileTree {...props} directory={query.data} />
  ) : (
    <div className="file-branch-status">
      {query.error ? (
        <button onClick={() => void query.refetch()}>{t('common:retry')}</button>
      ) : (
        t('terminal:loadingFiles')
      )}
    </div>
  );
}
export function RemoteFileTree(props: Props) {
  const { directory, view, update, mode, navigate, action, download } = props;
  const { t } = useI18n();
  const entries = [...directory.entries].sort(
    (a, b) =>
      Number(b.kind === 'directory') - Number(a.kind === 'directory') ||
      a.name.localeCompare(b.name),
  );
  return (
    <ul className="remote-tree" aria-label={directory.path}>
      {entries.map((entry) => {
        const path = joinRemote(directory.path, entry.name);
        const folder = entry.kind === 'directory';
        const expanded = folder && view.expanded.includes(path);
        const toggle = () =>
          update({
            expanded: expanded ? view.expanded.filter((p) => p !== path) : [...view.expanded, path],
          });
        return (
          <li key={entry.name}>
            <ContextMenu
              actions={[
                ...(folder
                  ? [
                      {
                        label: t('terminal:mkdir'),
                        icon: <FolderPlus size={14} />,
                        onSelect: () => action({ kind: 'mkdir', directory: path }),
                      },
                      {
                        label: t('terminal:enterFolder'),
                        icon: <FolderOpen size={14} />,
                        onSelect: () => navigate(path),
                      },
                    ]
                  : []),
                {
                  label: t('terminal:download'),
                  icon: <Download size={14} />,
                  disabled: entry.kind === 'link',
                  onSelect: () => download([path]),
                },
                {
                  label: t('terminal:rename'),
                  icon: <Pencil size={14} />,
                  onSelect: () => action({ kind: 'rename', path }),
                },
                {
                  label: t('terminal:deleteFiles'),
                  icon: <Trash2 size={14} />,
                  danger: true,
                  onSelect: () => action({ kind: 'deleteFiles', paths: [path] }),
                },
              ]}
            >
              <div
                className={`terminal-file-row ${view.selected.includes(path) ? 'is-selected' : ''}`}
                onContextMenu={(event) => event.stopPropagation()}
              >
                {view.selecting && (
                  <input
                    type="checkbox"
                    aria-label={t('terminal:selectFile', { name: entry.name })}
                    checked={view.selected.includes(path)}
                    onChange={(event) =>
                      update({
                        selected: event.target.checked
                          ? [...view.selected, path]
                          : view.selected.filter((p) => p !== path),
                      })
                    }
                  />
                )}
                {folder ? (
                  <button
                    className="file-disclosure"
                    aria-label={entry.name}
                    aria-expanded={expanded}
                    onClick={toggle}
                  >
                    <ChevronRight size={12} />
                  </button>
                ) : (
                  <span className="file-disclosure" />
                )}
                {folder ? (
                  expanded ? (
                    <FolderOpen size={15} />
                  ) : (
                    <Folder size={15} />
                  )
                ) : (
                  <File size={15} />
                )}
                <button
                  className="file-name"
                  title={entry.kind === 'link' ? t('terminal:linkHint') : path}
                  onClick={() => {
                    if (folder) mode === 'navigate' ? navigate(path) : toggle();
                    else if (view.selecting)
                      update({
                        selected: view.selected.includes(path)
                          ? view.selected.filter((p) => p !== path)
                          : [...view.selected, path],
                      });
                  }}
                >
                  {entry.name}
                </button>
                <span className="muted file-size">
                  {entry.size == null ? '' : `${entry.size} B`}
                </span>
              </div>
            </ContextMenu>
            {expanded && <DirectoryBranch {...props} path={path} />}
          </li>
        );
      })}
      {!entries.length && <li className="file-branch-status">{t('terminal:emptyDirectory')}</li>}
    </ul>
  );
}
