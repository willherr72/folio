import { useEffect, useRef, useState } from "react";
import { Modal } from "./Modal";
import { customTextError, ensureCustomFont, flushFontReleases, fontApi, releaseUnownedFont, type FontInfo, type InstalledFont } from "../editor/custom-fonts";
import "./font-picker.css";

export function FontPicker({ text, onChoose, onClose }: { text: string; onChoose(info: FontInfo): void; onClose(): void }) {
  const [fonts, setFonts] = useState<InstalledFont[] | null>(null);
  const [query, setQuery] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const active = useRef(true), locked = useRef(false);
  useEffect(() => {
    active.current = true;
    let cancelled = false;
    void fontApi.listInstalled().then(value => { if (!cancelled) setFonts(value); }).catch(error => {
      if (!cancelled) { setFonts([]); setError(String(error)); }
    });
    return () => { active.current = false; cancelled = true; };
  }, []);
  const choose = async (installed?: InstalledFont) => {
    if (locked.current) return;
    locked.current = true; setBusy(true); setError(null);
    let selected: FontInfo | null = null, transferred = false;
    try {
      await flushFontReleases();
      if (!active.current) return;
      selected = installed ? await fontApi.loadInstalled(installed.id) : await fontApi.importFont();
      if (!selected || !active.current) return;
      const problem = customTextError(selected, text);
      if (problem) throw new Error(problem);
      await ensureCustomFont(selected.id, selected);
      if (!active.current) return;
      onChoose(selected); transferred = true;
    } catch (error) {
      if (active.current) setError(error instanceof Error ? error.message : String(error));
    } finally {
      if (selected && !transferred) await releaseUnownedFont(selected.id);
      locked.current = false;
      if (active.current) setBusy(false);
    }
  };
  const visible = fonts?.filter(font => font.name.toLocaleLowerCase().includes(query.toLocaleLowerCase()));
  return <Modal title="Choose a font" onClose={() => { if (!locked.current) onClose(); }} className="font-picker">
    <p>Use an installed font or import a font file. Folio includes the selected font when saving your PDF.</p>
    <label className="field-label">Search fonts<input type="search" value={query} disabled={busy} onChange={event => setQuery(event.target.value)}/></label>
    <div className="font-picker-list" aria-busy={busy || fonts === null}>
      {fonts === null ? <p role="status">Finding installed fonts…</p> : visible?.length ? visible.map(font => <div className="font-picker-item" key={font.id}>
        <button type="button" disabled={busy || !font.supported} onClick={() => void choose(font)}>{font.name}</button>
        {!font.supported && <p>{font.reason ?? "This font format is not supported yet."}</p>}
      </div>) : <p>No matching fonts. You can import a local font file.</p>}
    </div>
    {busy && <p role="status">Loading font…</p>}
    {error && <p className="error-note" role="alert">{error}</p>}
    <footer className="dialog-actions"><button className="button" disabled={busy} onClick={onClose}>Cancel</button><button className="button primary" disabled={busy} onClick={() => void choose()}>Import font…</button></footer>
  </Modal>;
}
