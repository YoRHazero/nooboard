import { LocalDevicePanel, DevicesPanel } from '../features/devices';
export function DevicesPage() {
  return (
    <section className="devices-page">
      <LocalDevicePanel />
      <DevicesPanel />
    </section>
  );
}
