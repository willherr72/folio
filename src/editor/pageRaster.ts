// This owns temporary decode resources, never the caller's object URL.
const MAX_ENCODED_BYTES = 32 * 1024 * 1024;
const MAX_AXIS = 16384;
const MAX_PIXELS = 16_777_216;

export type RasterPaintState = { url?: string; kind: "empty" | "loading" | "canvas" | "fallback" };

async function boundedPng(url: string, signal: AbortSignal) {
  const response = await fetch(url, { signal });
  if (!response.ok || !response.body) throw new Error("Raster unavailable");
  const declared = response.headers.get("content-length");
  if (declared !== null && (!/^\d+$/.test(declared) || Number(declared) > MAX_ENCODED_BYTES)) {
    await response.body.cancel();
    throw new Error("Raster exceeds decode budget");
  }
  const reader = response.body.getReader();
  const chunks: ArrayBuffer[] = [];
  let size = 0, complete = false;
  try {
    while (true) {
      signal.throwIfAborted();
      const chunk = await reader.read();
      if (chunk.done) { complete = true; break; }
      size += chunk.value.byteLength;
      if (size > MAX_ENCODED_BYTES) throw new Error("Raster exceeds decode budget");
      // Own only the returned bytes, not a possibly larger/shared backing view.
      chunks.push(new Uint8Array(chunk.value).buffer);
    }
  } finally {
    if (!complete) await reader.cancel().catch(() => {});
    reader.releaseLock();
  }
  signal.throwIfAborted();
  const blob = new Blob(chunks, { type: "image/png" });
  const header = new Uint8Array(await blob.slice(0, 33).arrayBuffer());
  const prefix = [137,80,78,71,13,10,26,10,0,0,0,13,73,72,68,82];
  if (header.length !== 33 || prefix.some((byte, i) => header[i] !== byte)) throw new Error("Not a page PNG");
  const view = new DataView(header.buffer, header.byteOffset, header.byteLength);
  const width = view.getUint32(16), height = view.getUint32(20);
  if (!width || !height || width > MAX_AXIS || height > MAX_AXIS || width * height > MAX_PIXELS) {
    throw new Error("Raster exceeds surface budget");
  }
  return { blob, width, height };
}

/** Latest-only, serial decoding: a superseded createImageBitmap cannot be aborted,
 * so its result is closed before the next request starts. */
export function createRasterPainter(canvas: HTMLCanvasElement, notify: (state: RasterPaintState) => void) {
  let disposed = false, running = false, generation = 0;
  let source: string | undefined, initialized = false;
  let abort: AbortController | undefined;
  let context: CanvasRenderingContext2D | null = null;
  try { context = canvas.getContext("2d", { alpha: true, willReadFrequently: true }); } catch { /* SVG fallback. */ }

  async function pump() {
    if (running || disposed || !source) return;
    const url = source, request = generation;
    if (!context || typeof createImageBitmap !== "function" || !url.startsWith("blob:")) {
      notify({ url, kind: "fallback" });
      return;
    }
    running = true;
    const controller = new AbortController();
    abort = controller;
    let bitmap: ImageBitmap | undefined;
    try {
      const png = await boundedPng(url, controller.signal);
      controller.signal.throwIfAborted();
      bitmap = await createImageBitmap(png.blob);
      if (disposed || generation !== request) return;
      if (bitmap.width !== png.width || bitmap.height !== png.height) throw new Error("Unexpected raster size");
      // Attribute assignments resize immediately. A tall-to-wide transition must
      // not allocate new width * old height before the second assignment.
      if (bitmap.width * canvas.height > MAX_PIXELS) canvas.height = 0;
      if (canvas.width !== bitmap.width) canvas.width = bitmap.width;
      if (canvas.height !== bitmap.height) canvas.height = bitmap.height;
      context.clearRect(0, 0, canvas.width, canvas.height);
      context.drawImage(bitmap, 0, 0);
      notify({ url, kind: "canvas" });
    } catch {
      if (!disposed && generation === request) notify({ url, kind: "fallback" });
    } finally {
      bitmap?.close();
      running = false;
      if (abort === controller) abort = undefined;
      if (!disposed && generation !== request) void pump();
    }
  }
  return {
    setSource(url?: string) {
      if (disposed || (initialized && source === url)) return;
      initialized = true;
      source = url;
      generation++;
      abort?.abort();
      notify({ url, kind: url ? "loading" : "empty" });
      if (!url) { canvas.width = 0; canvas.height = 0; }
      else void pump();
    },
    dispose() {
      disposed = true;
      generation++;
      abort?.abort();
      canvas.width = 0;
      canvas.height = 0;
    },
  };
}
