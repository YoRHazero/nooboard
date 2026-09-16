import { useEffect, useRef, useState } from 'react';
import type { StageTarget } from '../mascot/targets';
import { PanelInteraction, type OpenPanel } from './panelInteraction';

/** Bridges pointer/focus input to the interaction model; never controls the mascot. */
export function useStagePanel(onDismiss?: (panel: OpenPanel) => void) {
  const stage = useRef<HTMLElement>(null);
  const panel = useRef<HTMLElement>(null);
  const triggers = useRef<Partial<Record<StageTarget, HTMLButtonElement>>>({});
  const [open, setOpen] = useState<OpenPanel | null>(null);
  const dismissed = useRef(onDismiss);
  dismissed.current = onDismiss;
  const previous = useRef<OpenPanel | null>(null);
  const [interaction] = useState(
    () =>
      new PanelInteraction((next) => {
        const before = previous.current;
        previous.current = next;
        if (
          before?.mode === 'pinned' &&
          (before.target !== next?.target || next?.mode !== 'pinned')
        )
          dismissed.current?.(before);
        setOpen(next);
      }),
  );

  useEffect(() => {
    const move = (event: PointerEvent) => {
      if (event.pointerType !== 'mouse' || document.hidden) return;
      const element = event.target;
      if (!(element instanceof Element)) return;
      if (panel.current?.contains(element)) {
        interaction.move('panel');
        return;
      }
      const trigger = element.closest<HTMLButtonElement>('[data-stage-trigger]');
      const target =
        trigger && stage.current?.contains(trigger)
          ? (trigger.dataset.stageTrigger as StageTarget)
          : null;
      interaction.move(target);
    };
    const leave = (event: PointerEvent) => {
      if (!event.relatedTarget) interaction.move(null);
    };
    const blur = () => interaction.cancelPreview();
    const visibility = () => {
      if (document.hidden) blur();
    };
    document.addEventListener('pointermove', move);
    document.addEventListener('pointerout', leave);
    window.addEventListener('blur', blur);
    document.addEventListener('visibilitychange', visibility);
    return () => {
      if (previous.current?.mode === 'pinned') dismissed.current?.(previous.current);
      document.removeEventListener('pointermove', move);
      document.removeEventListener('pointerout', leave);
      window.removeEventListener('blur', blur);
      document.removeEventListener('visibilitychange', visibility);
    };
  }, [interaction]);

  const close = (restoreFocus = true) => {
    const current = interaction.getSnapshot();
    const shouldFocus =
      restoreFocus &&
      (current?.mode === 'pinned' || panel.current?.contains(document.activeElement));
    interaction.dismiss();
    if (current && shouldFocus) triggers.current[current.target]?.focus({ preventScroll: true });
  };
  return {
    stage,
    panel,
    triggers,
    open,
    close,
    activate: (target: StageTarget) => interaction.activate(target),
    pin: () => interaction.pin(),
  };
}
