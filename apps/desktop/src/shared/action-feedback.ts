import { toProblem } from './i18n/errors';
import { useApplicationStore } from './store';
interface OperationState {
  isPending: boolean;
  error: unknown;
  submittedAt?: number;
}
/** Handle UI event promises; request state stays owned by the supplied Query mutations. */
export async function handleAction<T>(action: () => Promise<T>): Promise<T | undefined> {
  useApplicationStore.getState().clearError();
  try {
    return await action();
  } catch (error) {
    useApplicationStore.getState().report(toProblem(error));
    return undefined;
  }
}
export function useActionFeedback(operations: readonly OperationState[] = []) {
  const latest = operations.reduce<OperationState | undefined>(
    (last, operation) =>
      !last || (operation.submittedAt ?? 0) >= (last.submittedAt ?? 0) ? operation : last,
    undefined,
  );
  const error = latest?.error;
  return {
    execute: handleAction,
    pending: operations.some((operation) => operation.isPending),
    error: error ? toProblem(error) : null,
  };
}
