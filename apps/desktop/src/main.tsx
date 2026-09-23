import { bootstrap } from './app/bootstrap';
import './app/theme.css';
import './app/layout.css';
import './ui/controls.css';
import './features/home/home.css';
import './features/home/panels.css';
import './features/mascot/mascot.css';
import './features/mailbox/mailbox.css';
import './features/history/history.css';
import './features/devices/devices.css';
import './features/transfers/transfers.css';
import './features/settings/settings.css';
void bootstrap(document.getElementById('root')!).then((dispose) => {
  if (import.meta.hot) import.meta.hot.dispose(dispose);
});
