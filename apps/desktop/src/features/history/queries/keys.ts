export const historyKeys = {
  all: ['history'] as const,
  list: (session: string, contains: string, source: string) =>
    ['history', session, contains, source] as const,
};
