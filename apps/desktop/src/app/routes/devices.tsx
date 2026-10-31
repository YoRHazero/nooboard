import { createFileRoute } from '@tanstack/react-router';
import { DevicesPage } from '../../pages/DevicesPage';
export const Route = createFileRoute('/devices')({ component: DevicesPage });
