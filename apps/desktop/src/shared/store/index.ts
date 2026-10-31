import { create } from 'zustand';
import { createFeedbackSlice, type FeedbackSlice } from './feedback.slice';
import { createConnectionSlice, type ConnectionSlice } from './connection.slice';
/** Frontend lifecycle and feedback only. Backend business data lives in Query. */
export const useApplicationStore = create<FeedbackSlice & ConnectionSlice>()((...args) => ({
  ...createFeedbackSlice(...args),
  ...createConnectionSlice(...args),
}));
export const useConnection = () => useApplicationStore((state) => state.connection);
export const useFeedback = () => useApplicationStore();
