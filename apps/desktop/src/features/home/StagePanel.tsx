import { useI18n } from '../../i18n/react';
import { useEffect, useId, useLayoutEffect, useRef, type ReactNode, type RefObject } from 'react';
import { motion, useIsPresent, useReducedMotion } from 'motion/react';
import { Pin, X } from 'lucide-react';
import { IconButton } from '../../ui/controls';
import type { StageTarget } from '../mascot/targets';

/** Nonmodal panel shell; only explicit activation moves focus into its heading. */
export function StagePanel({
  target,
  title,
  onClose,
  panelRef: panel,
  mode,
  focusHeading,
  onPin,
  reducedMotion,
  children,
}: {
  target: StageTarget;
  title: string;
  onClose: (restoreFocus?: boolean) => void;
  panelRef: RefObject<HTMLElement | null>;
  mode: 'preview' | 'pinned';
  focusHeading: boolean;
  onPin: () => void;
  reducedMotion: boolean;
  children: ReactNode;
}) {
  const { t } = useI18n();
  const element = useRef<HTMLElement>(null);
  const heading = useRef<HTMLHeadingElement>(null);
  const titleId = useId();
  const isPresent = useIsPresent();
  const systemReduced = useReducedMotion();
  const reduce = reducedMotion || systemReduced;
  const close = useRef(onClose);
  // Exiting panels keep their own DOM ref, so they cannot clear the new active panel.
  useLayoutEffect(() => {
    if (!isPresent) return;
    const active = element.current;
    panel.current = active;
    return () => {
      if (panel.current === active) panel.current = null;
    };
  }, [isPresent, panel]);
  useEffect(() => {
    close.current = onClose;
  }, [onClose]);
  useEffect(() => {
    if (!isPresent) return;
    const outside = (event: PointerEvent) => {
      const element = event.target as Element;
      // Stage buttons handle the same outside click in activate(), without re-pinning.
      if (!panel.current?.contains(element) && !element.closest('[data-stage-trigger]'))
        close.current(false);
    };
    const escape = (event: KeyboardEvent) => {
      if (event.key === 'Escape' && !document.querySelector('dialog[open]')) {
        event.preventDefault();
        close.current(true);
      }
    };
    document.addEventListener('pointerdown', outside);
    document.addEventListener('keydown', escape);
    return () => {
      document.removeEventListener('pointerdown', outside);
      document.removeEventListener('keydown', escape);
    };
  }, [isPresent, panel]);
  useEffect(() => {
    if (isPresent && focusHeading) heading.current?.focus({ preventScroll: true });
  }, [isPresent, focusHeading]);
  const origin = {
    board: { x: -6, y: 6, rotate: -0.6, scale: 0.985 },
    bird: { x: -5, y: 2, rotate: 0, scale: 0.98 },
    mailbox: { x: 6, y: 6, rotate: 0.6, scale: 0.985 },
  }[target];
  return (
    <div
      className={`stage-panel-position stage-panel-position--${target}`}
      data-exiting={!isPresent || undefined}
    >
      <motion.section
        ref={element}
        id={isPresent ? 'stage-panel' : undefined}
        role="dialog"
        aria-labelledby={titleId}
        className={`stage-panel stage-panel--${target}`}
        data-panel-mode={mode}
        inert={!isPresent}
        aria-hidden={!isPresent || undefined}
        onClickCapture={(event) => {
          if (!(event.target as Element).closest('[data-panel-close]')) onPin();
        }}
        initial={{ opacity: 0, ...(reduce ? {} : origin) }}
        animate={{ opacity: 1, x: 0, y: 0, rotate: 0, scale: 1 }}
        exit={{ opacity: 0, ...(reduce ? {} : origin) }}
        transition={{ duration: reduce ? 0 : isPresent ? 0.12 : 0.09, ease: [0.22, 0.7, 0.3, 1] }}
      >
        {target === 'board' && (
          <>
            <img className="clipboard-mascot" src="/mascot/icon.png" alt="" />
            <span className="paper-clip" aria-hidden="true" />
          </>
        )}
        <div className="stage-panel__surface">
          <header className="stage-panel__header">
            <h2 ref={heading} tabIndex={-1} id={titleId}>
              {title}
            </h2>
            <div className="stage-panel__tools">
              <span
                className={`stage-panel__pin ${mode === 'pinned' ? 'is-pinned' : ''}`}
                role="img"
                aria-label={mode === 'pinned' ? t('home:pinned') : t('home:previewPin')}
                title={mode === 'pinned' ? t('home:unpinHint') : t('home:pinHint')}
              >
                <Pin size={13} />
              </span>
              <IconButton
                label={t('home:closePanel')}
                data-panel-close
                onClick={() => onClose(true)}
              >
                <X size={16} />
              </IconButton>
            </div>
          </header>
          {children}
        </div>
      </motion.section>
    </div>
  );
}
