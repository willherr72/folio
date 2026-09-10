import { useRef, useState } from "react";
import { Eraser, PenLine, X } from "lucide-react";
import type { InkPoint } from "../editor/types";

interface SignaturePadProps {
  onCancel(): void;
  onAccept(paths: InkPoint[][]): void;
}

export function SignaturePad({ onCancel, onAccept }: SignaturePadProps) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const activePath = useRef<InkPoint[] | null>(null);
  const [paths, setPaths] = useState<InkPoint[][]>([]);

  const point = (event: React.PointerEvent<HTMLCanvasElement>) => {
    const rect = event.currentTarget.getBoundingClientRect();
    return { x: (event.clientX - rect.left) * 360 / rect.width, y: (event.clientY - rect.top) * 140 / rect.height };
  };

  const draw = (allPaths: InkPoint[][]) => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    const context = canvas.getContext("2d");
    if (!context) return;
    context.clearRect(0, 0, canvas.width, canvas.height);
    context.strokeStyle = "#2D2A26";
    context.lineWidth = 2.6;
    context.lineCap = "round";
    context.lineJoin = "round";
    allPaths.forEach((path) => {
      if (path.length < 2) return;
      context.beginPath();
      context.moveTo(path[0].x, path[0].y);
      path.slice(1).forEach((value) => context.lineTo(value.x, value.y));
      context.stroke();
    });
  };

  const begin = (event: React.PointerEvent<HTMLCanvasElement>) => {
    event.currentTarget.setPointerCapture(event.pointerId);
    activePath.current = [point(event)];
  };

  const move = (event: React.PointerEvent<HTMLCanvasElement>) => {
    if (!activePath.current) return;
    activePath.current.push(point(event));
    draw([...paths, activePath.current]);
  };

  const end = () => {
    if (!activePath.current) return;
    const next = activePath.current.length > 1 ? [...paths, activePath.current] : paths;
    activePath.current = null;
    setPaths(next);
    draw(next);
  };

  const clear = () => { setPaths([]); draw([]); };

  return (
    <div className="modal-backdrop" role="presentation" onPointerDown={(event) => event.target === event.currentTarget && onCancel()}>
      <section className="signature-dialog" role="dialog" aria-modal="true" aria-labelledby="signature-heading">
        <header>
          <div className="dialog-mark"><PenLine size={18} /></div>
          <div><h2 id="signature-heading">Create a signature</h2><p>Draw with your mouse, trackpad, or pen.</p></div>
          <button className="icon-button" aria-label="Close signature pad" onClick={onCancel}><X size={18} /></button>
        </header>
        <div className="signature-paper">
          <canvas ref={canvasRef} width={360} height={140} aria-label="Signature drawing area"
            onPointerDown={begin} onPointerMove={move} onPointerUp={end} onPointerCancel={end} />
          <span>Sign above the line</span>
        </div>
        <footer>
          <button className="button subtle" onClick={clear}><Eraser size={16} /> Clear</button>
          <div className="dialog-actions"><button className="button" onClick={onCancel}>Cancel</button><button className="button primary" disabled={!paths.length} onClick={() => onAccept(paths)}>Use signature</button></div>
        </footer>
      </section>
    </div>
  );
}