import { useState } from "react";
import { createRoot } from "react-dom/client";
import { FontPicker } from "../../src/components/FontPicker";
import { fontApi, retainCustomFonts, type FontInfo, type InstalledFont } from "../../src/editor/custom-fonts";
import "../../src/styles.css";

const params = new URLSearchParams(location.search);
document.documentElement.dataset.theme = params.get("theme") ?? "light";
const names = ["Arial", "Arial Bold", "Calibri", "Cambria Bold", "DejaVu Serif", "Georgia", "Segoe UI", "Times New Roman"];
const infos = names.map((name, index): FontInfo => ({ id: String(index + 1).repeat(64), name, weight: name.includes("Bold") ? 700 : 400, italic: false, coverage: [[32, 126], [233, 233]] }));
const catalog: InstalledFont[] = [...infos.map((info, index) => ({ id: `installed-${index}`, name: info.name, supported: true })),
  { id: "restricted", name: "Restricted Sans", supported: false, reason: "This font does not allow editable embedding." },
  { id: "variable", name: "Variable Sans", supported: false, reason: "Choose a static version of this font." }];
const log = { loaded: [] as string[], released: [] as string[], bytes: [] as string[] };
Object.assign(window, { fontPickerLog: log });
fontApi.listInstalled = async () => catalog;
fontApi.loadInstalled = async id => {
  log.loaded.push(id);
  await new Promise(resolve => setTimeout(resolve, params.has("slow") ? 450 : 120));
  return infos[Number(id.replace("installed-", ""))];
};
fontApi.info = async id => infos.find(info => info.id === id)!;
fontApi.importFont = async () => infos[4];
fontApi.bytes = async id => {
  log.bytes.push(id);
  const font = infos.find(info => info.id === id)!;
  return (await fetch(font.name === "DejaVu Serif" ? "/tests/fixtures/corpus/fonts/DejaVuSerif.ttf" : `/__font_picker_fonts__/${encodeURIComponent(font.name)}.ttf`)).arrayBuffer();
};
fontApi.release = async id => { log.released.push(id); };

function Fixture() {
  const [open, setOpen] = useState(false), [chosen, setChosen] = useState<FontInfo | null>(null);
  return <main style={{ padding: 32 }}>
    <button className="button" onClick={() => setOpen(true)}>More fonts…</button>
    <output aria-label="Applied font">{chosen?.name ?? "None"}</output>
    {open && <FontPicker text={params.has("long") ? "A thoughtful choice for a document that needs a very long sentence and multiple lines.\nNumbers 1234567890 and café." : "A considered choice.\nThe quick brown fox, 123."}
      currentFontId={chosen?.id} onClose={() => setOpen(false)} onChoose={info => { retainCustomFonts(new Set([info.id])); setChosen(info); setOpen(false); }}/>}
  </main>;
}
createRoot(document.getElementById("root")!).render(<Fixture/>);
