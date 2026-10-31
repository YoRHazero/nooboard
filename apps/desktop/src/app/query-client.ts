import { MutationCache, QueryClient } from '@tanstack/react-query';
import { toProblem } from '../shared/i18n/errors';
import { useApplicationStore } from '../shared/store';
export function createQueryClient() {
  const client = new QueryClient({
    mutationCache: new MutationCache({
      onError: (error) => useApplicationStore.getState().report(toProblem(error)),
    }),
    defaultOptions: {
      queries: {
        networkMode: 'always',
        retry: false,
        staleTime: 15_000,
        refetchOnWindowFocus: false,
      },
      mutations: { networkMode: 'always', retry: false, gcTime: 0 },
    },
  });
  // Register before seeding: push-only data must survive periods without mounted observers.
  client.setQueryDefaults(['backend'], { staleTime: Infinity, gcTime: Infinity, enabled: false });
  return client;
}
