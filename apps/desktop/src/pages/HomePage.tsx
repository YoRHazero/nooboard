import { HomeOverview, HomeStage } from '../features/home';
interface Props {
  onDevices(): void;
  onSettings(): void;
  onTransfers(key?: string): void;
  motionPreview?: boolean;
}
export function HomePage(props: Props) {
  return (
    <div className="home-page">
      <HomeOverview onDevices={props.onDevices} />
      <HomeStage {...props} />
    </div>
  );
}
