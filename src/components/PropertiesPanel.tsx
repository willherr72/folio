import {createContext,useCallback,useContext,useId,useState,type ReactNode} from "react";
import {ChevronDown,ChevronUp} from "lucide-react";

const RevealProperties=createContext<()=>void>(()=>{});
export const usePropertiesReveal=()=>useContext(RevealProperties);

/** One persistent editor subtree, placed beside or below the page by CSS. */
export function PropertiesPanel({contextKey,autoExpand,children}:{contextKey:string;autoExpand:boolean;children:ReactNode}) {
  const contentId=useId();
  const [choice,setChoice]=useState({key:contextKey,expanded:autoExpand});
  const expanded=choice.key===contextKey?choice.expanded:autoExpand;
  // Expire a manual collapse on context exit without remounting editor children.
  // Adjust during render so child focus effects see the new visibility immediately.
  if(choice.key!==contextKey)setChoice({key:contextKey,expanded:autoExpand});
  const expand=useCallback(()=>setChoice(previous=>previous.key===contextKey&&previous.expanded?previous:{key:contextKey,expanded:true}),[contextKey]);
  return <RevealProperties.Provider value={expand}><aside className={`properties ${expanded?"is-expanded":"is-collapsed"}`} aria-label="Properties">
    <div className="panel-heading properties-heading"><span>PROPERTIES</span>
      <button className="properties-toggle" type="button" aria-label="Properties" aria-controls={contentId} aria-expanded={expanded}
        onClick={()=>setChoice({key:contextKey,expanded:!expanded})}>
        <span>Properties</span><span>{expanded?"Hide":"Show"}</span>{expanded?<ChevronDown size={16}/>:<ChevronUp size={16}/>}</button>
    </div>
    <div id={contentId} className="properties-body" onFocusCapture={event=>{if(event.currentTarget.contains(event.target))expand();}}>{children}</div>
  </aside></RevealProperties.Provider>;
}
