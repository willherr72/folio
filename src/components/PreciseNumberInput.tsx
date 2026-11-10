import {useState,type ComponentProps} from "react";
import {flushSync} from "react-dom";

type Props=Omit<ComponentProps<"input">,"type"|"value">&{value:string};

function idleValue(value:string,min:Props["min"],max:Props["max"],step:Props["step"]):string {
 const number=Number(value);
 if(!value.trim()||!Number.isFinite(number)||step!=="any")return value;
 const inRange=(n:number)=>(min===undefined||n>=Number(min))&&(max===undefined||n<=Number(max));
 if(!inRange(number))return value;
 let rounded=Number(number.toFixed(3));
 // A small but meaningful offset must not appear to be zero.
 if(rounded===0&&number!==0)rounded=Number(number.toPrecision(3));
 return inRange(rounded)?String(rounded):value;
}

/** Compact idle presentation only. The owner retains the exact numeric draft;
 * focus reveals it, and focus/blur never synthesize an edit. */
export function PreciseNumberInput({value,min,max,step="any",onFocus,onBlur,...props}:Props){
 const [focused,setFocused]=useState(false);
 return <input {...props} type="number" min={min} max={max} step={step}
  value={focused?value:idleValue(value,min,max,step)}
  // Native focus/select/replace can happen in one task. Expand before focus
  // returns, so a deferred value update cannot reset the browser's selection.
  onFocus={event=>{flushSync(()=>setFocused(true));onFocus?.(event);}}
  onBlur={event=>{setFocused(false);onBlur?.(event);}}/>;
}
