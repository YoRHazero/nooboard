import { useEffect } from 'react';
import { Toaster, toast } from 'sonner';
import { useConnection, useFeedback } from '../../shared/store';
import { useBackendSnapshotQuery } from '../../shared/queries/useBackendSnapshotQuery';
import { useReconnectMutation } from '../../shared/queries/useReconnectMutation';
import { usePreferenceStore } from '../../features/settings/store';
import { useI18n } from '../../shared/i18n/react';
import { errorText } from '../../shared/i18n/errors';
import { handleAction } from '../../shared/action-feedback';
import './notifications.css';

export function Notifications() {
  const { error, clearError } = useFeedback();
  const connection = useConnection();
  const notice = useBackendSnapshotQuery((snapshot) => snapshot.notice).data;
  const theme = usePreferenceStore((state) => state.appearance.theme);
  const reconnect = useReconnectMutation();
  const { t } = useI18n();
  useEffect(() => {
    if (!error) return;
    const message = errorText(error);
    toast.error(message, { id: `error:${message}`, duration: Infinity });
    clearError();
  }, [error, clearError]);
  useEffect(() => {
    if (connection.error)
      toast.error(errorText(connection.error), {
        id: 'connection',
        duration: Infinity,
        dismissible: false,
        closeButton: false,
        action: {
          label: t('common:reconnect'),
          onClick: (event) => {
            event.preventDefault();
            void handleAction(() => reconnect.mutateAsync([]));
          },
        },
      });
    else toast.dismiss('connection');
  }, [connection.error, t]);
  useEffect(() => {
    if (notice)
      toast.warning(errorText(notice.message), { id: `notice:${notice.id}`, duration: Infinity });
  }, [notice]);
  return (
    <Toaster
      theme={theme}
      position="bottom-right"
      closeButton
      duration={4500}
      visibleToasts={4}
      offset={24}
      toastOptions={{ closeButtonAriaLabel: t('common:dismiss') }}
    />
  );
}
