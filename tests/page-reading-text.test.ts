import {afterEach,expect,it,vi} from 'vitest';
import {createDemoAdapter} from '../src/editor/adapter';
import {clearPageReadingTextCache,readPageReadingText} from '../src/editor/page-reading-text';
const source='compact-cache';
const engine=()=>({...createDemoAdapter(),getPageReadingText:vi.fn(async()=> 'ff𝑴q\u0307\u0323 \r\n'),getPageText:vi.fn(async()=>({characters:[]}))});
afterEach(()=>clearPageReadingTextCache([source]));
it('deduplicates exact string requests without requesting rich geometry',async()=>{
 const adapter=engine();const a=readPageReadingText(adapter,source,0),b=readPageReadingText(adapter,source,0);
 expect(a).toBe(b);expect(await a).toBe('ff𝑴q\u0307\u0323 \r\n');
 expect(adapter.getPageReadingText).toHaveBeenCalledTimes(1);expect(adapter.getPageText).not.toHaveBeenCalled();
});
it('isolates adapter and provider identities and retries failures',async()=>{
 const a=engine(),b=engine();a.getPageReadingText.mockResolvedValue('A');b.getPageReadingText.mockResolvedValue('B');
 expect(await readPageReadingText(a,source,0)).toBe('A');expect(await readPageReadingText(b,source,0)).toBe('B');
 b.getPageReadingText=vi.fn(async()=> 'replacement');expect(await readPageReadingText(b,source,0)).toBe('replacement');
 b.getPageReadingText.mockRejectedValueOnce(Error('unavailable'));await expect(readPageReadingText(b,source,1)).rejects.toThrow('unavailable');
 expect(await readPageReadingText(b,source,1)).toBe('replacement');
});
it('never lets a cleared pending response replace a reopened source entry',async()=>{
 const a=engine();let done!:(text:string)=>void;a.getPageReadingText.mockImplementationOnce(()=>new Promise(resolve=>done=resolve));
 const old=readPageReadingText(a,source,0);await Promise.resolve();clearPageReadingTextCache([source]);
 a.getPageReadingText.mockResolvedValue('reopened');const fresh=readPageReadingText(a,source,0);expect(await fresh).toBe('reopened');
 done('old');expect(await old).toBe('old');expect(readPageReadingText(a,source,0)).toBe(fresh);
});
it('bounds completed strings and pending pages without truncating responses',async()=>{
 const a=engine();a.getPageReadingText.mockResolvedValue('𝑴'.repeat(125001));
 const large=await readPageReadingText(a,source,0);expect(large.length).toBe(250002);
 await readPageReadingText(a,source,0);expect(a.getPageReadingText).toHaveBeenCalledTimes(2);
 a.getPageReadingText.mockResolvedValue('small');
 for(let i=1;i<=61;i++)await readPageReadingText(a,source,i);
 await readPageReadingText(a,source,1);expect(a.getPageReadingText).toHaveBeenCalledTimes(64);
});

it('bounds pending requests and prevents late evicted completions from re-entering',async()=>{
 const a=engine();const done:Array<(text:string)=>void>=[];
 a.getPageReadingText.mockImplementation(()=>new Promise(resolve=>done.push(resolve)));
 const requests=Array.from({length:61},(_,i)=>readPageReadingText(a,source,i));
 await Promise.resolve();expect(a.getPageReadingText).toHaveBeenCalledTimes(61);
 done[0]('evicted');expect(await requests[0]).toBe('evicted');
 const reopened=readPageReadingText(a,source,0);await Promise.resolve();
 expect(a.getPageReadingText).toHaveBeenCalledTimes(62);expect(reopened).not.toBe(requests[0]);
 done.slice(1).forEach(resolve=>resolve('settled'));await Promise.all([...requests.slice(1),reopened]);
});
