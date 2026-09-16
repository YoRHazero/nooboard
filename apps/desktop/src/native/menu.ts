import { Menu, Submenu, PredefinedMenuItem } from '@tauri-apps/api/menu';
import type { PredefinedMenuItemOptions } from '@tauri-apps/api/menu';
import { platform } from '@tauri-apps/plugin-os';
import { i18n, t } from '../i18n';
import type { resources } from '../i18n/resources';

type Label = keyof typeof resources.en.common;
type NativeItem = Submenu | PredefinedMenuItem;
/** Keep native actions and shortcuts while updating only our menu labels. */
export async function initializeMenu() {
  const owned: (NativeItem | Menu)[] = [];
  const labels: { item: NativeItem; label: Label }[] = [];
  const item = async (kind: PredefinedMenuItemOptions['item'], label?: Label) => {
    const native = await PredefinedMenuItem.new({ item: kind, text: label ? t(label) : undefined });
    owned.push(native);
    if (label) labels.push({ item: native, label });
    return native;
  };
  const submenu = async (text: string, items: NativeItem[], label?: Label) => {
    const native = await Submenu.new({ text, items });
    owned.push(native);
    if (label) labels.push({ item: native, label });
    return native;
  };
  const mac = platform() === 'macos';
  const menus: Submenu[] = [];
  try {
    if (mac)
      menus.push(
        await submenu('nooboard', [
          await item({ About: { name: 'nooboard', version: '0.1.0' } }, 'menuAbout'),
          await item('Separator'),
          await item('Services', 'menuServices'),
          await item('Separator'),
          await item('Hide', 'menuHide'),
          await item('HideOthers', 'menuHideOthers'),
          await item('ShowAll', 'menuShowAll'),
          await item('Separator'),
          await item('Quit', 'menuQuit'),
        ]),
      );
    menus.push(
      await submenu(
        t('menuFile'),
        [await item('CloseWindow', 'menuClose'), ...(!mac ? [await item('Quit', 'menuQuit')] : [])],
        'menuFile',
      ),
    );
    menus.push(
      await submenu(
        t('menuEdit'),
        [
          await item('Undo', 'menuUndo'),
          await item('Redo', 'menuRedo'),
          await item('Separator'),
          await item('Cut', 'menuCut'),
          await item('Copy', 'copy'),
          await item('Paste', 'paste'),
          await item('SelectAll', 'menuSelectAll'),
        ],
        'menuEdit',
      ),
    );
    if (mac)
      menus.push(
        await submenu(t('menuView'), [await item('Fullscreen', 'menuFullscreen')], 'menuView'),
      );
    const windows = await submenu(
      t('menuWindow'),
      [
        await item('Minimize', 'menuMinimize'),
        await item('Maximize', 'menuMaximize'),
        ...(mac ? [await item('Separator'), await item('BringAllToFront', 'menuBringAll')] : []),
      ],
      'menuWindow',
    );
    menus.push(windows);
    const help = await submenu(
      t('menuHelp'),
      mac ? [] : [await item({ About: { name: 'nooboard', version: '0.1.0' } }, 'menuAbout')],
      'menuHelp',
    );
    menus.push(help);
    const menu = await Menu.new({ items: menus });
    owned.push(menu);
    const previous = await menu.setAsAppMenu();
    await previous?.close();
    if (mac) {
      await windows.setAsWindowsMenuForNSApp();
      await help.setAsHelpMenuForNSApp();
    }
    let pending = Promise.resolve();
    const refresh = () => {
      const changes = labels.map(({ item, label }) => ({ item, text: t(label) }));
      pending = pending
        .then(async () => {
          for (const { item, text } of changes) await item.setText(text);
        })
        .catch((error: unknown) => console.error('Native menu update failed', error));
    };
    i18n.on('languageChanged', refresh);
    refresh();
    return () => {
      i18n.off('languageChanged', refresh);
      void pending.then(() => Promise.allSettled(owned.map((item) => item.close())));
    };
  } catch (error) {
    await Promise.allSettled(owned.map((item) => item.close()));
    throw error;
  }
}
