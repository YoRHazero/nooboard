import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useState,
  useSyncExternalStore,
  type ReactNode,
} from 'react';
import { useDesktop } from '../../desktop/api';
import type { PlaybackState } from '../mascot/playback';
import { ActivityInbox } from './ActivityInbox';

const MailboxContext = createContext<{
  inbox: ActivityInbox;
  onPlayback: (state: PlaybackState) => void;
} | null>(null);

export function MailboxProvider({ children }: { children: ReactNode }) {
  const client = useDesktop();
  const [inbox] = useState(() => new ActivityInbox(client.getSnapshot().activities[0] ?? null));
  useEffect(() => {
    const unsubscribe = client.subscribe(() => inbox.sync(client.getSnapshot().activities));
    const unlisten = client.onEvent((event) => {
      const activities = client.getSnapshot().activities;
      if (event.type === 'reset') inbox.reset(activities[0] ?? null);
      else if (event.type === 'copied' || event.type === 'sent' || event.type === 'received') {
        const activity = activities.find((row) => row.id === event.sequence);
        if (activity) inbox.record(activity);
      }
    });
    return () => {
      unsubscribe();
      unlisten();
    };
  }, [client, inbox]);
  const onPlayback = useCallback(
    (state: PlaybackState) => {
      const activity = client.getSnapshot().activities.find((row) => row.id === state.activityId);
      inbox.performance(state.active !== null, activity ?? null);
    },
    [client, inbox],
  );
  const value = useMemo(() => ({ inbox, onPlayback }), [inbox, onPlayback]);
  return <MailboxContext.Provider value={value}>{children}</MailboxContext.Provider>;
}

export function useMailbox() {
  const context = useContext(MailboxContext);
  if (!context) throw new Error('MailboxProvider is required');
  const state = useSyncExternalStore(context.inbox.subscribe, context.inbox.getSnapshot);
  return { ...context, ...state };
}
