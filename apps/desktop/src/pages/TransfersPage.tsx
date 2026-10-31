import { TransfersPanel } from '../features/transfers';

export function TransfersPage({ focusKey }: { focusKey?: string }) {
  return <TransfersPanel focusKey={focusKey} />;
}
