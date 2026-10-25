import { createFileRoute } from '@tanstack/react-router';
import { DevicesPage } from '../../pages/devices/DevicesPage';
export const Route = createFileRoute('/devices')({ component: DevicesPage });
