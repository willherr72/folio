import {clearExistingTextCache} from "./existing-text-cache";
import {nativeTextRemovalAdapter,type TextRemovalAdapter,type ContentRemovalAdapter} from './text-removal';
import { cloneOverlay } from "./model";
import {startPdfSave,type PdfSaveOptions,type PdfSaveProgress,type PdfSaveTask} from './pdf-save';
import type {ExistingImages,ImageFitMode,ExistingImageEdit} from "./types";
import { invoke } from "@tauri-apps/api/core";
import {openProtectedPdf,type OpenPdfOptions} from "./password-native";
import type {SourceReplacement,PageSearchResult,PageContentReplacement} from "./types";
import type {StyledParagraphSelection,StyledParagraphLayoutRequest,FormParagraphRuns} from "./types";
import type {FormField,FillFormFieldRequest} from "./types";
import type {NativeComment,EditNativeCommentRequest,DeleteNativeCommentRequest} from "./types";
import type {OcrPreparedPage,OcrUpdate} from "./ocr/types";
import type { DocumentInfo, PagePlan, PageText, TextRuns, TextGroupPreview, TextParagraphSelection, AnnotationRect, FormTextRuns, ImagePageOptions, DocumentBookmark } from "./types";

export interface FolioAdapter extends Partial<TextRemovalAdapter>,Partial<ContentRemovalAdapter> {
  mapParagraphDraft?(sourceId:string,pageIndex:number,objectIndices:number[],expectedParagraphText:string,anchorIndex:number,expectedSourceText:string,kind:"uniform"|"styled"):Promise<import('./paragraph-draft').ParagraphDraftMapping>;
  mapFormParagraphDraft?(sourceId:string,pageIndex:number,objectPath:number[],objectIndices:number[],expectedParagraphText:string,anchorIndex:number,expectedSourceText:string):Promise<import('./paragraph-draft').ParagraphDraftMapping>;
  inspectDocumentProperties?(sourceId:string):Promise<import('./document-properties').DocumentProperties>;
  listExistingImages?(sourceId:string,pageIndex:number):Promise<ExistingImages>;
  previewImageEditOccurrence?(sourceId:string,occurrences:number[],selectedOccurrence:number,objectPath:number[],selectionId:string,edit:ExistingImageEdit):Promise<PageContentReplacement>;
  previewImageReplacementOccurrence?(sourceId:string,occurrences:number[],selectedOccurrence:number,objectPath:number[],selectionId:string,assetId:string,fitMode:ImageFitMode):Promise<PageContentReplacement>;
  listFormParagraphRuns?(sourceId:string,pageIndex:number,objectPath:number[],expectedText:string):Promise<FormParagraphRuns>;
  inspectFormStyledParagraph?(sourceId:string,pageIndex:number,objectPath:number[],objectIndices:number[]):Promise<StyledParagraphSelection>;
  suggestFormStyledParagraph?(sourceId:string,pageIndex:number,objectPath:number[],objectIndex:number,expectedText:string):Promise<StyledParagraphSelection>;
  fitFormStyledParagraph?(sourceId:string,pageIndex:number,objectPath:number[],request:StyledParagraphLayoutRequest):Promise<AnnotationRect>;
  previewFormStyledParagraphOccurrence?(sourceId:string,occurrences:number[],selectedOccurrence:number,objectPath:number[],request:StyledParagraphLayoutRequest):Promise<PageContentReplacement>;
  kind: "native" | "demo";
  listNativeComments?(sourceId:string):Promise<NativeComment[]>;
  deleteNativeComment?(sourceId:string,request:DeleteNativeCommentRequest):Promise<DocumentInfo>;
  editNativeComment?(sourceId:string,request:EditNativeCommentRequest):Promise<DocumentInfo>;
  openPdf(options?:OpenPdfOptions): Promise<DocumentInfo | null>;
  unlockSources?(sourceIds:string[],password:string):Promise<SourceReplacement[]|null>;
  checkSourceCombination?(sourceIds:string[]):Promise<void>;
  listFormFields?(sourceId:string,pageIndex:number):Promise<FormField[]>;
  inspectFormProperties?(sourceId:string,pageIndex:number,annotationIndex:number):Promise<import('./form-properties').FormProperties>;
  setFormProperties?(sourceId:string,request:import('./form-properties').SetFormPropertiesRequest):Promise<DocumentInfo>;
  inspectFormRemoval?(sourceId:string,pageIndex:number,annotationIndex:number):Promise<import('./form-removal').FormRemovalInfo>;
  removeFormField?(sourceId:string,request:import('./form-removal').RemoveFormFieldRequest):Promise<DocumentInfo>;
  inspectFormChoices?(sourceId:string,pageIndex:number,annotationIndex:number):Promise<import('./form-choices').FormChoices>;
  setFormChoices?(sourceId:string,request:import('./form-choices').SetFormChoicesRequest):Promise<DocumentInfo>;
  createFormField?(sourceId:string,occurrences:number[],selected:number,request:import("./form-authoring").CreateFormFieldRequest):Promise<PageContentReplacement>;
  positionFormField?(sourceId:string,occurrences:number[],selected:number,request:import("./form-positioning").PositionFormFieldRequest):Promise<PageContentReplacement>;
  inspectFormTextAppearance?(sourceId:string,pageIndex:number,annotationIndex:number):Promise<import('./form-text-appearance').FormTextAppearance>;
  inspectFormFrame?(sourceId:string,pageIndex:number,annotationIndex:number):Promise<import('./form-frame').FormFrameAppearance>;
  setFormFrame?(sourceId:string,occurrences:number[],selected:number,request:import('./form-frame').SetFormFrameRequest):Promise<PageContentReplacement>;
  setFormTextAppearance?(sourceId:string,occurrences:number[],selected:number,request:import('./form-text-appearance').SetFormTextAppearanceRequest):Promise<PageContentReplacement>;
  fillFormField?(sourceId:string,request:FillFormFieldRequest):Promise<DocumentInfo>;
  replacePageContent?(destinationId:string,occurrences:number[],selectedOccurrence:number,incomingId:string,incomingPage:number):Promise<PageContentReplacement>;
  prepareOcrPage?(sourceId:string,pageIndex:number,rotation:0|90|180|270):Promise<OcrPreparedPage>;
  applyOcr?(sourceId:string,updates:OcrUpdate[]):Promise<DocumentInfo>;
  createBlankDocument?(width:number,height:number):Promise<DocumentInfo>;
  pickImages?():Promise<string[]|null>;
  createImageDocument?(paths:string[],options:ImagePageOptions):Promise<DocumentInfo>;
  renderPage(sourceId: string, pageIndex: number, width: number): Promise<string>;
  renderPagePlan?(plan:PagePlan,width:number):Promise<string>;
  getPageText?(sourceId: string, pageIndex: number): Promise<PageText>;
  searchPage?(sourceId:string,pageIndex:number,query:string):Promise<PageSearchResult>;
  listFormTextRuns?(sourceId: string, pageIndex: number): Promise<FormTextRuns>;
  listOccurrenceFormTextRuns?(sourceId:string,pageIndex:number):Promise<FormTextRuns>;
  replaceFormTextOccurrence?(sourceId:string,occurrences:number[],selectedOccurrence:number,objectPath:number[],expectedText:string,replacement:string,fontId?:string):Promise<PageContentReplacement>;
  replaceFormText?(sourceId: string, pageIndex: number, objectPath: number[], expectedText: string, replacement: string, fontId?: string): Promise<DocumentInfo>;
  listTextRuns?(sourceId: string, pageIndex: number): Promise<TextRuns>;
  listOccurrenceTextRuns?(sourceId:string,pageIndex:number):Promise<TextRuns>;
  replaceTextOccurrence?(sourceId:string,occurrences:number[],selectedOccurrence:number,objectIndex:number,expectedText:string,replacement:string,fontId?:string):Promise<PageContentReplacement>;
  inspectOccurrenceTextParagraph?(sourceId:string,pageIndex:number,objectIndices:number[]):Promise<TextParagraphSelection>;
  suggestOccurrenceTextParagraph?(sourceId:string,pageIndex:number,objectIndex:number,expectedText:string):Promise<TextParagraphSelection>;
  inspectOccurrenceStyledParagraph?(sourceId:string,pageIndex:number,objectIndices:number[]):Promise<StyledParagraphSelection>;
  suggestOccurrenceStyledParagraph?(sourceId:string,pageIndex:number,objectIndex:number,expectedText:string):Promise<StyledParagraphSelection>;
  fitOccurrenceStyledParagraph?(sourceId:string,pageIndex:number,request:StyledParagraphLayoutRequest):Promise<AnnotationRect>;
  previewStyledParagraphOccurrence?(sourceId:string,occurrences:number[],selectedOccurrence:number,request:StyledParagraphLayoutRequest):Promise<PageContentReplacement>;
  previewTextParagraphOccurrence?(sourceId:string,occurrences:number[],selectedOccurrence:number,objectIndices:number[],expectedText:string,replacement:string,bounds:AnnotationRect,fontSize?:number,alignment?:"left"|"center"|"right"|"justify",lineSpacing?:number,fontId?:string,color?:string):Promise<PageContentReplacement>;
  inspectTextParagraph?(sourceId: string, pageIndex: number, objectIndices: number[]): Promise<TextParagraphSelection>;
  inspectStyledParagraph?(sourceId:string,pageIndex:number,objectIndices:number[]):Promise<StyledParagraphSelection>;
  suggestStyledParagraph?(sourceId:string,pageIndex:number,objectIndex:number,expectedText:string):Promise<StyledParagraphSelection>;
  previewStyledParagraph?(sourceId:string,pageIndex:number,request:StyledParagraphLayoutRequest):Promise<DocumentInfo>;
  fitStyledParagraph?(sourceId:string,pageIndex:number,request:StyledParagraphLayoutRequest):Promise<AnnotationRect>;
  suggestTextParagraph?(sourceId: string, pageIndex: number, objectIndex: number, expectedText: string): Promise<TextParagraphSelection>;
  previewTextParagraph?(sourceId: string, pageIndex: number, objectIndices: number[], expectedText: string, replacement: string, bounds: AnnotationRect, fontSize?: number, alignment?: "left"|"center"|"right"|"justify", lineSpacing?: number, fontId?: string, color?: string): Promise<DocumentInfo>;
  inspectOccurrenceTextGroup?(sourceId:string,pageIndex:number,objectIndices:number[]):Promise<TextGroupPreview>;
  replaceTextGroupOccurrence?(sourceId:string,occurrences:number[],selectedOccurrence:number,objectIndices:number[],expectedText:string,replacement:string):Promise<PageContentReplacement>;
  inspectTextGroup?(sourceId: string, pageIndex: number, objectIndices: number[]): Promise<TextGroupPreview>;
  replaceTextGroup?(sourceId: string, pageIndex: number, objectIndices: number[], expectedText: string, replacement: string): Promise<DocumentInfo>;
  replaceText?(sourceId: string, pageIndex: number, objectIndex: number, expectedText: string, replacement: string, fontId?: string): Promise<DocumentInfo>;
  exportPdf(pages: PagePlan[], options?: {flatten?: boolean;bookmarks?:DocumentBookmark[];metadata?:import('./document-properties').DocumentMetadataEdit}): Promise<string | null>;
  startPdfSave?(pages:PagePlan[],options:PdfSaveOptions,onProgress:(event:PdfSaveProgress)=>void):PdfSaveTask;
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
  inspectDocumentProperties:sourceId=>invoke('inspect_document_properties',{sourceId}),
  ...nativeTextRemovalAdapter,
  startPdfSave,
  listExistingImages:(sourceId,pageIndex)=>invoke("list_existing_images",{sourceId,pageIndex}),
  previewImageReplacementOccurrence:(sourceId,occurrences,selectedOccurrence,objectPath,selectionId,assetId,fitMode)=>invoke("preview_image_replacement_occurrence",{sourceId,occurrences,selectedOccurrence,objectPath,selectionId,assetId,fitMode}),
  previewImageEditOccurrence:(sourceId,occurrences,selectedOccurrence,objectPath,selectionId,edit)=>invoke("preview_image_edit_occurrence",{sourceId,occurrences,selectedOccurrence,objectPath,selectionId,edit}),
  mapParagraphDraft:(sourceId,pageIndex,objectIndices,expectedParagraphText,anchorIndex,expectedSourceText,kind)=>invoke("map_paragraph_draft",{sourceId,pageIndex,objectIndices,expectedParagraphText,anchorIndex,expectedSourceText,kind}),
  mapFormParagraphDraft:(sourceId,pageIndex,objectPath,objectIndices,expectedParagraphText,anchorIndex,expectedSourceText)=>invoke("map_form_paragraph_draft",{sourceId,pageIndex,objectPath,objectIndices,expectedParagraphText,anchorIndex,expectedSourceText}),
  listFormParagraphRuns:(sourceId,pageIndex,objectPath,expectedText)=>invoke("list_form_paragraph_runs",{sourceId,pageIndex,objectPath,expectedText}),
  inspectFormStyledParagraph:(sourceId,pageIndex,objectPath,objectIndices)=>invoke("inspect_form_styled_paragraph",{sourceId,pageIndex,objectPath,objectIndices}),
  suggestFormStyledParagraph:(sourceId,pageIndex,objectPath,objectIndex,expectedText)=>invoke("suggest_form_styled_paragraph",{sourceId,pageIndex,objectPath,objectIndex,expectedText}),
  fitFormStyledParagraph:(sourceId,pageIndex,objectPath,request)=>invoke("fit_form_styled_paragraph",{sourceId,pageIndex,objectPath,request}),
  previewFormStyledParagraphOccurrence:(sourceId,occurrences,selectedOccurrence,objectPath,request)=>invoke("preview_form_styled_paragraph_occurrence",{sourceId,occurrences,selectedOccurrence,objectPath,request}),
  kind: "native",
  listNativeComments:sourceId=>invoke<NativeComment[]>("list_native_comments",{sourceId}),
  deleteNativeComment:(sourceId,request)=>invoke<DocumentInfo>("delete_native_comment",{sourceId,request}),
  editNativeComment:(sourceId,request)=>invoke<DocumentInfo>("edit_native_comment",{sourceId,request}),
  openPdf: options => options?openProtectedPdf(options):invoke<DocumentInfo | null>("open_pdf"),
  unlockSources:(sourceIds,password)=>invoke<SourceReplacement[]|null>("unlock_sources",{sourceIds,password}),
  checkSourceCombination:sourceIds=>invoke<void>("check_source_combination",{sourceIds}),
  searchPage:(sourceId,pageIndex,query)=>invoke<PageSearchResult>("search_page",{sourceId,pageIndex,query}),
  listFormFields:(sourceId,pageIndex)=>invoke<FormField[]>("list_form_fields",{sourceId,pageIndex}),
  createFormField:(sourceId,occurrences,selected,request)=>invoke<PageContentReplacement>("create_form_field_occurrence",{sourceId,occurrences,selected,request}),
  positionFormField:(sourceId,occurrences,selected,request)=>invoke<PageContentReplacement>("position_form_field_occurrence",{sourceId,occurrences,selected,request}),
  inspectFormTextAppearance:(sourceId,pageIndex,annotationIndex)=>invoke('inspect_form_text_appearance',{sourceId,pageIndex,annotationIndex}),
  inspectFormFrame:(sourceId,pageIndex,annotationIndex)=>invoke('inspect_form_frame',{sourceId,pageIndex,annotationIndex}),
  setFormFrame:(sourceId,occurrences,selected,request)=>invoke('set_form_frame_occurrence',{sourceId,occurrences,selected,request}),
  inspectFormProperties:(sourceId,pageIndex,annotationIndex)=>invoke('inspect_form_properties',{sourceId,pageIndex,annotationIndex}),
  setFormProperties:(sourceId,request)=>invoke('set_form_properties',{sourceId,request}),
  inspectFormRemoval:(sourceId,pageIndex,annotationIndex)=>invoke('inspect_form_removal',{sourceId,pageIndex,annotationIndex}),
  removeFormField:(sourceId,request)=>invoke('remove_form_field',{sourceId,request}),
  inspectFormChoices:(sourceId,pageIndex,annotationIndex)=>invoke('inspect_form_choices',{sourceId,pageIndex,annotationIndex}),
  setFormChoices:(sourceId,request)=>invoke('set_form_choices',{sourceId,request}),
  setFormTextAppearance:(sourceId,occurrences,selected,request)=>invoke('set_form_text_appearance_occurrence',{sourceId,occurrences,selected,request}),
  fillFormField:(sourceId,request)=>invoke<DocumentInfo>("fill_form_field",{sourceId,request}),
  replacePageContent:(destinationId,occurrences,selectedOccurrence,incomingId,incomingPage)=>invoke<PageContentReplacement>("replace_page_content_occurrence",{destinationId,occurrences,selectedOccurrence,incomingId,incomingPage}),
  async prepareOcrPage(sourceId,pageIndex,rotation) {
    const result=await invoke<Omit<OcrPreparedPage,"png">&{png:number[]}>("prepare_ocr_page",{sourceId,pageIndex,rotation});
    return {...result,png:new Uint8Array(result.png)};
  },
  applyOcr:(sourceId,updates)=>invoke<DocumentInfo>("apply_ocr",{sourceId,updates}),
  createBlankDocument: (width,height) => invoke<DocumentInfo>("create_blank_document",{width,height}),
  pickImages: () => invoke<string[]|null>("pick_images"),
  createImageDocument: (paths,options) => invoke<DocumentInfo>("create_image_document",{paths,options}),
  async renderPagePlan(plan,width) {
    return bytesToUrl(await invoke<ArrayBuffer | Uint8Array | number[]>("render_page_plan",{plan,width:Math.max(64,Math.min(2400,Math.round(width)))}));
  },
  async renderPage(sourceId, pageIndex, width) {
    const bytes = await invoke<ArrayBuffer | Uint8Array | number[]>("render_page", {
      sourceId,
      pageIndex,
      width: Math.max(64, Math.min(2400, Math.round(width))),
    });
    return bytesToUrl(bytes);
  },
  getPageText: (sourceId, pageIndex) => invoke<PageText>("page_text", { sourceId, pageIndex }),
  listFormTextRuns: (sourceId, pageIndex) => invoke<FormTextRuns>("list_form_text_runs", {sourceId,pageIndex}),
  listOccurrenceFormTextRuns: (sourceId,pageIndex) => invoke<FormTextRuns>("list_occurrence_form_text_runs",{sourceId,pageIndex}),
  replaceFormTextOccurrence: (sourceId,occurrences,selectedOccurrence,objectPath,expectedText,replacement,fontId) => invoke<PageContentReplacement>("replace_form_text_occurrence",{sourceId,occurrences,selectedOccurrence,objectPath,expectedText,replacement,...(fontId?{fontId}:{})}),
  replaceFormText: (sourceId,pageIndex,objectPath,expectedText,replacement,fontId) => invoke<DocumentInfo>("replace_form_text", {sourceId,pageIndex,objectPath,expectedText,replacement,...(fontId?{fontId}:{})}),
  listTextRuns: (sourceId, pageIndex) => invoke<TextRuns>("list_text_runs", {sourceId,pageIndex}),
  listOccurrenceTextRuns:(sourceId,pageIndex)=>invoke<TextRuns>("list_occurrence_text_runs",{sourceId,pageIndex}),
  replaceTextOccurrence:(sourceId,occurrences,selectedOccurrence,objectIndex,expectedText,replacement,fontId)=>invoke<PageContentReplacement>("replace_text_occurrence",{sourceId,occurrences,selectedOccurrence,objectIndex,expectedText,replacement,...(fontId?{fontId}:{})}),
  inspectTextParagraph: (sourceId, pageIndex, objectIndices) => invoke<TextParagraphSelection>("inspect_text_paragraph", {sourceId,pageIndex,objectIndices}),
  inspectStyledParagraph:(sourceId,pageIndex,objectIndices)=>invoke<StyledParagraphSelection>("inspect_styled_paragraph",{sourceId,pageIndex,objectIndices}),
  suggestStyledParagraph:(sourceId,pageIndex,objectIndex,expectedText)=>invoke<StyledParagraphSelection>("suggest_styled_paragraph",{sourceId,pageIndex,objectIndex,expectedText}),
  previewStyledParagraph:(sourceId,pageIndex,request)=>invoke<DocumentInfo>("preview_styled_paragraph",{sourceId,pageIndex,request}),
  inspectOccurrenceTextGroup:(sourceId,pageIndex,objectIndices)=>invoke("inspect_occurrence_text_group",{sourceId,pageIndex,objectIndices}),
  replaceTextGroupOccurrence:(sourceId,occurrences,selectedOccurrence,objectIndices,expectedText,replacement)=>invoke("replace_text_group_occurrence",{sourceId,occurrences,selectedOccurrence,objectIndices,expectedText,replacement}),
  inspectOccurrenceTextParagraph:(sourceId,pageIndex,objectIndices)=>invoke("inspect_occurrence_text_paragraph",{sourceId,pageIndex,objectIndices}),
  suggestOccurrenceTextParagraph:(sourceId,pageIndex,objectIndex,expectedText)=>invoke("suggest_occurrence_text_paragraph",{sourceId,pageIndex,objectIndex,expectedText}),
  inspectOccurrenceStyledParagraph:(sourceId,pageIndex,objectIndices)=>invoke("inspect_occurrence_styled_paragraph",{sourceId,pageIndex,objectIndices}),
  suggestOccurrenceStyledParagraph:(sourceId,pageIndex,objectIndex,expectedText)=>invoke("suggest_occurrence_styled_paragraph",{sourceId,pageIndex,objectIndex,expectedText}),
  fitOccurrenceStyledParagraph:(sourceId,pageIndex,request)=>invoke("fit_occurrence_styled_paragraph",{sourceId,pageIndex,request}),
  previewStyledParagraphOccurrence:(sourceId,occurrences,selectedOccurrence,request)=>invoke("preview_styled_paragraph_occurrence",{sourceId,occurrences,selectedOccurrence,request}),
  previewTextParagraphOccurrence:(sourceId,occurrences,selectedOccurrence,objectIndices,expectedText,replacement,bounds,fontSize,alignment,lineSpacing,fontId,color)=>invoke("preview_text_paragraph_occurrence",{sourceId,occurrences,selectedOccurrence,objectIndices,expectedText,replacement,bounds,...(fontSize===undefined?{}:{fontSize}),...(alignment===undefined?{}:{alignment}),...(lineSpacing===undefined?{}:{lineSpacing}),...(fontId===undefined?{}:{fontId}),...(color===undefined?{}:{color})}),
  fitStyledParagraph:(sourceId,pageIndex,request)=>invoke<AnnotationRect>("fit_styled_paragraph",{sourceId,pageIndex,request}),
  suggestTextParagraph: (sourceId, pageIndex, objectIndex, expectedText) => invoke<TextParagraphSelection>("suggest_text_paragraph", {sourceId,pageIndex,objectIndex,expectedText}),
  previewTextParagraph: (sourceId, pageIndex, objectIndices, expectedText, replacement, bounds, fontSize, alignment, lineSpacing, fontId, color) => invoke<DocumentInfo>("preview_text_paragraph", {sourceId,pageIndex,objectIndices,expectedText,replacement,bounds,...(fontSize===undefined?{}:{fontSize}),...(alignment===undefined?{}:{alignment}),...(lineSpacing===undefined?{}:{lineSpacing}),...(fontId===undefined?{}:{fontId}),...(color===undefined?{}:{color})}),
  inspectTextGroup: (sourceId, pageIndex, objectIndices) => invoke<TextGroupPreview>("inspect_text_group", {sourceId,pageIndex,objectIndices}),
  replaceTextGroup: (sourceId, pageIndex, objectIndices, expectedText, replacement) => invoke<DocumentInfo>("replace_text_group", {sourceId,pageIndex,objectIndices,expectedText,replacement}),
  replaceText: (sourceId, pageIndex, objectIndex, expectedText, replacement, fontId) => invoke<DocumentInfo>("replace_text", {sourceId,pageIndex,objectIndex,expectedText,replacement,...(fontId ? {fontId} : {})}),
  exportPdf: (pages, options) => invoke<string | null>("export_pdf", { request: { pages, ...(options?.flatten ? {flatten:true} : {}) },...(options?.bookmarks!==undefined?{bookmarks:options.bookmarks}:{}),...(options?.metadata!==undefined?{metadata:options.metadata}:{}) }),
  closeDocument: async (sourceId) => {
    clearExistingTextCache([sourceId]);
    try { await invoke<void>("close_document", { sourceId }); }
    finally { clearExistingTextCache([sourceId]); }
  },
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
    async exportPdf(pages,options) {
      const blob = new Blob([JSON.stringify({ format: "folio-demo-edit-plan", pages,...(options?.bookmarks!==undefined?{bookmarks:options.bookmarks}:{}) }, null, 2)], { type: "application/json" });
      const url = URL.createObjectURL(blob);
      const anchor = document.createElement("a");
      anchor.href = url;
      anchor.download = "folio-demo-edit-plan.json";
      anchor.click();
      URL.revokeObjectURL(url);
      return "folio-demo-edit-plan.json";
    },
    async closeDocument(sourceId) { clearExistingTextCache([sourceId]); },
    async engineStatus() { return "Browser demo · generated pages · no PDF export"; },
  };
}

export function documentToPages(info: DocumentInfo): PagePlan[] {
  return info.pages.map((page, pageIndex) => ({
    id: `${info.id}-page-${pageIndex}-${crypto.randomUUID?.() ?? pageIndex}`,
    sourceId: info.id,
    ...(info.protection?{protection:info.protection}:{}),
    pageIndex,
    width: page.width,
    height: page.height,
    rotation: 0,
    overlays: (page.overlays ?? []).map(cloneOverlay),
  }));
}
