import type { PageText, PdfTextCharacter } from "./types";

export interface SelectableTextSpan extends PdfTextCharacter { start: number; end: number }
const spansCache = new WeakMap<PageText, SelectableTextSpan[]>();

/** Coincident reader rectangles share a hit target, not separate caret geometry.
 * Keep logical text and original character indices; never infer reading order.
 * The 0.02-point allowance covers measured PDF metric rounding. Compare every
 * member to the first rectangle, so the tolerance cannot grow along a line.
 */
export function selectableTextSpans(text: PageText): SelectableTextSpan[] {
  const cached = spansCache.get(text);
  if (cached) return cached;
  const spans: SelectableTextSpan[] = [];
  for (const [index, character] of text.characters.entries()) {
    const previous = spans[spans.length - 1];
    const first = previous && text.characters[previous.start];
    const visible = (c: PdfTextCharacter) => c.width > 0 && c.height > 0 && c.text.length > 0 && !/\s/u.test(c.text);
    if (first && visible(first) && visible(character)
      && [character.x - first.x, character.y - first.y,
        character.x + character.width - first.x - first.width,
        character.y + character.height - first.y - first.height].every(delta => Math.abs(delta) <= .02)) {
      const right = Math.max(previous.x + previous.width, character.x + character.width);
      const bottom = Math.max(previous.y + previous.height, character.y + character.height);
      previous.x = Math.min(previous.x, character.x); previous.y = Math.min(previous.y, character.y);
      previous.width = right - previous.x; previous.height = bottom - previous.y;
      previous.text += character.text; previous.end = index + 1;
    } else spans.push({ ...character, start: index, end: index + 1 });
  }
  spansCache.set(text, spans);
  return spans;
}

/** DOM offsets use UTF-16; copying half a supplementary scalar is never useful. */
export function selectedScalarText(value: string, start: number, end: number): string {
  if (end <= start) return "";
  const splitsPair = (offset: number) => offset > 0 && offset < value.length
    && value.charCodeAt(offset - 1) >= 0xd800 && value.charCodeAt(offset - 1) <= 0xdbff
    && value.charCodeAt(offset) >= 0xdc00 && value.charCodeAt(offset) <= 0xdfff;
  return value.slice(splitsPair(start) ? start - 1 : start, splitsPair(end) ? end + 1 : end);
}
