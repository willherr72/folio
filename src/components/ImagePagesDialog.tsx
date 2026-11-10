import {useEffect,useLayoutEffect,useRef,useState} from "react";
import {Modal} from "./Modal";
import type {ImagePageOptions} from "../editor/types";
import type {BlankPagePosition} from "./BlankPageDialog";
import "./release-dialogs.css";
import "./image-pages-dialog.css";

const sizes={letter:[612,792],a4:[595.28,841.89],legal:[612,1008]} as const;
export function ImagePagesDialog({inserting,onPick,onCreate,onClose}:{
  inserting:boolean;onPick():Promise<string[]|null>;
  onCreate(paths:string[],options:ImagePageOptions,position:BlankPagePosition):Promise<boolean>;onClose():void;
}) {
  const [images,setImages]=useState<Array<{id:number;path:string}>>([]);
  const paths=images.map(image=>image.path);
  const nextImageId=useRef(0),focusAfterChange=useRef<number|null>(null);
  const rows=useRef(new Map<number,HTMLLIElement>()),addButton=useRef<HTMLButtonElement>(null);
  useLayoutEffect(()=>{
    const id=focusAfterChange.current;
    if(id===null)return;
    (id===-1?addButton.current:rows.current.get(id))?.focus();
    focusAfterChange.current=null;
  },[images]);
  const [paper,setPaper]=useState<keyof typeof sizes>("letter");
  const [orientation,setOrientation]=useState("auto");
  const [margin,setMargin]=useState(36);
  const [position,setPosition]=useState<BlankPagePosition>("after");
  const [busy,setBusy]=useState<string|null>(null),[error,setError]=useState<string|null>(null);
  const pending=useRef(false),mounted=useRef(true);
  useEffect(()=>{mounted.current=true;return()=>{mounted.current=false;};},[]);
  const close=()=>{if(!pending.current)onClose();};
  const run=async(label:string,action:()=>Promise<void>)=>{
    if(pending.current)return;
    pending.current=true;setBusy(label);setError(null);
    try{await action();}catch(e){if(mounted.current)setError(e instanceof Error?e.message:String(e));}
    finally{pending.current=false;if(mounted.current)setBusy(null);}
  };
  const pick=()=>run("Choosing images…",async()=>{
    const chosen=await onPick();if(!chosen||!mounted.current)return;
    if(paths.length+chosen.length>32)throw new Error("Choose at most 32 images per import. Remove some images or choose fewer files.");
    setImages([...images,...chosen.map(path=>({id:nextImageId.current++,path}))]);
  });
  const create=()=>run("Creating image pages…",async()=>{
    const [short,long]=sizes[paper];
    const options={width:orientation==="landscape"?long:short,height:orientation==="landscape"?short:long,margin,autoOrientation:orientation==="auto"};
    // Creating the first document replaces the empty-workspace tree. Complete
    // the parent request even if that transition remounted this dialog.
    if(await onCreate(paths,options,position))onClose();
  });
  const move=(index:number,delta:number)=>{
    focusAfterChange.current=images[index].id;
    setImages(value=>{const next=[...value];[next[index],next[index+delta]]=[next[index+delta],next[index]];return next;});
  };
  const remove=(index:number)=>{
    focusAfterChange.current=images[index+1]?.id??images[index-1]?.id??-1;
    setImages(images.filter((_,i)=>i!==index));
  };
  return <Modal className="release-dialog image-pages-dialog" title={inserting?"Insert image pages":"Images to PDF"} description="One PNG or JPEG per page, fitted without cropping. Transparency uses a white background." onClose={close}>
    <div className="image-pages-body">
      <button ref={addButton} className="button" disabled={!!busy||paths.length>=32} onClick={()=>void pick()}>Add images…</button>
      <p>{paths.length} of 32 images · 64 MiB and 32 million pixels per image · Up to 32,768 pixels wide or tall</p>
      <ol className="image-pages-list" aria-label="Image page order">{images.map(({path,id},index)=><li key={id} tabIndex={-1} ref={element=>{if(element)rows.current.set(id,element);else rows.current.delete(id);}}>
        <span title={path}>{path.split(/[\\/]/).pop()}</span>
        <button className="button" aria-label={`Move image ${index+1} up`} disabled={!!busy||index===0} onClick={()=>move(index,-1)}>↑</button>
        <button className="button" aria-label={`Move image ${index+1} down`} disabled={!!busy||index===paths.length-1} onClick={()=>move(index,1)}>↓</button>
        <button className="button" aria-label={`Remove image ${index+1}`} disabled={!!busy} onClick={()=>remove(index)}>Remove</button>
      </li>)}</ol>
      <div className="release-form">
        <label>Paper size<select disabled={!!busy} value={paper} onChange={e=>setPaper(e.target.value as typeof paper)}><option value="letter">Letter · 8.5 × 11 in</option><option value="a4">A4 · 210 × 297 mm</option><option value="legal">Legal · 8.5 × 14 in</option></select></label>
        <label>Orientation<select disabled={!!busy} value={orientation} onChange={e=>setOrientation(e.target.value)}><option value="auto">Automatic for each image</option><option value="portrait">Portrait</option><option value="landscape">Landscape</option></select></label>
        <label>Margins<select disabled={!!busy} value={margin} onChange={e=>setMargin(Number(e.target.value))}><option value="36">½ inch · 36 pt</option><option value="0">None</option></select></label>
        {inserting&&<label>Insert<select disabled={!!busy} value={position} onChange={e=>setPosition(e.target.value as BlankPagePosition)}><option value="before">Before current page</option><option value="after">After current page</option><option value="end">At end of document</option></select></label>}
      </div>
      {error&&<p className="error-note" role="alert">{error}</p>}
      {busy&&<p role="status">{busy}</p>}
    </div>
    <div className="dialog-actions"><button className="button" disabled={!!busy} onClick={close}>Cancel</button><button className="button primary" disabled={!!busy||!paths.length} onClick={()=>void create()}>{inserting?"Insert pages":"Create PDF"}</button></div>
  </Modal>;
}
