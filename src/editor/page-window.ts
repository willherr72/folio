export interface PageWindowMetrics{columns:number;itemWidth:number;itemHeight:number;gap:number;paddingTop:number;height:number}
export function pageWindow(count:number,m:PageWindowMetrics,scrollTop:number,retained:number[],overscan=2):{indices:number[];totalHeight:number}{
 const rows=Math.ceil(count/m.columns),stride=m.itemHeight+m.gap,totalHeight=Math.max(0,rows*stride-m.gap);
 if(!count)return {indices:[],totalHeight:0};
 const top=Math.max(0,Math.min(scrollTop,totalHeight+m.paddingTop-Math.max(0,m.height))),first=Math.max(0,Math.floor((top-m.paddingTop)/stride));
 const last=Math.min(rows,Math.max(first+1,Math.ceil((top+Math.max(0,m.height)-m.paddingTop)/stride)));
 const keep=new Set<number>();
 for(let row=Math.max(0,first-overscan);row<Math.min(rows,last+overscan);row++)keep.add(row);
 for(const index of retained)if(Number.isInteger(index)&&index>=0&&index<count)keep.add(Math.floor(index/m.columns));
 const indices:number[]=[];for(const row of [...keep].sort((a,b)=>a-b))for(let col=0;col<m.columns;col++){const index=row*m.columns+col;if(index<count)indices.push(index);}
 return {indices,totalHeight};
}
export function revealPage(index:number,m:PageWindowMetrics,scrollTop:number):number{
 const top=m.paddingTop+Math.floor(index/m.columns)*(m.itemHeight+m.gap),bottom=top+m.itemHeight;
 if(top<scrollTop)return Math.max(0,top);
 if(bottom>scrollTop+m.height)return Math.max(0,Math.min(top,bottom-m.height));
 return Math.max(0,scrollTop);
}
export function anchorPageWindow(scrollTop:number,before:PageWindowMetrics,after:PageWindowMetrics):number{
 if(scrollTop<=before.paddingTop)return Math.max(0,scrollTop);
 const offset=scrollTop-before.paddingTop,stride=before.itemHeight+before.gap,row=Math.floor(offset/stride),page=row*before.columns;
 return after.paddingTop+Math.floor(page/after.columns)*(after.itemHeight+after.gap)+Math.min(offset-row*stride,after.itemHeight+after.gap-1);
}
