import { createFileRoute } from '@tanstack/react-router';
import { HistoryPage } from '../../pages/history/HistoryPage';
export const Route = createFileRoute('/history')({ component: HistoryPage });
