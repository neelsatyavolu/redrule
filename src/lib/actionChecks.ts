/** The list on screen, and the list the last successful save stored. */
export interface CheckSave<T> {
  shown: T;
  saved: T;
}

/** Shows `shown` at once. The previous saved list stays until a save confirms it. */
export function beginCheck<T>(state: CheckSave<T>, shown: T): CheckSave<T> {
  return { saved: state.saved, shown };
}

/**
 * Applies one finished save. A success records that list even if a newer toggle is already on
 * screen. A failure puts the saved list back only when this attempt is still the one shown, so an
 * earlier request cannot wipe a later check.
 */
export function finishCheck<T>(state: CheckSave<T>, attempted: T, ok: boolean): CheckSave<T> {
  if (ok) return { shown: state.shown, saved: attempted };
  if (state.shown !== attempted) return state;
  return { shown: state.saved, saved: state.saved };
}
