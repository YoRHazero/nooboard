import type { TerminalTarget } from '../../shared/types';
export const terminalKey = (target: TerminalTarget) => `${target.kind}:${target.id}`;
