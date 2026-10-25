import { createFileRoute, useNavigate } from '@tanstack/react-router';
import { HomePage } from '../../pages/home/HomePage';
export const Route = createFileRoute('/')({ component: Home });
function Home() {
  const { motionPreview } = Route.useRouteContext();
  const navigate = useNavigate();
  return (
    <HomePage
      motionPreview={motionPreview}
      onDevices={() => void navigate({ to: '/devices' })}
      onSettings={() => void navigate({ to: '/settings' })}
      onTransfers={(focus) => void navigate({ to: '/transfers', search: { focus } })}
    />
  );
}
