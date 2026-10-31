import { onBackendEvent } from '../../shared/events';
import { getBackendSnapshot, subscribeBackendSnapshot } from '../../shared/backend';
import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useState,
  type ReactNode,
} from 'react';
import type { PlaybackState } from '../mascot/index';
import { useStore } from 'zustand';
import { createInboxStore, type InboxStore } from './store';

const MailboxContext = createContext<{
  inbox: InboxStore;
  onPlayback: (state: PlaybackState) => void;
} | null>(null);

export function MailboxProvider({ children }: { children: ReactNode }) {
  const [inbox] = useState(() => createInboxStore(getBackendSnapshot().activities[0] ?? null));
  useEffect(() => {
    const unsubscribe = subscribeBackendSnapshot(() =>
      inbox.getState().sync(getBackendSnapshot().activities),
    );
    const unlisten = onBackendEvent((event) => {
      const activities = getBackendSnapshot().activities;
      if (event.type === 'reset') inbox.getState().reset(activities[0] ?? null);
      else if (
        event.type === 'copied' ||
        event.type === 'sent' ||
        event.type === 'received' ||
        event.contentNode === 'finished'
      ) {
        const activity = activities.find((row) => row.id === event.sequence);
        if (activity) inbox.getState().record(activity);
      }
    });
    return () => {
      unsubscribe();
      unlisten();
    };
  }, [inbox]);
  const onPlayback = useCallback(
    (state: PlaybackState) => {
      const activity = getBackendSnapshot().activities.find((row) => row.id === state.activityId);
      inbox.getState().performance(state.activityId !== null, activity ?? null);
    },
    [inbox],
  );
  const value = useMemo(() => ({ inbox, onPlayback }), [inbox, onPlayback]);
  return <MailboxContext.Provider value={value}>{children}</MailboxContext.Provider>;
}

export function useMailbox() {
  const context = useContext(MailboxContext);
  if (!context) throw new Error('MailboxProvider is required');
  const state = useStore(context.inbox);
  return { ...context, ...state };
}
