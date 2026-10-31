import { useEffect, useState } from 'react';
import {
  CheckSquare,
  Download,
  FolderPlus,
  RefreshCw,
  Settings2,
  Trash2,
  Upload,
  X,
} from 'lucide-react';
import { toast } from 'sonner';
import { useQueryClient } from '@tanstack/react-query';
import { useSnapshot } from '../../shared/queries/useBackendSnapshotQuery';
import { useActionFeedback } from '../../shared/action-feedback';
import { useI18n } from '../../shared/i18n/react';
import { errorText, toProblem, fail } from '../../shared/i18n/errors';
import { Button, IconButton } from '../../shared/ui/controls';
import { Menu, ContextMenu } from '../../shared/ui/Menu';
import { Dialog } from '../../shared/ui/Dialog';
import { useRenameMutation } from './queries/useRenameMutation';
import { useDeleteFilesMutation } from './queries/useDeleteFilesMutation';
import { useRemoteDirectoryQuery } from './queries/useRemoteDirectoryQuery';
import { useMkdirMutation } from './queries/useMkdirMutation';
import { useDownloadMutation } from './queries/useDownloadMutation';
import { useUploadMutation } from './queries/useUploadMutation';
import { useTerminalStore } from './store';
import { emptyFileView, type FileView } from './store/files.slice';
import { FileSettings } from './FileSettings';
import { DirectoryPath } from './DirectoryPath';
import { RemoteFileTree, type FileAction } from './RemoteFileTree';
import { basenameRemote, joinRemote, parentRemote, topLevelPaths } from './remote-path';
import './remote-files.css';

export function RemoteFiles({
  id,
  name: targetName,
  onClose,
}: {
  id: string;
  name: string;
  onClose(): void;
}) {
  const { t } = useI18n();
  const view = useTerminalStore((state) => state.fileViews[id] ?? emptyFileView);
  const conflict = useTerminalStore((state) => state.conflict);
  const mode = useTerminalStore((state) => state.folderClick);
  const updateView = useTerminalStore((state) => state.updateFileView);
  const update = (patch: Partial<FileView>) => updateView(id, patch);
  const directoryQuery = useRemoteDirectoryQuery(id, view.path);
  const directory = directoryQuery.data;
  const queryClient = useQueryClient();
  const session = useSnapshot((s) => s.session);
  const upload = useUploadMutation();
  const download = useDownloadMutation();
  const mkdir = useMkdirMutation();
  const rename = useRenameMutation();
  const remove = useDeleteFilesMutation();
  const { execute, pending } = useActionFeedback([upload, download, mkdir, rename, remove]);
  const [settings, setSettings] = useState(false);
  const [action, setAction] = useState<FileAction | null>(null);
  const [name, setName] = useState('');
  const refresh = () =>
    queryClient.invalidateQueries({ queryKey: ['terminal', 'directory', session, id] });
  useEffect(() => {
    if (directoryQuery.error)
      toast.error(errorText(toProblem(directoryQuery.error)), {
        id: `browse:${id}:${view.path}`,
        duration: Infinity,
      });
  }, [directoryQuery.error, id, view.path]);
  const navigate = (path: string) => {
    update({ path, selected: [], expanded: [] });
    if (path === view.path) void directoryQuery.refetch();
  };
  const openAction = (action: FileAction) => {
    setName(action.kind === 'rename' ? basenameRemote(action.path) : '');
    setAction(action);
  };
  const downloadPaths = (paths: string[]) =>
    void execute(() => download.mutateAsync({ id, paths: topLevelPaths(paths), conflict }));
  const apply = () =>
    execute(async () => {
      if (!action) return;
      if (
        action.kind !== 'deleteFiles' &&
        (!name || name === '.' || name === '..' || /[/\\\0]/.test(name))
      )
        throw fail('terminalInvalidFileName');
      if (action.kind === 'mkdir')
        await mkdir.mutateAsync({ id, path: joinRemote(action.directory, name) });
      else if (action.kind === 'rename')
        await rename.mutateAsync({
          id,
          source: action.path,
          destination: joinRemote(parentRemote(action.path), name),
        });
      else await remove.mutateAsync({ id, paths: topLevelPaths(action.paths) });
      if (action.kind !== 'deleteFiles')
        toast.success(
          t(action.kind === 'mkdir' ? 'terminal:folderCreated' : 'terminal:fileRenamed'),
        );
      setAction(null);
      update({ selected: [] });
      await refresh();
    });
  return (
    <aside className="terminal-files" aria-label={t('terminal:files')}>
      <header>
        <div>
          <h2>{t('terminal:files')}</h2>
          <span className="muted small" title={targetName}>
            {targetName}
          </span>
        </div>
        <div className="terminal-file-actions">
          <IconButton label={t('terminal:fileSettings')} onClick={() => setSettings(true)}>
            <Settings2 size={15} />
          </IconButton>
          <IconButton label={t('common:close')} onClick={onClose}>
            <X size={16} />
          </IconButton>
        </div>
      </header>
      <DirectoryPath
        id={id}
        root={directory?.path}
        loading={directoryQuery.isFetching}
        navigate={navigate}
        reload={() => void refresh()}
      />
      <ContextMenu
        actions={[
          {
            label: t('terminal:mkdir'),
            icon: <FolderPlus size={14} />,
            disabled: !directory,
            onSelect: () => directory && openAction({ kind: 'mkdir', directory: directory.path }),
          },
          {
            label: t('terminal:refresh'),
            icon: <RefreshCw size={14} />,
            onSelect: () => void refresh(),
          },
        ]}
      >
        <div className="terminal-file-list">
          {directory ? (
            <RemoteFileTree
              id={id}
              directory={directory}
              view={view}
              update={update}
              mode={mode}
              navigate={navigate}
              action={openAction}
              download={downloadPaths}
            />
          ) : (
            <div className="file-branch-status">
              {directoryQuery.error ? (
                <Button onClick={() => void directoryQuery.refetch()}>{t('common:retry')}</Button>
              ) : (
                t('terminal:loadingFiles')
              )}
            </div>
          )}
        </div>
      </ContextMenu>
      <footer className="terminal-file-footer">
        <span className="muted small">
          {view.selecting
            ? t('terminal:selectedCount', { count: view.selected.length })
            : t(mode === 'expand' ? 'terminal:expandFolder' : 'terminal:enterFolder')}
        </span>
        <div className="terminal-file-actions">
          <Menu
            trigger={
              <IconButton
                label={t('terminal:uploadTo', { path: directory?.path ?? '—' })}
                disabled={!directory || pending}
              >
                <Upload size={16} />
              </IconButton>
            }
            actions={[false, true].map((folder) => ({
              label: t(folder ? 'terminal:uploadFolder' : 'terminal:uploadFiles'),
              onSelect: () => {
                if (directory)
                  void execute(() =>
                    upload.mutateAsync({ id, directory: directory.path, folder, conflict }),
                  );
              },
            }))}
          />
          <IconButton
            label={t('terminal:download')}
            disabled={!view.selected.length || pending}
            onClick={() => downloadPaths(view.selected)}
          >
            <Download size={16} />
          </IconButton>
          <IconButton
            label={t(view.selecting ? 'terminal:exitSelection' : 'terminal:selectFiles')}
            aria-pressed={view.selecting}
            onClick={() => update({ selecting: !view.selecting, selected: [] })}
          >
            <CheckSquare size={16} />
          </IconButton>
          <IconButton
            label={t('terminal:deleteFiles')}
            disabled={!view.selected.length || pending}
            onClick={() => openAction({ kind: 'deleteFiles', paths: [...view.selected] })}
          >
            <Trash2 size={15} />
          </IconButton>
        </div>
      </footer>
      {settings && <FileSettings onClose={() => setSettings(false)} />}
      {action && (
        <Dialog title={t(`terminal:${action.kind}`)} onClose={() => setAction(null)}>
          <form
            className="terminal-form"
            onSubmit={(event) => {
              event.preventDefault();
              if (!pending) void apply();
            }}
          >
            {action.kind === 'deleteFiles' ? (
              <>
                <p>
                  {t('terminal:deleteFilesQuestion', { count: topLevelPaths(action.paths).length })}
                </p>
                <ul className="file-delete-paths">
                  {topLevelPaths(action.paths).map((path) => (
                    <li key={path}>{path}</li>
                  ))}
                </ul>
              </>
            ) : (
              <>
                <p className="small muted mono">
                  {action.kind === 'mkdir' ? action.directory : action.path}
                </p>
                <label className="form-field">
                  <span>{t('terminal:fileName')}</span>
                  <input
                    required
                    data-autofocus
                    autoFocus
                    value={name}
                    onChange={(event) => setName(event.target.value)}
                  />
                </label>
              </>
            )}
            <div className="dialog-actions">
              <Button onClick={() => setAction(null)}>{t('common:cancel')}</Button>
              <Button
                type="submit"
                variant={action.kind === 'deleteFiles' ? 'danger' : 'primary'}
                disabled={pending}
              >
                {t(`terminal:${action.kind}`)}
              </Button>
            </div>
          </form>
        </Dialog>
      )}
    </aside>
  );
}
export function RemoteFilesPanel() {
  const selected = useTerminalStore((state) => state.selected);
  const open = useTerminalStore((state) => state.filesOpen);
  const close = useTerminalStore((state) => state.closeFiles);
  const runs = useSnapshot((snapshot) => snapshot.terminal.runs);
  const run = runs.find(
    (run) => selected && run.target.id === selected.id && run.target.kind === selected.kind,
  );
  return open && run?.target.kind === 'terminal' && run.sshTarget ? (
    <RemoteFiles key={run.target.id} id={run.target.id} name={run.name} onClose={close} />
  ) : null;
}
