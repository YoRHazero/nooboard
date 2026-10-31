import { ConfigurationPanel, TerminalPanel, RemoteFilesPanel } from '../features/terminal';
export function TerminalPage() {
  return (
    <main className="terminal-page">
      <ConfigurationPanel />
      <div className="terminal-workbench">
        <TerminalPanel />
        <RemoteFilesPanel />
      </div>
    </main>
  );
}
