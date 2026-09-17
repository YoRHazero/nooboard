import { useI18n } from '../../i18n/react';
import { useState } from 'react';
import { ArrowUpRight } from 'lucide-react';
import { AnimatePresence } from 'motion/react';
import { useSnapshot } from '../../api/NooboardProvider';
import { Mascot } from '../mascot/Mascot';
import { posterTargets } from '../mascot/poster';
import type { StageTarget, TargetLayout } from '../mascot/targets';
import { StagePanel } from './StagePanel';
import { ClipboardPanel } from './ClipboardPanel';
import { LocalPanel } from './LocalPanel';
import { MailboxPanel } from '../mailbox/MailboxPanel';
import { MailboxBadge } from '../mailbox/MailboxBadge';
import { useMailbox } from '../mailbox/MailboxProvider';
import { useStagePanel } from './useStagePanel';

// Accessible controls remain available when the renderer cannot load.
const fallback = posterTargets(1200, 700);

export function HomeStage({
  onDevices,
  onSettings,
  onTransfers,
}: {
  onDevices: () => void;
  onSettings: () => void;
  onTransfers: (key?: string) => void;
}) {
  const { t } = useI18n();
  const { localDevice, settings } = useSnapshot();
  const [targets, setTargets] = useState<TargetLayout>(fallback);
  const mailbox = useMailbox();
  const panels = useStagePanel((panel) => {
    if (panel.target === 'mailbox') mailbox.inbox.acknowledge();
  });
  const selected = panels.open?.target;
  const labels = {
    board: {
      title: t('home:currentClipboard'),
      hint: t('home:viewText'),
      aria: t('home:boardLabel'),
    },
    bird: {
      title: localDevice.deviceName,
      hint: t('home:localControls'),
      aria: t('home:birdLabel'),
    },
    mailbox: {
      title: t('home:mailbox'),
      hint: t('home:mailboxHint'),
      aria: t('home:mailboxLabel'),
    },
  };
  return (
    <section ref={panels.stage} className="home-stage" aria-label={t('home:stageLabel')}>
      <div className="home-stage__scene">
        <Mascot onLayout={setTargets} onPlayback={mailbox.onPlayback} />
        {(Object.keys(labels) as StageTarget[]).map((target) => {
          const rect = targets[target];
          const label = labels[target];
          return (
            <button
              key={target}
              ref={(element) => {
                if (element) panels.triggers.current[target] = element;
                else delete panels.triggers.current[target];
              }}
              type="button"
              data-stage-trigger={target}
              className={`stage-target stage-target--${target} ${selected === target ? 'is-selected' : ''}`}
              style={{
                left: `${rect.left}%`,
                top: `${rect.top}%`,
                width: `${rect.width}%`,
                height: `${rect.height}%`,
              }}
              aria-label={label.aria}
              aria-haspopup="dialog"
              aria-expanded={selected === target}
              aria-controls={selected === target ? 'stage-panel' : undefined}
              onClick={() => panels.activate(target)}
            >
              {target === 'mailbox' && (
                <MailboxBadge count={mailbox.batch.length} reducedMotion={settings.reducedMotion} />
              )}
              <span className="stage-target__label">
                <strong>
                  {label.title}
                  <ArrowUpRight size={12} />
                </strong>
                <small>{label.hint}</small>
              </span>
            </button>
          );
        })}
      </div>
      <AnimatePresence mode="sync">
        {selected && panels.open && (
          <StagePanel
            key={selected}
            target={selected}
            title={
              selected === 'board'
                ? t('home:currentClipboard')
                : selected === 'bird'
                  ? t('home:syncControls')
                  : t('home:mailbox')
            }
            onClose={panels.close}
            panelRef={panels.panel}
            mode={panels.open.mode}
            focusHeading={panels.open.focusHeading}
            onPin={panels.pin}
            reducedMotion={settings.reducedMotion}
          >
            {selected === 'board' && <ClipboardPanel />}
            {selected === 'bird' && <LocalPanel onSettings={onSettings} />}
            {selected === 'mailbox' && (
              <MailboxPanel
                onTransfers={(key) => {
                  panels.close(false);
                  onTransfers(key);
                }}
                onDevices={() => {
                  panels.close(false);
                  onDevices();
                }}
              />
            )}
          </StagePanel>
        )}
      </AnimatePresence>
    </section>
  );
}
