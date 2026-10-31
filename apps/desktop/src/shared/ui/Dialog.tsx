import * as DialogPrimitive from '@radix-ui/react-dialog';
import { useI18n } from '../i18n/react';
import { useRef, type ReactNode } from 'react';
import { X } from 'lucide-react';
import { IconButton } from './controls';

export function Dialog({
  title,
  onClose,
  children,
}: {
  title: string;
  onClose: () => void;
  children: ReactNode;
}) {
  const { t } = useI18n();
  const ref = useRef<HTMLDivElement>(null);
  const opener = useRef<HTMLElement | null>(
    document.activeElement instanceof HTMLElement ? document.activeElement : null,
  );
  return (
    <DialogPrimitive.Root
      open
      onOpenChange={(open) => {
        if (!open) onClose();
      }}
    >
      <DialogPrimitive.Portal>
        <DialogPrimitive.Overlay className="dialog-overlay" />
        <DialogPrimitive.Content
          ref={ref}
          className="dialog"
          aria-describedby={undefined}
          onOpenAutoFocus={(event) => {
            const field = ref.current?.querySelector<HTMLElement>('[data-autofocus]');
            if (field) {
              event.preventDefault();
              field.focus();
            }
          }}
          onCloseAutoFocus={(event) => {
            event.preventDefault();
            const next = document.querySelector<HTMLElement>('[role="dialog"][data-state="open"]');
            if (next && next !== ref.current) {
              next.querySelector<HTMLElement>('[data-autofocus]')?.focus();
              return;
            }
            if (opener.current?.isConnected) opener.current.focus({ preventScroll: true });
          }}
        >
          <div className="dialog__surface">
            <header>
              <DialogPrimitive.Title asChild>
                <h2>{title}</h2>
              </DialogPrimitive.Title>
              <IconButton label={t('common:closeDialog')} onClick={onClose}>
                <X size={18} />
              </IconButton>
            </header>
            {children}
          </div>
        </DialogPrimitive.Content>
      </DialogPrimitive.Portal>
    </DialogPrimitive.Root>
  );
}
