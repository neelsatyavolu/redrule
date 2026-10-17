/** Hits from a notes-and-transcript search, kept only for the query that produced them. */
const EMPTY_HITS: ReadonlyMap<string, string | null> = new Map();

export interface SearchResult {
  query: string;
  hits: ReadonlyMap<string, string | null>;
}

/**
 * Title matching can run immediately. Passage hits from an earlier query must not keep meetings
 * in the list, or show the old snippet, while the next search is still running.
 */
export function applicableHits(query: string, result: SearchResult | null): ReadonlyMap<string, string | null> | null {
  const trimmed = query.trim();
  if (trimmed === "") return null;
  return result?.query === trimmed ? result.hits : EMPTY_HITS;
}
