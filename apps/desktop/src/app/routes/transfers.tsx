import { createFileRoute } from '@tanstack/react-router';
import { TransfersPage } from '../../pages/transfers/TransfersPage';
export const Route = createFileRoute('/transfers')({
  validateSearch: (search: Record<string, unknown>): { focus?: string } => ({
    focus: typeof search.focus === 'string' ? search.focus : undefined,
  }),
  component: Transfers,
});
function Transfers() {
  const { focus } = Route.useSearch();
  return <TransfersPage focusKey={focus} />;
}
