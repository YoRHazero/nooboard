import { useSnapshot } from '../../api/NooboardProvider';
import { useMailbox } from './MailboxProvider';
import { ActivityList } from './ActivityList';
import { PairedDevices } from './PairedDevices';
import { Postmark } from './Postmark';

export function MailboxPanel({ onDevices }: { onDevices: () => void }) {
  const { latest, batch } = useMailbox();
  const { peers } = useSnapshot();
  const activities = batch.length ? batch : latest ? [latest] : [];
  return (
    <div className="mailbox-content">
      <Postmark />
      <ActivityList activities={activities} grouped={batch.length > 0} />
      <PairedDevices devices={peers} onManage={onDevices} />
    </div>
  );
}
