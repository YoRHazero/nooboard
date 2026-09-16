import { useI18n } from '../../i18n/react';
import { motion, useReducedMotion } from 'motion/react';

export function MailboxBadge({ count, reducedMotion }: { count: number; reducedMotion: boolean }) {
  const systemReduced = useReducedMotion();
  const { t } = useI18n();
  if (!count) return null;
  return (
    <span className="mailbox-badge" role="status" aria-label={t('home:batchCount', { count })}>
      <motion.span
        key={count}
        initial={reducedMotion || systemReduced ? false : { scale: 0.85 }}
        animate={{ scale: 1 }}
        transition={{ duration: 0.14 }}
        aria-hidden="true"
      >
        {count > 99 ? '99+' : count}
      </motion.span>
    </span>
  );
}
