import { useEffect, useState } from "react";
import { api } from "./api";
import { applicableHits, type SearchResult } from "./searchHits";
import { useStore } from "./store";

/** Waits for typing to pause before searching notes and transcripts. */
const DEBOUNCE_MS = 150;

/** Meetings whose notes or transcript match `query`, with the passage that matched; null while the box is empty. */
export function useSearchHits(query: string): ReadonlyMap<string, string | null> | null {
  // A changed meeting can change what matches, so search again when the library changes.
  const revision = useStore((s) => s.app?.revision ?? 0);
  const [result, setResult] = useState<SearchResult | null>(null);
  const trimmed = query.trim();

  useEffect(() => {
    if (trimmed === "") {
      setResult(null);
      return;
    }
    let current = true;
    const timer = window.setTimeout(() => {
      api
        .searchMeetings(trimmed)
        .then((found) => current && setResult({ query: trimmed, hits: new Map(found.map((hit) => [hit.id, hit.snippet])) }))
        .catch(() => current && setResult({ query: trimmed, hits: new Map() }));
    }, DEBOUNCE_MS);
    return () => {
      current = false;
      window.clearTimeout(timer);
    };
  }, [trimmed, revision]);

  return applicableHits(query, result);
}
