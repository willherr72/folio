import { cloneOverlay } from "./model";
import { invoke } from "@tauri-apps/api/core";
import type { DocumentInfo, PagePlan, PageText } from "./types";

export interface FolioAdapter {
  kind: "native" | "demo";
  openPdf(): Promise<DocumentInfo | null>;
  renderPage(sourceId: string, pageIndex: number, width: number): Promise<string>;
  getPageText?(sourceId: string, pageIndex: number): Promise<PageText>;
  exportPdf(pages: PagePlan[]): Promise<string | null>;
  closeDocument(sourceId: string): Promise<void>;
  engineStatus(): Promise<string>;
}

function bytesToUrl(value: ArrayBuffer | Uint8Array | number[]): string {
  const bytes = value instanceof ArrayBuffer
    ? new Uint8Array(value)
    : value instanceof Uint8Array ? value : new Uint8Array(value);
  const copy = new Uint8Array(bytes.byteLength);
  copy.set(bytes);
  return URL.createObjectURL(new Blob([copy.buffer], { type: "image/png" }));
}

export const nativeAdapter: FolioAdapter = {
  kind: "native",
  openPdf: () => invoke<DocumentInfo | null>("open_pdf"),
  async renderPage(sourceId, pageIndex, width) {
    const bytes = await invoke<ArrayBuffer | Uint8Array | number[]>("render_page", {
      sourceId,
      pageIndex,
      width: Math.max(64, Math.min(2400, Math.round(width))),
    });
    return bytesToUrl(bytes);
  },
  getPageText: (sourceId, pageIndex) => invoke<PageText>("page_text", { sourceId, pageIndex }),
  exportPdf: (pages) => invoke<string | null>("export_pdf", { request: { pages } }),
  closeDocument: (sourceId) => invoke<void>("close_document", { sourceId }),
  engineStatus: () => invoke<string>("engine_status"),
};

const DEMO_PAGES = [
  { width: 612, height: 792 },
  { width: 612, height: 792 },
  { width: 792, height: 612 },
];

function demoSvg(pageIndex: number): string {
  const page = DEMO_PAGES[pageIndex] ?? DEMO_PAGES[0];
  const colors = ["#C76D4B", "#64746B", "#B68A56"];
  const titles = ["Welcome to Folio", "Shape the details", "Export with confidence"];
  const subtitles = [
    "A calmer way to finish everyday PDFs.",
    "Add text, place a signature, and arrange pages.",
    "Your source stays untouched. Your work stays local.",
  ];
  const landscape = page.width > page.height;
  const cardWidth = landscape ? 220 : 430;
  const cards = landscape ? 3 : 1;
  const blocks = Array.from({ length: cards }, (_, index) => {
    const x = landscape ? 74 + index * 240 : 74;
    const y = landscape ? 280 : 355 + index * 90;
    return `<rect x="${x}" y="${y}" width="${cardWidth}" height="150" rx="8" fill="#F5F1E9"/><rect x="${x + 24}" y="${y + 26}" width="72" height="8" rx="4" fill="${colors[pageIndex]}" opacity=".8"/><rect x="${x + 24}" y="${y + 53}" width="${cardWidth - 48}" height="5" rx="2.5" fill="#C9C2B7"/><rect x="${x + 24}" y="${y + 72}" width="${cardWidth - 78}" height="5" rx="2.5" fill="#DDD7CE"/>`;
  }).join("");
  const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="${page.width}" height="${page.height}" viewBox="0 0 ${page.width} ${page.height}"><rect width="100%" height="100%" fill="#FFFEFB"/><rect x="0" y="0" width="16" height="100%" fill="${colors[pageIndex]}"/><text x="74" y="92" fill="#2D2A26" font-family="Arial,sans-serif" font-size="13" letter-spacing="2">FOLIO · ${String(pageIndex + 1).padStart(2, "0")}</text><text x="74" y="180" fill="#2D2A26" font-family="Georgia,serif" font-size="${landscape ? 44 : 42}">${titles[pageIndex]}</text><text x="74" y="228" fill="#746F68" font-family="Arial,sans-serif" font-size="18">${subtitles[pageIndex]}</text>${blocks}<text x="74" y="${page.height - 54}" fill="#AAA39A" font-family="Arial,sans-serif" font-size="11">PRIVATE BY DESIGN · LOCAL ON YOUR DEVICE</text></svg>`;
  return `data:image/svg+xml;charset=utf-8,${encodeURIComponent(svg)}`;
}

function demoPageText(pageIndex: number): PageText {
  const page = DEMO_PAGES[pageIndex] ?? DEMO_PAGES[0];
  const titles = ["Welcome to Folio", "Shape the details", "Export with confidence"];
  const subtitles = [
    "A calmer way to finish everyday PDFs.",
    "Add text, place a signature, and arrange pages.",
    "Your source stays untouched. Your work stays local.",
  ];
  const lines = [
    { text: `FOLIO · ${String(pageIndex + 1).padStart(2, "0")}`, baseline: 92, size: 13, spacing: 2 },
    { text: titles[pageIndex] ?? titles[0], baseline: 180, size: page.width > page.height ? 44 : 42, spacing: 0 },
    { text: subtitles[pageIndex] ?? subtitles[0], baseline: 228, size: 18, spacing: 0 },
    { text: "PRIVATE BY DESIGN · LOCAL ON YOUR DEVICE", baseline: page.height - 54, size: 11, spacing: 0 },
  ];
  const characters: PageText["characters"] = [];
  for (const [index, line] of lines.entries()) {
    let x = 74;
    for (const text of line.text) {
      // The generated browser demo uses approximate metrics; native PDFs use PDFium glyph bounds.
      const width = line.size * (text === " " ? 0.28 : 0.52) + line.spacing;
      characters.push({ text, x, y: line.baseline - line.size * 0.8, width, height: line.size });
      x += width;
    }
    if (index < lines.length - 1) {
      characters.push({ text: "\n", x, y: line.baseline - line.size * 0.8, width: 0, height: line.size });
    }
  }
  return { characters };
}

export function createDemoAdapter(): FolioAdapter {
  const sourceId = `folio-demo-${crypto.randomUUID()}`;
  return {
    kind: "demo",
    async openPdf() {
      return { id: sourceId, name: "Folio welcome.pdf", pages: DEMO_PAGES.map((page) => ({ ...page })) };
    },
    async renderPage(_sourceId, pageIndex) { return demoSvg(pageIndex); },
    async getPageText(_sourceId, pageIndex) { return demoPageText(pageIndex); },
    async exportPdf(pages) {
      const blob = new Blob([JSON.stringify({ format: "folio-demo-edit-plan", pages }, null, 2)], { type: "application/json" });
      const url = URL.createObjectURL(blob);
      const anchor = document.createElement("a");
      anchor.href = url;
      anchor.download = "folio-demo-edit-plan.json";
      anchor.click();
      URL.revokeObjectURL(url);
      return "folio-demo-edit-plan.json";
    },
    async closeDocument() {},
    async engineStatus() { return "Browser demo · generated pages · no PDF export"; },
  };
}

export function documentToPages(info: DocumentInfo): PagePlan[] {
  return info.pages.map((page, pageIndex) => ({
    id: `${info.id}-page-${pageIndex}-${crypto.randomUUID?.() ?? pageIndex}`,
    sourceId: info.id,
    pageIndex,
    width: page.width,
    height: page.height,
    rotation: 0,
    overlays: (page.overlays ?? []).map(cloneOverlay),
  }));
}