import { useEffect, useState } from "react";
import { api } from "./api";
import { useStore } from "./store";

/** Waits for typing to pause before searching notes and transcripts. */
const DEBOUNCE_MS = 150;

/** Meetings whose notes or transcript match `query`, with the passage that matched; null while the box is empty. */
export function useSearchHits(query: string): ReadonlyMap<string, string | null> | null {
  // A changed meeting can change what matches, so search again when the library changes.
  const revision = useStore((s) => s.app?.revision ?? 0);
  const [hits, setHits] = useState<ReadonlyMap<string, string | null> | null>(null);
  const trimmed = query.trim();

  useEffect(() => {
    if (trimmed === "") {
      setHits(null);
      return;
    }
    let current = true;
    const timer = window.setTimeout(() => {
      api
        .searchMeetings(trimmed)
        .then((found) => current && setHits(new Map(found.map((hit) => [hit.id, hit.snippet]))))
        .catch(() => current && setHits(new Map()));
    }, DEBOUNCE_MS);
    return () => {
      current = false;
      window.clearTimeout(timer);
    };
  }, [trimmed, revision]);

  return trimmed === "" ? null : hits;
}
