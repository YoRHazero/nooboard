import * as Popover from '@radix-ui/react-popover';
import { ArrowUpCircle } from 'lucide-react';
import { useSnapshot } from '../../shared/queries/useBackendSnapshotQuery';
import { useI18n } from '../../shared/i18n/react';
import { errorText, toProblem } from '../../shared/i18n/errors';
import { Button, IconButton } from '../../shared/ui/controls';
import { useInstallUpdateMutation } from './queries/useInstallUpdateMutation';
import './update.css';

export function UpdateNotice() {
  const { host } = useSnapshot();
  const { t } = useI18n();
  const install = useInstallUpdateMutation();
  const update = host.update;
  if (!update) return null;
  const busy = update.installing || install.isPending;
  const label = t('common:updateReady', { version: update.version });
  return (
    <Popover.Root>
      <Popover.Trigger asChild>
        <IconButton label={label} className="update-trigger">
          <ArrowUpCircle size={16} />
        </IconButton>
      </Popover.Trigger>
      <Popover.Portal>
        <Popover.Content
          className="update-notice"
          aria-label={label}
          side="top"
          align="start"
          sideOffset={8}
          collisionPadding={12}
        >
          <strong>{label}</strong>
          <Button
            variant="primary"
            disabled={busy}
            onClick={() => void install.mutateAsync().catch(() => {})}
          >
            {busy ? t('common:updateInstalling') : t('common:restartUpdate')}
          </Button>
          {install.error && (
            <p className="update-notice__error" role="alert">
              {errorText(toProblem(install.error))}
            </p>
          )}
        </Popover.Content>
      </Popover.Portal>
    </Popover.Root>
  );
}
