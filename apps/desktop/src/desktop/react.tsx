import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useRef,
  useState,
  useSyncExternalStore,
  type ReactNode,
} from 'react';
import type { Desktop } from './api';
import type { DesktopSnapshot } from './snapshot/model';
import { toProblem, type Problem } from '../i18n/errors';
const DesktopContext = createContext<Desktop | null>(null);
const FeedbackContext = createContext<{
  error: Problem | null;
  clearError(): void;
  report(error: Problem): void;
} | null>(null);
export function DesktopProvider({ desktop, children }: { desktop: Desktop; children: ReactNode }) {
  const [error, setError] = useState<Problem | null>(null);
  const clearError = useCallback(() => setError(null), []);
  const feedback = useMemo(() => ({ error, clearError, report: setError }), [error, clearError]);
  return (
    <DesktopContext.Provider value={desktop}>
      <FeedbackContext.Provider value={feedback}>{children}</FeedbackContext.Provider>
    </DesktopContext.Provider>
  );
}
export function useDesktop() {
  const desktop = useContext(DesktopContext);
  if (!desktop) throw new Error('DesktopProvider is required');
  return desktop;
}
const identity = (snapshot: DesktopSnapshot) => snapshot;
export function useSnapshot<T = DesktopSnapshot>(
  selector: (snapshot: DesktopSnapshot) => T = identity as (snapshot: DesktopSnapshot) => T,
): T {
  const desktop = useDesktop();
  // Store branches are structurally shared; selectors should return a branch or a primitive.
  const read = useCallback(() => selector(desktop.getSnapshot()), [desktop, selector]);
  return useSyncExternalStore(desktop.subscribe, read);
}
export function useConnection() {
  const desktop = useDesktop();
  return useSyncExternalStore(desktop.subscribeConnection, desktop.getConnection);
}
export function usePreferences() {
  const desktop = useDesktop();
  return useSyncExternalStore(desktop.subscribePreferences, desktop.getPreferences);
}
export function useFeedback() {
  const feedback = useContext(FeedbackContext);
  if (!feedback) throw new Error('DesktopProvider is required');
  return feedback;
}
/** Pending state belongs to the caller; failures are also shown by the application shell. */
export function useCommand() {
  const { report, clearError } = useFeedback();
  const [pending, setPending] = useState(0);
  const [error, setError] = useState<Problem | null>(null);
  const mounted = useRef(false);
  useEffect(() => {
    mounted.current = true;
    return () => {
      mounted.current = false;
    };
  }, []);
  const execute = useCallback(
    async <T,>(operation: () => Promise<T>): Promise<T | undefined> => {
      clearError();
      setError(null);
      setPending((v) => v + 1);
      try {
        return await operation();
      } catch (reason) {
        if (mounted.current) {
          const problem = toProblem(reason);
          setError(problem);
          report(problem);
        }
        return undefined;
      } finally {
        if (mounted.current) setPending((v) => v - 1);
      }
    },
    [report, clearError],
  );
  return { execute, pending: pending > 0, error };
}
