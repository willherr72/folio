import {useLayoutEffect, useRef, useState} from "react";
import {createRasterPainter, type RasterPaintState} from "../editor/pageRaster";

export type PageRasterStatus = {
  url?: string;
  phase: "empty" | "loading" | "ready" | "error";
  mode: "canvas" | "svg";
};
export type PageRasterProps = {
  url?: string;
  width: number;
  height: number;
  transform?: string;
  onStatus?: (status: PageRasterStatus) => void;
};

/** A borrowed page raster at SVG page coordinates. Not yet enabled in PageView:
 * whole-editor display and sustained-resource acceptance remain separate. */
export function PageRaster({url, width, height, transform, onStatus}: PageRasterProps) {
  const canvas = useRef<HTMLCanvasElement>(null);
  const painter = useRef<ReturnType<typeof createRasterPainter>>();
  const callback = useRef(onStatus);
  const [paint, setPaint] = useState<RasterPaintState>({kind: "empty"});
  const [fallback, setFallback] = useState<{paint: RasterPaintState; phase: "ready" | "error"}>();
  useLayoutEffect(() => { callback.current = onStatus; });
  useLayoutEffect(() => {
    const owned = createRasterPainter(canvas.current!, setPaint);
    painter.current = owned;
    return () => { painter.current = undefined; owned.dispose(); };
  }, []);
  useLayoutEffect(() => { painter.current!.setSource(url); }, [url]);

  // The URL comparison hides the previous pixels during the render before the
  // source effect runs; paint identity also distinguishes A-B-A fallback events.
  const current = paint.url === url;
  const mode = current && paint.kind === "fallback" ? "svg" : "canvas";
  const phase = !url ? "empty" : !current ? "loading" : paint.kind === "canvas" ? "ready"
    : paint.kind === "fallback" && fallback?.paint === paint ? fallback.phase : "loading";
  useLayoutEffect(() => { callback.current?.({url, mode, phase}); }, [url, mode, phase]);

  return <g data-page-raster="" data-raster-state={phase} data-raster-mode={mode} aria-hidden="true" pointerEvents="none">
    <foreignObject width={width} height={height} transform={transform}
      style={{pointerEvents: "none", overflow: "hidden", visibility: current && paint.kind === "canvas" ? "visible" : "hidden"}}>
      <canvas ref={canvas} style={{display: "block", width: "100%", height: "100%", pointerEvents: "none"}}/>
    </foreignObject>
    {current && paint.kind === "fallback" && <image key={url} href={url} width={width} height={height} transform={transform} preserveAspectRatio="none"
      onLoad={() => setFallback({paint, phase: "ready"})} onError={() => setFallback({paint, phase: "error"})}/>}
  </g>;
}
