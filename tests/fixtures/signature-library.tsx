import React, {useState} from "react";
import {createRoot} from "react-dom/client";
import {SignaturePad} from "../../src/components/SignaturePad";
import "../../src/styles.css";
function Example(){
 const [open,setOpen]=useState(false);const [accepted,setAccepted]=useState("");
 return <><button onClick={()=>setOpen(true)}>Open signature library</button><output>{accepted}</output>{open&&<SignaturePad onCancel={()=>setOpen(false)} onAccept={paths=>{setAccepted(JSON.stringify(paths));setOpen(false);}}/>}</>;
}
createRoot(document.getElementById("root")!).render(<Example/>);
