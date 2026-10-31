import { useTerminalStore } from './store';
import { useI18n } from '../../shared/i18n/react';
import { Dialog } from '../../shared/ui/Dialog';
import { Select, SelectItem } from '../../shared/ui/Select';
import type { TerminalConflict } from '../../shared/types';
import type { FolderClickMode } from './store/files.slice';
export function FileSettings({ onClose }: { onClose(): void }) {
  const { folderClick, conflict, setFileSettings } = useTerminalStore((s) => s);
  const { t } = useI18n();
  return (
    <Dialog title={t('terminal:fileSettings')} onClose={onClose}>
      <div className="terminal-form">
        <label className="form-field">
          <span>{t('terminal:folderClick')}</span>
          <Select
            value={folderClick}
            onValueChange={(value) => setFileSettings({ folderClick: value as FolderClickMode })}
          >
            <SelectItem value="expand">{t('terminal:expandFolder')}</SelectItem>
            <SelectItem value="navigate">{t('terminal:enterFolder')}</SelectItem>
          </Select>
        </label>
        <label className="form-field">
          <span>{t('terminal:conflict')}</span>
          <Select
            value={conflict}
            onValueChange={(value) => setFileSettings({ conflict: value as TerminalConflict })}
          >
            {(['fail', 'skip', 'overwrite'] as const).map((kind) => (
              <SelectItem key={kind} value={kind}>
                {t(`terminal:${kind}`)}
              </SelectItem>
            ))}
          </Select>
        </label>
        <p className="small muted">{t('terminal:fileSettingsHint')}</p>
      </div>
    </Dialog>
  );
}
