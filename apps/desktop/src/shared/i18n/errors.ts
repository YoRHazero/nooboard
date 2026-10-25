import { t } from './index';
import { resources } from './resources';

export type ErrorCode = keyof typeof resources.en.errors | 'pairingCodeRemaining';
export interface Problem {
  code: ErrorCode;
  params?: Record<string, string | number>;
}
export const problem = (code: ErrorCode, params?: Problem['params']): Problem => ({ code, params });
export function errorText(error: Problem): string {
  return t(`errors:${error.code}`, error.params ?? {});
}
export class ProblemError extends Error {
  constructor(readonly problem: Problem) {
    super(errorText(problem));
    this.name = 'ProblemError';
  }
}
export const fail = (code: ErrorCode, params?: Problem['params']) =>
  new ProblemError(problem(code, params));
export function toProblem(reason: unknown): Problem {
  if (reason instanceof ProblemError) return reason.problem;
  if (
    reason &&
    typeof reason === 'object' &&
    'code' in reason &&
    typeof reason.code === 'string' &&
    (Object.hasOwn(resources.en.errors, reason.code) || reason.code === 'pairingCodeRemaining')
  ) {
    const params: Problem['params'] = {};
    if ('params' in reason && reason.params && typeof reason.params === 'object')
      for (const [key, value] of Object.entries(reason.params))
        if (typeof value === 'string' || typeof value === 'number') params[key] = value;
    return { code: reason.code as ErrorCode, params };
  }
  return problem('generic');
}
