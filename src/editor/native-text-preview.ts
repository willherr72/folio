/** Serialize-only native v1 contract. Coordinates and placements use PDF source axes. */
export interface NativeTextPreview {
  readonly version: 1;
  readonly fontId: string;
  readonly text: string;
  readonly width: number;
  readonly height: number;
  readonly rotation: 0 | 90 | 180 | 270;
  readonly color: readonly [number, number, number];
  readonly outlines: readonly { readonly glyphId: number; readonly path: string }[];
  readonly glyphs: readonly {
    readonly glyphId: number;
    readonly transform: readonly [number, number, number, number, number, number];
  }[];
}
