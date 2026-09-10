import type { CommentOverlay, HighlightOverlay } from "../editor/types";
import "./annotations.css";
export function HighlightMarks({ overlay, selected = false }: { overlay: HighlightOverlay; selected?: boolean }) {
  return <g data-highlight-id={overlay.id} className={`annotation-highlight${selected ? " selected" : ""}`} pointerEvents="none" aria-hidden="true">
    {overlay.rects.map((rect, index) => <rect key={index} {...rect} fill={overlay.color} style={{ fillOpacity: overlay.opacity ?? 96 / 255 }} />)}
  </g>;
}
export function CommentIcon({ overlay }: { overlay: CommentOverlay }) {
  return <g className="annotation-comment" transform={`translate(${overlay.x} ${overlay.y})`}>
    <title>{overlay.text || "Comment"}</title>
    <path d="M1 1H19V15H8L3 19V15H1Z" fill={overlay.color} stroke="#594600" strokeWidth="1" strokeLinejoin="round" />
    <path d="M5 5H15M5 9H13" fill="none" stroke="#594600" strokeWidth="1.4" strokeLinecap="round" />
  </g>;
}