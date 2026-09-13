import {createRoot} from "react-dom/client";
import {TextOverlayPresentation} from "../../src/components/TextOverlayPresentation";
import {shapedTextApi} from "../../src/editor/shaped-text";
import {fontApi,retainCustomFonts} from "../../src/editor/custom-fonts";
const params=new URLSearchParams(location.search),stem=params.get("case")!,zoom=Number(params.get("zoom")??150);
const base="/artifacts/shaped-text/editor-native/";
const {overlay,result}=await (await fetch(`${base}${encodeURIComponent(stem)}.prepared.json`)).json();
shapedTextApi.prepare=async()=>result;fontApi.release=async()=>{};retainCustomFonts(new Set([overlay.fontId]));
createRoot(document.getElementById("root")!).render(<><h1>{stem} — {zoom}%</h1><div className="comparison"><figure><figcaption>Editor native text box</figcaption><div id="native-preview"><svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 400 400" width={400*zoom/100} height={400*zoom/100}><TextOverlayPresentation overlay={overlay}/></svg></div></figure><figure><figcaption>Exported PDF</figcaption><img id="pdfium-preview" src={`${base}${stem}.preview-${zoom}.png`}/></figure></div></>);
