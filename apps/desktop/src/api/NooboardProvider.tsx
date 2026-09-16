import { type Problem, toProblem } from '../i18n/errors';
import { createContext, useContext, useState, useSyncExternalStore, type ReactNode } from 'react';
import type { DesktopClient } from './contracts';

const ClientContext = createContext<DesktopClient | null>(null);
const FeedbackContext = createContext<{
  error: Problem | null;
  clearError: () => void;
  execute: (operation: () => Promise<unknown>) => void;
} | null>(null);

export function NooboardProvider({
  client,
  children,
}: {
  client: DesktopClient;
  children: ReactNode;
}) {
  const [error, setError] = useState<Problem | null>(null);
  const execute = (operation: () => Promise<unknown>) => {
    setError(null);
    void operation().catch((reason: unknown) => setError(toProblem(reason)));
  };
  return (
    <ClientContext.Provider value={client}>
      <FeedbackContext.Provider value={{ error, clearError: () => setError(null), execute }}>
        {children}
      </FeedbackContext.Provider>
    </ClientContext.Provider>
  );
}

export function useClient() {
  const client = useContext(ClientContext);
  if (!client) throw new Error('NooboardProvider is required');
  return client;
}

export function useSnapshot() {
  const client = useClient();
  return useSyncExternalStore(client.subscribe, client.getSnapshot);
}

export function useCommand() {
  const feedback = useContext(FeedbackContext);
  if (!feedback) throw new Error('NooboardProvider is required');
  return feedback;
}
