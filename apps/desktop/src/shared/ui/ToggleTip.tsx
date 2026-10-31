import * as Popover from '@radix-ui/react-popover';
import { Info } from 'lucide-react';
import type { ReactNode } from 'react';
import { IconButton } from './controls';
import './toggle-tip.css';

export function ToggleTip({ label, children }: { label: string; children: ReactNode }) {
  return (
    <Popover.Root>
      <Popover.Trigger asChild>
        <IconButton label={label} className="toggle-tip-trigger">
          <Info size={15} />
        </IconButton>
      </Popover.Trigger>
      <Popover.Portal>
        <Popover.Content
          className="toggle-tip"
          aria-label={label}
          side="top"
          align="start"
          sideOffset={6}
          collisionPadding={12}
        >
          {children}
          <Popover.Arrow className="toggle-tip-arrow" />
        </Popover.Content>
      </Popover.Portal>
    </Popover.Root>
  );
}
