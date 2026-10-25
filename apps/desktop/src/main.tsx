import { bootstrap } from './app/bootstrap';
import './app/theme.css';
import './app/layout.css';
import './shared/ui/controls.css';
import './pages/home/home.css';
import './pages/home/panels.css';
import './features/mascot/mascot.css';
import './features/mailbox/mailbox.css';
import './pages/history/history.css';
import './features/devices/devices.css';
import './features/transfers/transfers.css';
import './pages/settings/settings.css';
void bootstrap(document.getElementById('root')!).then((dispose) => {
  if (import.meta.hot) import.meta.hot.dispose(dispose);
});
