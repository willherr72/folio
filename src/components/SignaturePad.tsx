import { useId, useRef, useState } from "react";
import { Eraser, Plus, Save, Trash2 } from "lucide-react";
import { Modal } from "./Modal";
import { deleteSignature, loadSignatures, MAX_SIGNATURE_POINTS, renameSignature, resetSignatureLibrary, saveSignature, type SavedSignature } from "../editor/signatures";
import type { InkPoint } from "../editor/types";
import "./signature-library.css";

interface SignaturePadProps {
  onCancel(): void;
  onAccept(paths: InkPoint[][]): void;
}

const errorMessage = (error: unknown) => error instanceof Error ? error.message : String(error);
function initialLibrary() {
  try { return { entries: loadSignatures(), error: null as string | null }; }
  catch (error) { return { entries: [] as SavedSignature[], error: errorMessage(error) }; }
}
function SignaturePreview({ paths, label }: { paths: InkPoint[][]; label?: string }) {
  return <svg viewBox="0 0 360 140" role={label ? "img" : undefined} aria-label={label} aria-hidden={label ? undefined : true}>
    {paths.map((path, index) => <polyline key={index} points={path.map(point => `${point.x},${point.y}`).join(" ")} fill="none" stroke="#2D2A26" strokeWidth="2.6" strokeLinecap="round" strokeLinejoin="round" />)}
  </svg>;
}

export function SignaturePad({ onCancel, onAccept }: SignaturePadProps) {
  const cancelRef = useRef<HTMLButtonElement>(null);
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const activePath = useRef<{ pointerId: number; points: InkPoint[] } | null>(null);
  const [paths, setPaths] = useState<InkPoint[][]>([]);
  const [initial] = useState(initialLibrary);
  const [library, setLibrary] = useState(initial.entries);
  const [error, setError] = useState(initial.error);
  const [libraryError, setLibraryError] = useState(!!initial.error);
  const [notice, setNotice] = useState<string | null>(null);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [draftName, setDraftName] = useState("");
  const [renameName, setRenameName] = useState("");
  const [resetConfirm, setResetConfirm] = useState(false);
  const nameId = useId();
  const selected = library.find(item => item.id === selectedId);
  const acceptedPaths = selected?.paths ?? paths;

  const point = (event: React.PointerEvent<HTMLCanvasElement>) => {
    const rect = event.currentTarget.getBoundingClientRect();
    return { x: Math.max(0, Math.min(360, (event.clientX - rect.left) * 360 / rect.width)),
      y: Math.max(0, Math.min(140, (event.clientY - rect.top) * 140 / rect.height)) };
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
    allPaths.forEach(path => {
      if (path.length < 2) return;
      context.beginPath(); context.moveTo(path[0].x, path[0].y);
      path.slice(1).forEach(value => context.lineTo(value.x, value.y)); context.stroke();
    });
  };
  const begin = (event: React.PointerEvent<HTMLCanvasElement>) => {
    if (event.button !== 0 || activePath.current) return;
    if (paths.length >= 512 || paths.reduce((sum, path) => sum + path.length, 0) >= MAX_SIGNATURE_POINTS - 1) {
      setError("This drawing has reached its size limit. Clear it to draw a simpler signature."); return;
    }
    event.currentTarget.setPointerCapture(event.pointerId);
    activePath.current = { pointerId: event.pointerId, points: [point(event)] };
    setNotice(null);
  };
  const move = (event: React.PointerEvent<HTMLCanvasElement>) => {
    const active = activePath.current;
    if (!active || active.pointerId !== event.pointerId) return;
    if (paths.reduce((sum, path) => sum + path.length, active.points.length) >= MAX_SIGNATURE_POINTS) {
      setError("This drawing has reached its size limit. Clear it to draw a simpler signature."); return;
    }
    active.points.push(point(event)); draw([...paths, active.points]);
  };
  const end = (event: React.PointerEvent<HTMLCanvasElement>) => {
    const active = activePath.current;
    if (!active || active.pointerId !== event.pointerId) return;
    const next = active.points.length > 1 ? [...paths, active.points] : paths;
    activePath.current = null; setPaths(next); draw(next);
  };
  const clear = () => { activePath.current = null; setPaths([]); draw([]); setNotice(null); if (!libraryError) setError(null); };
  const selectSaved = (item: SavedSignature) => {
    setSelectedId(item.id); setRenameName(item.name); setNotice(null); setError(null);
  };
  const save = () => {
    try {
      const next = saveSignature(draftName, paths);
      const saved = next[next.length - 1];
      setLibrary(next); setSelectedId(saved.id); setRenameName(saved.name);
      setPaths([]); draw([]); setDraftName(""); setError(null);
      setNotice(`Saved “${saved.name}” on this device.`);
    } catch (value) { setError(errorMessage(value)); }
  };
  const rename = () => {
    if (!selected) return;
    try { setLibrary(renameSignature(selected.id, renameName)); setError(null); setNotice("Signature renamed."); }
    catch (value) { setError(errorMessage(value)); }
  };
  const remove = () => {
    if (!selected) return;
    try { setLibrary(deleteSignature(selected.id)); setSelectedId(null); setError(null); setNotice("Saved signature deleted. Signatures already placed in PDFs are unchanged."); }
    catch (value) { setError(errorMessage(value)); }
  };
  const reset = () => {
    try { resetSignatureLibrary(); setLibrary([]); setSelectedId(null); setError(null); setLibraryError(false); setResetConfirm(false); setNotice("Saved signature library reset."); }
    catch (value) { setError(errorMessage(value)); }
  };

  return <Modal title="Create a signature" description="Draw a signature or reuse one saved on this device." onClose={onCancel} className="signature-dialog signature-library-dialog" initialFocusRef={cancelRef}>
    <div className="signature-library-content">
      <aside className="signature-library-list" aria-label="Saved signatures">
        <h3>Saved signatures <span>{library.length}/30</span></h3>
        <button className="button signature-new" aria-pressed={!selected} onClick={() => { setSelectedId(null); setNotice(null); if (!libraryError) setError(null); }}><Plus size={15} /> Draw new</button>
        {!library.length && <p className="signature-empty">Save a named signature to use it again later.</p>}
        <ul>{library.map(item => <li key={item.id}>
          <button className="signature-saved" aria-label={`Select signature ${item.name}`} aria-pressed={selected?.id === item.id} onClick={() => selectSaved(item)}>
            <SignaturePreview paths={item.paths} /><span>{item.name}</span>
          </button>
        </li>)}</ul>
      </aside>
      <div className="signature-editor">
        <div className="signature-paper" hidden={!!selected}>
          <canvas ref={canvasRef} width={360} height={140} aria-label="Signature drawing area"
            onPointerDown={begin} onPointerMove={move} onPointerUp={end} onPointerCancel={end} onLostPointerCapture={end} />
          <span>Sign above the line</span>
        </div>
        {selected && <div className="signature-selected-preview"><SignaturePreview paths={selected.paths} label="Selected signature preview" /></div>}
        <div className="signature-name-field">
          <label htmlFor={nameId}>Signature name</label>
          <input id={nameId} maxLength={80} value={selected ? renameName : draftName} placeholder="For example, Everyday" onChange={event => selected ? setRenameName(event.target.value) : setDraftName(event.target.value)} />
          <div className="signature-library-actions">
            {selected ? <><button className="button" disabled={!renameName.trim() || renameName.trim() === selected.name} onClick={rename}>Rename</button><button className="button subtle" aria-label="Delete saved signature" onClick={remove}><Trash2 size={15} /> Delete</button></>
              : <button className="button" disabled={!paths.length || !draftName.trim()} onClick={save}><Save size={15} /> Save to library</button>}
          </div>
        </div>
        <p className="signature-local-note">These are visual signatures, not certificate-based digital signatures.</p>
      </div>
    </div>
    {error && <div className="signature-library-error"><p role="alert">{error}</p>
      {libraryError && !resetConfirm && <button className="button" onClick={() => setResetConfirm(true)}>Reset saved library</button>}
      {libraryError && resetConfirm && <div className="signature-reset-confirm"><p>Remove every saved signature from this device? This cannot be undone.</p><button className="button" onClick={() => setResetConfirm(false)}>Keep library</button><button className="button" onClick={reset}>Remove saved library</button></div>}
    </div>}
    {notice && <p className="signature-library-notice" role="status">{notice}</p>}
    <footer>
      {!selected ? <button className="button subtle" onClick={clear}><Eraser size={16} /> Clear</button> : <span />}
      <div className="dialog-actions"><button ref={cancelRef} className="button" onClick={onCancel}>Cancel</button><button className="button primary" disabled={!acceptedPaths.length} onClick={() => onAccept(acceptedPaths.map(path => path.map(value => ({ ...value }))))}>Use signature</button></div>
    </footer>
  </Modal>;
}
