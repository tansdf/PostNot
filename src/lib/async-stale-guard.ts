/**
 * Coordinates overlapping async work (e.g. route-driven loads) so only the latest
 * completion applies UI state.
 */
export function createStaleGuard() {
  let latest = 0;

  return {
    next: (): number => {
      latest += 1;
      return latest;
    },
    isStale: (seq: number): boolean => seq !== latest
  };
}
