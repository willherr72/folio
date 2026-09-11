import { useEffect, useId, useRef, useState, type KeyboardEvent } from "react";
import { Check, LoaderCircle, Search, Type, Upload } from "lucide-react";
import { Modal } from "./Modal";
import { acquireFontInOrder, customTextError, ensureCustomFont, flushFontReleases, fontApi, fontFamily, holdCustomFont, type FontInfo, type InstalledFont } from "../editor/custom-fonts";
import "./release-dialogs.css";
import "./font-picker.css";

interface FontPickerProps {
  text: string;
  title?: string;
  description?: string;
  currentFontId?: string;
  onChoose(info: FontInfo): void;
  onClose(): void;
}
type Choice = { kind: "installed"; font: InstalledFont } | { kind: "import" } | { kind: "current"; id: string };
interface TemporaryFont { info: FontInfo; releaseLease(releaseIfUnowned?: boolean): Promise<void>; transferred?: boolean; release?: Promise<void> }
function dispose(font: TemporaryFont | null) {
  if (!font || font.transferred) return Promise.resolve();
  return font.release ??= font.releaseLease();
}
function styleName(info: FontInfo) {
  const weight = info.weight >= 800 ? "Extra bold" : info.weight >= 700 ? "Bold" : info.weight >= 600 ? "Semibold" : info.weight >= 500 ? "Medium" : info.weight <= 300 ? "Light" : "Regular";
  return `${weight}${info.italic ? " · Italic" : ""}`;
}

export function FontPicker({ text, title = "Choose a font", description = "Preview an installed font or import your own. The selected font is included when you save your PDF.", currentFontId, onChoose, onClose }: FontPickerProps) {
  const [fonts, setFonts] = useState<InstalledFont[] | null>(null);
  const [query, setQuery] = useState("");
  const [busy, setBusy] = useState(false);
  const [selected, setSelected] = useState<FontInfo | null>(null);
  const [selectedInstalledId, setSelectedInstalledId] = useState<string | null>(null);
  const [focusedId, setFocusedId] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [catalogError, setCatalogError] = useState<string | null>(null);
  const [applied, setApplied] = useState(false);
  const active = useRef(false), running = useRef(false), applying = useRef(false), version = useRef(0);
  const pending = useRef<(Choice & { version: number }) | null>(null);
  const previewFont = useRef<TemporaryFont | null>(null), loadingFont = useRef<TemporaryFont | null>(null);
  const searchRef = useRef<HTMLInputElement>(null), listRef = useRef<HTMLUListElement>(null);
  const latestText = useRef(text);
  latestText.current = text;
  const labelId = useId();

  const discard = () => {
    const previous = previewFont.current;
    previewFont.current = null;
    void dispose(previous);
    void dispose(loadingFont.current);
  };
  const cancel = () => {
    if (!active.current) return;
    active.current = false;
    version.current++;
    pending.current = null;
    discard();
    onClose();
  };

  // Keep one native load in flight. Rapid selections replace the queued choice;
  // stale results are released before the next font is loaded.
  const processChoices = async () => {
    if (running.current) return;
    running.current = true;
    try {
      while (active.current && pending.current) {
        const choice = pending.current;
        pending.current = null;
        await acquireFontInOrder(async () => {
          let owned: TemporaryFont | null = null;
          const isCurrent = () => active.current && choice.version === version.current;
          try {
            await flushFontReleases();
            if (!isCurrent()) return;
            const info = choice.kind === "installed" ? await fontApi.loadInstalled(choice.font.id)
              : choice.kind === "current" ? await fontApi.info(choice.id) : await fontApi.importFont();
            if (!info) return;
            owned = { info, releaseLease: holdCustomFont(info.id) };
            if (!isCurrent()) return;
            loadingFont.current = owned;
            const problem = customTextError(info, latestText.current);
            if (problem) throw new Error(problem);
            await ensureCustomFont(info.id, info);
            if (!isCurrent()) return;
            previewFont.current = owned;
            loadingFont.current = null;
            setSelected(info);
          } catch (error) {
            if (isCurrent()) setError(error instanceof Error ? error.message : String(error));
          } finally {
            if (loadingFont.current === owned) loadingFont.current = null;
            if (previewFont.current !== owned) await dispose(owned);
            if (isCurrent()) setBusy(false);
          }
        });
      }
    } finally {
      running.current = false;
    }
  };
  const choose = (choice: Choice) => {
    if (!active.current || applying.current) return;
    if (choice.kind === "installed" && selectedInstalledId === choice.font.id && previewFont.current) return;
    discard();
    pending.current = { ...choice, version: ++version.current };
    setSelected(null);
    setSelectedInstalledId(choice.kind === "installed" ? choice.font.id : null);
    setBusy(true);
    setError(null);
    void processChoices();
  };

  useEffect(() => {
    active.current = true;
    let cancelled = false;
    void fontApi.listInstalled().then(value => { if (!cancelled) setFonts(value); }).catch(error => {
      if (!cancelled) { setFonts([]); setCatalogError(error instanceof Error ? error.message : String(error)); }
    });
    return () => {
      active.current = false;
      cancelled = true;
      version.current++;
      pending.current = null;
      discard();
    };
  }, []);
  useEffect(() => {
    if (currentFontId) choose({ kind: "current", id: currentFontId });
  }, [currentFontId]);

  const problem = selected ? customTextError(selected, text) : null;
  const apply = () => {
    const owned = previewFont.current;
    if (!active.current || applying.current || busy || !owned || problem) return;
    // Transfer before calling the parent, which may synchronously unmount us.
    applying.current = true;
    owned.transferred = true;
    previewFont.current = null;
    try {
      onChoose(owned.info);
      void owned.releaseLease(false);
      if (active.current) setApplied(true);
    } catch (error) {
      owned.transferred = false;
      applying.current = false;
      if (active.current) {
        previewFont.current = owned;
        setError(error instanceof Error ? error.message : String(error));
      } else void dispose(owned);
    }
  };
  const visible = fonts?.filter(font => font.name.toLocaleLowerCase().includes(query.trim().toLocaleLowerCase()));
  const tabStop = visible?.find(font => font.supported && font.id === focusedId)?.id ?? visible?.find(font => font.supported)?.id;
  const navigateFonts = (event: KeyboardEvent<HTMLUListElement>) => {
    if (!["ArrowDown", "ArrowUp", "Home", "End"].includes(event.key)) return;
    const buttons = Array.from(listRef.current?.querySelectorAll<HTMLButtonElement>("button:not(:disabled)") ?? []);
    if (!buttons.length) return;
    event.preventDefault();
    const index = buttons.indexOf(document.activeElement as HTMLButtonElement);
    const next = event.key === "Home" ? 0 : event.key === "End" ? buttons.length - 1
      : Math.max(0, Math.min(buttons.length - 1, index + (event.key === "ArrowDown" ? 1 : -1)));
    buttons[next].focus();
  };

  return <Modal title={title} description={description} onClose={cancel} initialFocusRef={searchRef} className="release-dialog font-picker">
    <div className="font-picker-body">
      <label className="font-picker-search">Search fonts
        <span><Search size={16} aria-hidden="true"/><input ref={searchRef} type="search" value={query} placeholder="Search installed fonts" onChange={event => setQuery(event.target.value)}/></span>
      </label>
      <div className="font-picker-columns">
        <div className="font-picker-catalog">
          <div className="font-picker-section-heading"><h3 id={`${labelId}-list`}>Installed fonts</h3>{fonts && <span>{visible?.length} of {fonts.length}</span>}</div>
          <ul ref={listRef} className="font-picker-list" aria-labelledby={`${labelId}-list`} aria-busy={fonts === null} onKeyDown={navigateFonts}>
            {fonts === null ? <li className="font-picker-empty" role="status">Finding installed fonts…</li>
              : visible?.length ? visible.map((font, index) => <li className="font-picker-item" key={font.id}>
                <button type="button" disabled={!font.supported || applied} aria-pressed={selectedInstalledId === font.id}
                  aria-describedby={!font.supported ? `${labelId}-reason-${index}` : undefined} tabIndex={font.id === tabStop ? 0 : -1}
                  onFocus={() => setFocusedId(font.id)} onClick={() => choose({ kind: "installed", font })}>
                  <span>{font.name}</span>{selectedInstalledId === font.id && <Check size={15} aria-hidden="true"/>}
                </button>
                {!font.supported && <p id={`${labelId}-reason-${index}`}>{font.reason ?? "This font format is not supported yet."}</p>}
              </li>) : <li className="font-picker-empty">{catalogError ? "Installed fonts could not be loaded. You can still import a font file." : "No matching fonts. Try another name or import a font file."}</li>}
          </ul>
        </div>
        <div className="font-picker-preview-column">
          <div className="font-picker-section-heading"><h3>Preview</h3><span>Your text</span></div>
          <div className={`font-picker-preview-card${selected && !problem ? " has-font" : ""}`} aria-busy={busy}>
            {selected && !problem ? <>
              <div className="font-picker-face-info"><strong>{selected.name}</strong><span>{styleName(selected)}</span></div>
              <div className="font-picker-sample" role="region" aria-label="Font preview" style={{ fontFamily: fontFamily(selected.id), fontKerning: "none", fontVariantLigatures: "none", fontFeatureSettings: '"kern" 0, "liga" 0, "clig" 0', fontSynthesis: "none" }}>{text}</div>
            </> : <div className="font-picker-placeholder">
              {busy ? <LoaderCircle size={26} className="font-picker-spinner" aria-hidden="true"/> : <Type size={28} aria-hidden="true"/>}
              <strong>{busy ? "Preparing preview…" : error || problem ? "Preview unavailable" : "Find the right font"}</strong>
              <p role={busy ? "status" : undefined}>{busy ? "Loading your selected font." : "Select a font to see your text here."}</p>
            </div>}
          </div>
        </div>
      </div>
      {(error || problem) && <p className="error-note font-picker-error" role="alert">{error || problem}</p>}
      {catalogError && <p className="error-note font-picker-error" role="alert">{catalogError}</p>}
    </div>
    <footer className="dialog-actions font-picker-actions">
      <button type="button" className="button font-picker-import" disabled={busy || applied} onClick={() => choose({ kind: "import" })}><Upload size={14} aria-hidden="true"/>Import font…</button>
      <button type="button" className="button" onClick={cancel}>Cancel</button>
      <button type="button" className="button primary" disabled={busy || !selected || !!problem || applied} onClick={apply}>Apply font</button>
    </footer>
  </Modal>;
}
