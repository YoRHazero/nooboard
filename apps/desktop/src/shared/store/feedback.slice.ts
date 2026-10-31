import type { StateCreator } from 'zustand';
import type { Problem } from '../i18n/errors';
export interface FeedbackSlice {
  error: Problem | null;
  report(error: Problem): void;
  clearError(): void;
}
export const createFeedbackSlice: StateCreator<FeedbackSlice, [], [], FeedbackSlice> = (set) => ({
  error: null,
  report: (error) => set({ error }),
  clearError: () => set({ error: null }),
});
