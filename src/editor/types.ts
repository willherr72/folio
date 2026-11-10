import type { TextFontName } from "./text-fonts";
export interface ImagePoint {x:number;y:number}
export interface ImageCrop {left:number;top:number;right:number;bottom:number}
export interface ExistingImage {objectPath:number[];selectionId:string;corners:[ImagePoint,ImagePoint,ImagePoint,ImagePoint];bounds:AnnotationRect;pixelWidth:number;pixelHeight:number;crop?:ImageCrop}
export interface ExistingImages {images:ExistingImage[]}
export type ImageFitMode="fit"|"fill"|"stretch";
export interface ImageTransform {deltaX:number;deltaY:number;scaleX:number;scaleY:number;rotationDegrees:number}
export type ExistingImageEdit={kind:"transform";transform:ImageTransform}|{kind:"adjust";transform:ImageTransform;crop:ImageCrop}|{kind:"remove"};
export type Rotation = 0 | 90 | 180 | 270;
export interface ImagePageOptions {width:number;height:number;margin:number;autoOrientation:boolean}
export type FormFieldValue={kind:"text";text:string}|{kind:"toggle";checked:boolean}|{kind:"choice";indices:number[]};
export interface FormField {
  annotationIndex:number;name:string;label:string;kind:string;value:FormFieldValue;options:string[];selectedIndices:number[];
  readOnly:boolean;required:boolean;multiline:boolean;password:boolean;multiSelect:boolean;editableChoice:boolean;reason:string|null;
  /** Source display coordinates, including intrinsic rotation; older adapters may omit geometry. */
  bounds?:AnnotationRect|null;visible?:boolean;noRotate?:boolean;
}
export interface FillFormFieldRequest {pageIndex:number;annotationIndex:number;expectedValue:FormFieldValue;value:FormFieldValue}
export type PdfValue = {kind:"null"}|{kind:"boolean";value:boolean}|{kind:"integer"|"real";value:number}
  |{kind:"name"|"string";value:number[]}|{kind:"page";value:string}|{kind:"array";value:PdfValue[]}
  |{kind:"dictionary";value:Array<[number[],PdfValue]>}
  |{kind:"stream";value:{dictionary:Array<[number[],PdfValue]>;content:number[]}};
export interface DocumentBookmark {
  id:string;title:string;level:number;destination?:PdfValue;action?:PdfValue;
  bold?:boolean;italic?:boolean;closed?:boolean;color?:[number,number,number];
}
export type EditorTool = "images" | "edit" | "form" | "select" | "text" | "area" | "image" | "signature" | "draw" | "highlight" | "comment";

export interface TextOverlay {
  type: "text";
  id: string;
  x: number;
  y: number;
  text: string;
  fontSize: number;
  /** Absent in older documents; defaults to Helvetica. */
  fontName?: TextFontName;
  /** Content-addressed, embedded custom font program. */
  fontId?: string;
  /** Explicit native layout; absence preserves legacy rendering. */
  shaping?: { version: 1; direction: "auto" | "ltr" | "rtl"; ligatures: boolean };
  /** Original area geometry; wrapping is regenerated from text and the embedded font. */
  area?: { version: 1; width: number; height: number; inset: number; lineSpacing: number; alignment: "left" | "center" | "right" };
  color: string;
  /** Clockwise orientation about the local top-left x/y anchor. */
  rotation?: Rotation;
}

export interface InkPoint { x: number; y: number }

export interface ImageOverlay {
  type:"image";id:string;assetId:string;x:number;y:number;width:number;height:number;rotation:Rotation;
}

export interface InkOverlay {
  type: "ink";
  id: string;
  paths: InkPoint[][];
  color: string;
  strokeWidth: number;
}

export interface AnnotationRect { x: number; y: number; width: number; height: number }
/** Existing page-content text; bounds include crop and intrinsic page rotation. */
export interface EditableTextRun {
  objectIndex: number;
  /** Complete occurrence identity for text inside nested form objects. */
  objectPath?: number[];
  text: string;
  /** Verified PDF operand text; text remains the native selection identity. */
  sourceText?: string;
  fontName: string;
  /** Readable embedded name; the raw fontName remains the PDF identity. */
  fontDisplayName?: string;
  fontSize: number;
  /** Physical vertical em-vector length; raw fontSize retains its prior meaning. */
  displayFontSize?: number;
  bounds: AnnotationRect;
  supported: boolean;
  isEmbedded?: boolean;
  canSubstitute?: boolean;
  fontSubstitutionReason?: string;
  reason?: string;
}
export interface FormTextRun extends Omit<EditableTextRun, "objectIndex" | "objectPath"> { objectPath: number[] }
export interface FormTextRuns { runs: FormTextRun[]; reason?: string }
export interface TextRuns { runs: EditableTextRun[]; reason?: string }
export interface FormParagraphRuns { anchorIndex: number; runs: EditableTextRun[] }
export interface TextGroupPreview { objectIndices: number[]; text: string; fontName: string; fontDisplayName?: string; fontSize: number; displayFontSize?: number }
export interface HighlightOverlay {
  type: "highlight";
  id: string;
  rects: AnnotationRect[];
  color: string;
  opacity?: number;
  text?: string;
}
export interface CommentOverlay {
  type: "comment";
  id: string;
  x: number;
  y: number;
  text: string;
  color: string;
}
export type Overlay = TextOverlay | InkOverlay | HighlightOverlay | CommentOverlay | ImageOverlay;

export interface PagePlan {
  /** Dirty tracking only: an owner-unlocked source has identical PDF content. */
  contentSourceId?:string;
  protection?:ProtectionInfo|null;
  id: string;
  sourceId: string;
  pageIndex: number;
  width: number;
  height: number;
  rotation: Rotation;
  /** Retained physical source-display rectangle, before workspace rotation. */
  crop?: AnnotationRect;
  overlays: Overlay[];
}

export interface DocumentInfo {
  protection?:ProtectionInfo|null;
  id: string;
  name: string;
  pages: Array<{ width: number; height: number; overlays?: Overlay[] }>;
  bookmarks?:DocumentBookmark[];
  bookmarkError?:string;
}

export interface ProtectionInfo {
 documentId:string;
 owner:boolean;
 revision:number;
 permissions:{print:boolean;printHighQuality:boolean;copyContent:boolean;editContent:boolean;annotate:boolean;fillForms:boolean;assemble:boolean};
}
export interface SourceReplacement {sourceId:string;document:DocumentInfo}
export interface PageContentReplacement {document:DocumentInfo;pageIndices:number[]}
export interface PageSearchResult {matches:AnnotationRect[][];hasText:boolean;truncated:boolean}

export interface EditorDocument {
  metadata?:import('./document-properties').DocumentMetadataEdit;
  name: string;
  pages: PagePlan[];
  selectedPageId: string | null;
  selectedOverlayId: string | null;
  bookmarks?:DocumentBookmark[];
  bookmarkError?:string;
}

export interface History<T> {
  past: T[];
  present: T;
  future: T[];
}
/** Embedded text in source reading order; coordinates include crop and intrinsic rotation. */
export interface PdfTextCharacter {
  text: string;
  x: number;
  y: number;
  width: number;
  height: number;
}

export interface PageText {
  /** Source page rotation already included in the character bounds; defaults to zero. */
  intrinsicRotation?: Rotation;
  characters: PdfTextCharacter[];
}

export type ParagraphAlignment="left"|"center"|"right"|"justify";
export interface TextParagraphSelection extends TextGroupPreview { bounds: AnnotationRect; alignment?:ParagraphAlignment|null; lineSpacing?:number|null; color?:string|null }
export interface StyledParagraphRun {text:string;styleId:number}
export interface ParagraphBaselineAnchor {x:number;y:number}
export interface StyledParagraphStyle {styleId:number;objectIndices:number[];fontName:string;fontDisplayName:string|null;fontSize:number;color:string;isEmbedded:boolean;backgroundColor?:string|null}
export interface StyledParagraphSelection {baselineAnchor?:ParagraphBaselineAnchor|null;objectIndices:number[];text:string;runs:StyledParagraphRun[];styles:StyledParagraphStyle[];bounds:AnnotationRect;alignment:ParagraphAlignment|null;lineSpacing:number|null}
/** Every override resolves from the original checked palette, never a changed target. */
export interface StyledParagraphStyleChange {styleId:number;sourceStyleId:number;fontSize?:number;color?:string;fontId?:string}
export interface StyledParagraphLayoutRequest {baselineAnchor?:ParagraphBaselineAnchor;objectIndices:number[];expectedText:string;runs:StyledParagraphRun[];styleChanges?:StyledParagraphStyleChange[];bounds:AnnotationRect;alignment:ParagraphAlignment;lineSpacing:number}

export interface NativeComment {
  pageIndex:number; annotationIndex:number; text:string;
  author?:string|null; subject?:string|null; bounds:AnnotationRect; reason?:string|null;
  color?:string|null; canChangeColor?:boolean; deleteReason?:string|null; onPage?:boolean; hitBounds?:AnnotationRect; noRotate?:boolean;
}
export interface NativeCommentChanges {text:string;position?:{x:number;y:number};color?:string}
export interface EditNativeCommentRequest extends NativeCommentChanges {pageIndex:number;annotationIndex:number;expectedText:string}
export interface DeleteNativeCommentRequest {pageIndex:number;annotationIndex:number;expectedText:string}
