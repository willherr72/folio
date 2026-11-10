export type OcrLanguage = 'eng' | 'fra' | 'spa';
export interface OcrRaster { png: Uint8Array; width: number; height: number; dpi: number }
export interface OcrBounds { x: number; y: number; width: number; height: number }
export interface OcrWord { text: string; confidence: number; bounds: OcrBounds }
export interface OcrLine { words: OcrWord[] }
export interface OcrParagraph { lines: OcrLine[] }
export interface OcrBlock { paragraphs: OcrParagraph[] }
export interface OcrPage { width: number; height: number; blocks: OcrBlock[] }
export interface OcrPreparedPage {png:Uint8Array;width:number;height:number;effectiveDpi:number;exclusions:OcrBounds[];owned:OcrPage|null}
export interface OcrUpdate {pageIndex:number;rotation:0|90|180|270;page:OcrPage;preserveOwnedOrientation:boolean}
export interface OcrProgress { stage: 'initializing' | 'recognizing'; fraction: number }
export interface OcrSession {
  recognize(raster: OcrRaster, signal?: AbortSignal): Promise<OcrPage>;
  dispose(): void;
}
export interface OcrSessionOptions {
  signal?: AbortSignal;
  onProgress?: (progress: OcrProgress) => void;
}
