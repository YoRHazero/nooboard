import { useI18n } from '../../i18n/react';
import type { ReactNode } from 'react';
import { History, Palette, Radio, FolderOpen, PanelBottom } from 'lucide-react';
import {
  useDesktop,
  useCommand,
  useSnapshot,
  usePreferences,
  type LanguagePreference,
} from '../../desktop/api';
import type { Theme } from '../../desktop/api';
import { Button, Toggle } from '../../ui/controls';

function SettingRow({
  title,
  description,
  children,
}: {
  title: string;
  description?: string;
  children: ReactNode;
}) {
  return (
    <div className="setting-row">
      <div>
        <h3>{title}</h3>
        {description && <p>{description}</p>}
      </div>
      <div className="setting-row__control">{children}</div>
    </div>
  );
}
export function SettingsPage() {
  const { t } = useI18n();
  const { language, appearance } = usePreferences();
  const { settings, host, configuration } = useSnapshot();
  const client = useDesktop();
  const { execute } = useCommand();
  const update = (patch: Parameters<typeof client.updateSyncSettings>[0]) =>
    execute(() => client.updateSyncSettings(patch));
  return (
    <div className="settings-page">
      {configuration.restartRequired && <p role="status">{t('settings:restartRequired')}</p>}
      {host.traySupported && (
        <section className="settings-group">
          <div className="settings-group__heading">
            <PanelBottom size={19} />
            <h2>{t('settings:background')}</h2>
          </div>
          <SettingRow title={t('settings:closeToTray')} description={t('settings:closeToTrayHelp')}>
            <Toggle
              label={t('settings:closeToTray')}
              checked={!!host.closeToTray}
              disabled={!host.trayAvailable || host.preferenceError}
              onChange={(closeToTray) =>
                execute(() => client.updateHostPreferences({ closeToTray }))
              }
            />
          </SettingRow>
          {!host.trayAvailable && <p role="status">{t('settings:trayUnavailable')}</p>}
          {host.preferenceError && <p role="alert">{t('errors:desktopPreferences')}</p>}
        </section>
      )}
      <section className="settings-group">
        <div className="settings-group__heading">
          <Radio size={19} />
          <h2>{t('settings:textSync')}</h2>
        </div>
        <SettingRow title={t('settings:mode')}>
          <div className="segmented">
            <button
              aria-pressed={settings.mode === 'manual'}
              onClick={() => update({ mode: 'manual' })}
            >
              {t('common:manual')}
            </button>
            <button
              aria-pressed={settings.mode === 'automatic'}
              onClick={() => update({ mode: 'automatic' })}
            >
              {t('common:automatic')}
            </button>
          </div>
        </SettingRow>
        <SettingRow title={t('settings:receive')}>
          <Toggle
            label={t('common:receiveOn')}
            checked={settings.receive}
            onChange={(receive) => update({ receive })}
          />
        </SettingRow>
        <SettingRow title={t('common:pause')}>
          <Toggle
            label={t('common:pause')}
            checked={settings.paused}
            onChange={(paused) => update({ paused })}
          />
        </SettingRow>
      </section>
      <section className="settings-group">
        <div className="settings-group__heading">
          <FolderOpen size={19} />
          <h2>{t('common:transfers')}</h2>
        </div>
        <SettingRow
          title={t('transfers:receiveDirectory')}
          description={t('transfers:directoryHelp')}
        >
          <Button variant="quiet" onClick={() => execute(() => client.selectReceiveDirectory())}>
            {t('transfers:changeDirectory')}
          </Button>
        </SettingRow>
        <p className="receive-directory-path">
          {settings.receiveDirectory || t('transfers:noDirectory')}
        </p>
      </section>
      <section className="settings-group">
        <div className="settings-group__heading">
          <History size={19} />
          <h2>{t('common:history')}</h2>
        </div>
        <SettingRow title={t('settings:history')} description={t('settings:localOnly')}>
          <Toggle
            label={t('settings:history')}
            checked={settings.history}
            onChange={(history) => update({ history })}
          />
        </SettingRow>
        <SettingRow title={t('settings:retention')}>
          <select
            aria-label={t('settings:retentionLabel')}
            value={settings.historyDays}
            onChange={(event) => update({ historyDays: Number(event.target.value) })}
          >
            <option value={7}>{t('common:days', { count: 7 })}</option>
            <option value={30}>{t('common:days', { count: 30 })}</option>
            <option value={90}>{t('common:days', { count: 90 })}</option>
            <option value={365}>{t('common:year')}</option>
          </select>
        </SettingRow>
        <SettingRow title={t('settings:limit')}>
          <select
            aria-label={t('settings:limitLabel')}
            value={settings.maxHistoryEntries}
            onChange={(event) => update({ maxHistoryEntries: Number(event.target.value) })}
          >
            <option value={100}>{t('common:items', { count: 100 })}</option>
            <option value={500}>{t('common:items', { count: 500 })}</option>
            <option value={1000}>{t('common:items', { count: 1000 })}</option>
            <option value={5000}>{t('common:items', { count: 5000 })}</option>
          </select>
        </SettingRow>
      </section>
      <section className="settings-group">
        <div className="settings-group__heading">
          <Palette size={19} />
          <h2>{t('settings:interface')}</h2>
        </div>
        <SettingRow title={t('common:language')}>
          <select
            aria-label={t('common:language')}
            value={language}
            onChange={(event) =>
              execute(() => client.setLanguage(event.target.value as LanguagePreference))
            }
          >
            <option value="system">{t('common:system')}</option>
            <option value="zh-CN">{t('common:chinese')}</option>
            <option value="en">{t('common:english')}</option>
          </select>
        </SettingRow>
        <SettingRow title={t('settings:appearance')}>
          <select
            aria-label={t('settings:appearanceLabel')}
            value={appearance.theme}
            onChange={(event) =>
              execute(() => client.updateAppearance({ theme: event.target.value as Theme }))
            }
          >
            <option value="system">{t('common:system')}</option>
            <option value="light">{t('common:light')}</option>
            <option value="dark">{t('common:dark')}</option>
          </select>
        </SettingRow>
        <SettingRow title={t('settings:reduceMotion')}>
          <Toggle
            label={t('settings:reduceMotion')}
            checked={appearance.reducedMotion}
            onChange={(reducedMotion) => execute(() => client.updateAppearance({ reducedMotion }))}
          />
        </SettingRow>
      </section>
    </div>
  );
}
