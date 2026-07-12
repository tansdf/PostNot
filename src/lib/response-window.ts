export function computeVirtualWindowStart(
  scrollTop: number,
  averageRowHeight: number,
  overscanRows: number
) {
  const visibleStart = Math.floor(Math.max(0, scrollTop) / Math.max(1, averageRowHeight));
  return Math.max(0, visibleStart - Math.max(0, overscanRows));
}

export function moveWrappedMatchIndex(current: number, delta: number, matchCount: number) {
  if (matchCount <= 0) return -1;
  return (current + delta + matchCount) % matchCount;
}

export function prepareRepresentationSwitch(activeSearchId: string, searchSequence: number) {
  return {
    searchIdToCancel: activeSearchId,
    nextSearchSequence: searchSequence + 1
  };
}
