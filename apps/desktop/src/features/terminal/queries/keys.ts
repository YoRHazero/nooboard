export const terminalKeys = {
  directories: ['terminal', 'directory'] as const,
  directory: (session: string, id: string, path?: string) =>
    ['terminal', 'directory', session, id, path ?? null] as const,
};
