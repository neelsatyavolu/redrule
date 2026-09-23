import { toast } from "sonner";
import { create } from "zustand";
import { api, errorMessage } from "./api";
import type { Exchange } from "./types";

interface Thread {
  exchanges: Exchange[];
  /** The question waiting for an answer. */
  pending: string | null;
}

interface AskState {
  threads: Record<string, Thread>;
  ask: (meetingId: string, question: string) => Promise<boolean>;
  clear: (meetingId: string) => void;
}

const EMPTY: Thread = { exchanges: [], pending: null };

/** Questions and answers for each meeting, kept while the app is open. */
export const useAsk = create<AskState>((set, get) => {
  const change = (meetingId: string, patch: (thread: Thread) => Thread) =>
    set((state) => ({ threads: { ...state.threads, [meetingId]: patch(state.threads[meetingId] ?? EMPTY) } }));

  return {
    threads: {},
    ask: async (meetingId, question) => {
      const thread = get().threads[meetingId] ?? EMPTY;
      if (thread.pending) return false;
      change(meetingId, (current) => ({ ...current, pending: question }));
      try {
        const answer = await api.askMeeting(meetingId, question, thread.exchanges);
        change(meetingId, (current) => ({ exchanges: [...current.exchanges, { question, answer }], pending: null }));
        return true;
      } catch (error) {
        change(meetingId, (current) => ({ ...current, pending: null }));
        toast.error(errorMessage(error));
        return false;
      }
    },
    clear: (meetingId) => change(meetingId, (current) => ({ ...current, exchanges: [] })),
  };
});

export function useThread(meetingId: string): Thread {
  return useAsk((state) => state.threads[meetingId] ?? EMPTY);
}
