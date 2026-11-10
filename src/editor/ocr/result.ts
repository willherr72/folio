import type {OcrBlock, OcrLine, OcrPage, OcrParagraph, OcrRaster, OcrWord} from './types';

const invalid = (reason: string) => new Error(`Invalid OCR data: ${reason}.`);
function dimensions(width: number, height: number): void {
  if (!Number.isInteger(width) || !Number.isInteger(height) || width < 1 || height < 1
    || width > 6000 || height > 6000 || width * height > 20_000_000) {
    throw invalid('raster dimensions exceed the supported limit');
  }
}
export function validateOcrRaster(raster: OcrRaster): void {
  dimensions(raster.width, raster.height);
  if (!Number.isFinite(raster.dpi) || raster.dpi < 72 || raster.dpi > 600) throw invalid('DPI must be between72 and600');
  const bytes = raster.png;
  if (!(bytes instanceof Uint8Array) || bytes.length < 33 || bytes.length > 32 * 1024 * 1024) throw invalid('PNG byte limit');
  const signature = [137,80,78,71,13,10,26,10];
  if (signature.some((byte,index) => bytes[index] !== byte)) throw invalid('a PNG raster is required');
  const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  if (view.getUint32(8) !== 13 || view.getUint32(12) !== 0x49484452
    || view.getUint32(16) !== raster.width || view.getUint32(20) !== raster.height) throw invalid('PNG dimensions do not match the raster');
}
function object(value: unknown): Record<string, unknown> {
  if (!value || typeof value !== 'object' || Array.isArray(value)) throw invalid('expected a structured result');
  return value as Record<string, unknown>;
}
function array(value: unknown): unknown[] {
  if (!Array.isArray(value)) throw invalid('expected a result list');
  return value;
}
function finite(value: unknown): number {
  if (typeof value !== 'number' || !Number.isFinite(value)) throw invalid('non-finite geometry or confidence');
  return value;
}

/** Discard rich engine-only data, but never silently discard an invalid recognized word. */
export function normalizeOcrResult(raw: unknown, width: number, height: number): OcrPage {
  return normalize(raw,width,height,false);
}
/** Revalidate corrections and native owned records, retaining only bounded plain data. */
export function copyOcrPage(raw:unknown):OcrPage {
  const page=object(raw);
  return normalize(raw,finite(page.width),finite(page.height),true);
}
function normalize(raw: unknown, width: number, height: number, native:boolean): OcrPage {
  dimensions(width,height);
  const result = object(raw), output: OcrPage = {width,height,blocks:[]};
  if (result.blocks === null) {
    if (typeof result.text === 'string' && result.text.trim()) throw invalid('text has no word geometry');
    return output;
  }
  let words = 0, lines = 0, containers = 0, scalars = 0, textBytes = 0;
  const encoder = new TextEncoder();
  function list(value: unknown): unknown[] {
    const items = array(value);
    containers += items.length;
    if (containers > 30_000) throw invalid('result container limit');
    return items;
  }
  function normalizeWord(value: unknown): OcrWord {
    const item = object(value), text = item.text;
    if (typeof text !== 'string' || !text.trim() || /[\u0000-\u001f\u007f-\u009f]/u.test(text)) throw invalid('empty or control text');
    // Reject malformed UTF-16 instead of encoding replacement characters into a PDF.
    for (const scalar of text) {
      const code = scalar.codePointAt(0)!;
      if (code >= 0xd800 && code <= 0xdfff) throw invalid('unpaired Unicode surrogate');
      if (++scalars > 100_000) throw invalid('Unicode scalar limit');
    }
    textBytes += encoder.encode(text).length;
    if (textBytes > 2 * 1024 * 1024) throw invalid('UTF-8 text limit');
    const confidence = finite(item.confidence), box = object(native?item.bounds:item.bbox);
    const x = finite(native?box.x:box.x0), y = finite(native?box.y:box.y0);
    const right=native?x+finite(box.width):finite(box.x1),bottom=native?y+finite(box.height):finite(box.y1);
    if (confidence < 0 || confidence > 100 || x < 0 || y < 0 || right > width || bottom > height
      || right <= x || bottom <= y) throw invalid('word bounds or confidence are outside the raster');
    return {text,confidence,bounds:{x,y,width:right-x,height:bottom-y}};
  }
  for (const rawBlock of list(result.blocks)) {
    const block: OcrBlock = {paragraphs:[]};
    for (const rawParagraph of list(object(rawBlock).paragraphs)) {
      const paragraph: OcrParagraph = {lines:[]};
      const lineItems = list(object(rawParagraph).lines);
      lines += lineItems.length;
      if (lines > 10_000) throw invalid('line limit');
      for (const rawLine of lineItems) {
        const wordItems = array(object(rawLine).words);
        words += wordItems.length;
        if (words > 20_000) throw invalid('word limit');
        const line: OcrLine = {words:wordItems.map(normalizeWord)};
        if (line.words.length) paragraph.lines.push(line);
      }
      if (paragraph.lines.length) block.paragraphs.push(paragraph);
    }
    if (block.paragraphs.length) output.blocks.push(block);
  }
  return output;
}
