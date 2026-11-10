/** Textareas expose LF-only offsets. Preserve source delimiters outside the edit. */
export function applyTextAreaEdit(original: string, edited: string): string {
  if (!original.includes("\r")) return edited;
  const normalized = original.replace(/\r\n?/g, "\n");
  if (normalized === edited) return original;
  let prefix = 0, suffix = 0;
  while (prefix < normalized.length && prefix < edited.length && normalized[prefix] === edited[prefix]) prefix++;
  while (suffix < normalized.length - prefix && suffix < edited.length - prefix && normalized[normalized.length - suffix - 1] === edited[edited.length - suffix - 1]) suffix++;
  const sourceOffset = (offset: number) => {
    let index = 0;
    for (let count = 0; count < offset; count++) {
      index += original[index] === "\r" && original[index + 1] === "\n" ? 2 : 1;
    }
    return index;
  };
  const before = original.slice(0, sourceOffset(prefix));
  const after = original.slice(sourceOffset(normalized.length - suffix));
  // New breaks inherit the nearest surrounding paragraph's convention.
  const delimiter = before.match(/\r\n|\r|\n/g)?.at(-1) ?? after.match(/\r\n|\r|\n/)?.[0] ?? "\n";
  return before + edited.slice(prefix, edited.length - suffix).replace(/\n/g, delimiter) + after;
}
