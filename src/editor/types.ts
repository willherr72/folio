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

export type Overlay = TextOverlay | InkOverlay;

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
  pages: Array<{ width: number; height: number }>;
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