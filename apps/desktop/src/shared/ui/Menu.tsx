import * as Dropdown from '@radix-ui/react-dropdown-menu';
import * as Context from '@radix-ui/react-context-menu';
import type { ReactNode } from 'react';
import './menu.css';
function preserveDialogFocus(event: Event) {
  const dialog = document.querySelector<HTMLElement>('[role="dialog"][data-state="open"]');
  if (dialog) {
    event.preventDefault();
    dialog.querySelector<HTMLElement>('[data-autofocus]')?.focus();
  }
}
export interface MenuAction {
  label: string;
  icon?: ReactNode;
  disabled?: boolean;
  reason?: string;
  danger?: boolean;
  onSelect(): void;
}
export function Menu({
  trigger,
  actions,
  label,
}: {
  trigger: ReactNode;
  actions: MenuAction[];
  label?: string;
}) {
  return (
    <Dropdown.Root modal={false}>
      <Dropdown.Trigger asChild>{trigger}</Dropdown.Trigger>
      <Dropdown.Portal>
        <Dropdown.Content
          className="action-menu"
          sideOffset={5}
          align="end"
          aria-label={label}
          onCloseAutoFocus={preserveDialogFocus}
        >
          {actions.map((action) => (
            <Dropdown.Item
              key={action.label}
              className="action-menu-item"
              data-danger={action.danger || undefined}
              disabled={action.disabled}
              onSelect={action.onSelect}
              title={action.reason}
            >
              {action.icon}
              <span>
                {action.label}
                {action.reason && <small>{action.reason}</small>}
              </span>
            </Dropdown.Item>
          ))}
        </Dropdown.Content>
      </Dropdown.Portal>
    </Dropdown.Root>
  );
}
export function ContextMenu({ children, actions }: { children: ReactNode; actions: MenuAction[] }) {
  return (
    <Context.Root modal={false}>
      <Context.Trigger asChild>{children}</Context.Trigger>
      <Context.Portal>
        <Context.Content
          className="action-menu"
          onContextMenu={(event) => event.stopPropagation()}
          onCloseAutoFocus={preserveDialogFocus}
        >
          {actions.map((action) => (
            <Context.Item
              key={action.label}
              className="action-menu-item"
              data-danger={action.danger || undefined}
              disabled={action.disabled}
              onSelect={action.onSelect}
            >
              {action.icon}
              <span>{action.label}</span>
            </Context.Item>
          ))}
        </Context.Content>
      </Context.Portal>
    </Context.Root>
  );
}
