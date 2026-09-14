/** UTF-16 selection for a native deletion; never normalize or reorder logical text. */
export function graphemeDeletionRange(text: string, start: number, end: number, direction: "backward" | "forward"): [number, number] | null {
  const segmenter = new Intl.Segmenter(undefined, { granularity: "grapheme" });
  let from = start, to = end;
  for (const { index, segment } of segmenter.segment(text)) {
    const limit = index + segment.length;
    if (start === end) {
      if ((index < start && start < limit) ||
          (direction === "backward" && limit === start) ||
          (direction === "forward" && index === start)) return [index, limit];
    } else {
      if (index < start && start < limit) from = index;
      if (index < end && end < limit) to = limit;
    }
  }
  return start === end ? null : [from, to];
}
