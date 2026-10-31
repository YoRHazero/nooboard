import type { DesktopEvent, TerminalEnded } from './types';
const listeners = new Set<(event: DesktopEvent) => void>();
export function onBackendEvent(listener: (event: DesktopEvent) => void) {
  listeners.add(listener);
  return () => {
    listeners.delete(listener);
  };
}
export function publishBackendEvent(event: DesktopEvent) {
  listeners.forEach((listener) => listener(event));
}
const terminalListeners = new Set<(event: TerminalEnded) => void>();
export function onTerminalEnded(listener: (event: TerminalEnded) => void) {
  terminalListeners.add(listener);
  return () => {
    terminalListeners.delete(listener);
  };
}
export function publishTerminalEnded(event: TerminalEnded) {
  terminalListeners.forEach((listener) => listener(event));
}
