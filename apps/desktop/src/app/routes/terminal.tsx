import { createFileRoute } from '@tanstack/react-router';
import { TerminalPage } from '../../pages/TerminalPage';
export const Route = createFileRoute('/terminal')({ component: TerminalPage });
