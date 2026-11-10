// Real App, isolated synthetic native services. Does not read or write PDFs.
import {useEffect} from 'react';
import {createRoot} from 'react-dom/client';
import {App} from '../../src/App';
import {nativeAdapter} from '../../src/editor/adapter';
import {DEFAULT_PREFERENCES,PREFERENCES_KEY} from '../../src/editor/preferences';
import type {DocumentInfo} from '../../src/editor/types';
import '../../src/styles.css';
localStorage.setItem(PREFERENCES_KEY,JSON.stringify({...DEFAULT_PREFERENCES,theme:'light'}));
const short=new URLSearchParams(location.search).has("short");
let count=0;
nativeAdapter.openPdf=async()=>{count++;return {id:`navigation-fixture-${count}`,name:count===1?'Navigation fixture.pdf':'Other fixture.pdf',pages:Array.from({length:4},(_,index)=>({width:800,height:short?100:1200,...(index===1?{overlays:[{type:'comment',id:'note',x:80,y:800,color:'#FFE066',text:'Review this paragraph'}]}:{})})),bookmarks:[{id:'ending',title:'Ending',level:0,destination:{kind:'array',value:[{kind:'page',value:'3'},{kind:'name',value:[70,105,116]}]}}]} as DocumentInfo;};
nativeAdapter.renderPage=async(_source,page)=>'data:image/svg+xml,'+encodeURIComponent(`<svg xmlns="http://www.w3.org/2000/svg" width="800" height="1200"><rect width="800" height="1200" fill="white"/><text x="60" y="80" font-size="25">Navigation fixture — Page ${page+1}</text>${Array.from({length:10},(_,line)=>`<text x="60" y="${150+line*90}" font-size="15">Line ${line+1}: return here after following another destination.</text>`).join('')}</svg>`);
nativeAdapter.engineStatus=async()=> 'Synthetic navigation acceptance service';
nativeAdapter.closeDocument=async()=>{};
nativeAdapter.getPageText=async()=>({characters:[...'Needle'].map((text,i)=>({text,x:60+i*8,y:600,width:8,height:18}))});
nativeAdapter.listTextRuns=async()=>({runs:[]});nativeAdapter.listOccurrenceFormTextRuns=undefined;nativeAdapter.listFormTextRuns=async()=>({runs:[]});
function Fixture(){useEffect(()=>{(document.querySelector('.empty-actions button') as HTMLButtonElement|null)?.click();},[]);return <App initialDemo={false}/>;}
createRoot(document.getElementById('root')!).render(<Fixture/>);
