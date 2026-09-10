export type Rotation = 0 | 90 | 180 | 270;

export interface TextOverlay {
  type: "text";
  id: string;
  x: number;
  y: number;
  text: string;
  fontSize: number;
  color: string;
}

export interface InkPoint { x: number; y: number }

export interface InkOverlay {
  type: "ink";
  id: string;
  paths: InkPoint[][];
  color: string;
  strokeWidth: number;
}

export interface AnnotationRect { x: number; y: number; width: number; height: number }
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
export type Overlay = TextOverlay | InkOverlay | HighlightOverlay | CommentOverlay;

export interface PagePlan {
  id: string;
  sourceId: string;
  pageIndex: number;
  width: number;
  height: number;
  rotation: Rotation;
  overlays: Overlay[];
}

export interface DocumentInfo {
  id: string;
  name: string;
  pages: Array<{ width: number; height: number; overlays?: Overlay[] }>;
}

export interface EditorDocument {
  name: string;
  pages: PagePlan[];
  selectedPageId: string | null;
  selectedOverlayId: string | null;
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
