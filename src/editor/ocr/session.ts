import {normalizeOcrResult, validateOcrRaster} from './result';
import type {OcrLanguage, OcrPage, OcrProgress, OcrRaster, OcrSession, OcrSessionOptions} from './types';

const DEADLINE_MS = 120_000;
const aborted = () => new DOMException('OCR canceled.','AbortError');
interface Pending {
  id: string;
  action: string;
  resolve(value: unknown): void;
  reject(error: Error): void;
}

/** Pinned Tesseract.js6 worker protocol; retain termination ownership during initialization. */
export async function startOcrSession(languages: readonly OcrLanguage[], options: OcrSessionOptions = {}): Promise<OcrSession> {
  if (options.signal?.aborted) throw aborted();
  if (!Array.isArray(languages) || languages.length < 1 || languages.length > 3
    || new Set(languages).size !== languages.length
    || languages.some(language => !['eng','fra','spa'].includes(language))) {
    throw new Error('Choose one to three distinct supported OCR languages.');
  }
  const selected = [...languages];
  const base = new URL('/ocr/',window.location.href);
  const worker = new Worker(new URL('worker.js',base),{name:'folio-ocr'});
  let failure: Error | undefined, pending: Pending | undefined, sequence = 0, busy = false;
  let stage: OcrProgress['stage'] = 'initializing';
  let timer: ReturnType<typeof setTimeout> | undefined;
  let removePageAbort: (() => void) | undefined;
  const onAbort = () => close(aborted());
  function clearDeadline(): void {
    if (timer !== undefined) clearTimeout(timer);
    timer = undefined;
  }
  function close(error: Error): void {
    if (failure) return;
    failure = error;
    clearDeadline();
    options.signal?.removeEventListener('abort',onAbort);
    removePageAbort?.(); removePageAbort = undefined;
    worker.onmessage = null; worker.onerror = null; worker.onmessageerror = null;
    worker.terminate();
    const job = pending; pending = undefined;
    job?.reject(error);
  }
  function report(fraction: number): void {
    if (!failure && Number.isFinite(fraction) && fraction >= 0 && fraction <= 1) {
      try { options.onProgress?.({stage,fraction}); } catch { /* An observer cannot strand native work. */ }
    }
  }
  function deadline(): void {
    clearDeadline();
    timer = setTimeout(() => close(new Error(`OCR ${stage} timed out. Try fewer languages or a smaller page.`)),DEADLINE_MS);
  }
  function request(action: string, payload: unknown): Promise<unknown> {
    if (failure) return Promise.reject(failure);
    return new Promise((resolve,reject) => {
      const id = String(++sequence);
      pending = {id,action,resolve,reject};
      try { worker.postMessage({workerId:'folio-ocr',jobId:id,action,payload}); }
      catch (error) { close(error instanceof Error ? error : new Error('Could not send the OCR job.')); }
    });
  }
  worker.onmessage = event => {
    if (failure) return;
    const packet: unknown = event.data;
    if (!packet || typeof packet !== 'object') { close(new Error('Invalid OCR worker reply.')); return; }
    const reply = packet as Record<string,unknown>;
    if (!pending || reply.jobId !== pending.id) return; // A completed/aborted job cannot settle another one.
    if (reply.workerId !== 'folio-ocr' || reply.action !== pending.action) { close(new Error('Invalid OCR worker reply.')); return; }
    if (reply.status === 'progress') {
      if (stage === 'recognizing' && reply.data && typeof reply.data === 'object') {
        const fraction = (reply.data as Record<string,unknown>).progress;
        if (typeof fraction === 'number') report(fraction);
      }
      return;
    }
    if (reply.status === 'reject') { close(new Error(`OCR failed: ${String(reply.data).slice(0,500)}`)); return; }
    if (reply.status !== 'resolve') { close(new Error('Invalid OCR worker status.')); return; }
    const job = pending; pending = undefined; job.resolve(reply.data);
  };
  worker.onerror = event => { event.preventDefault(); close(new Error(`OCR worker failed: ${event.message || 'unknown error'}`)); };
  worker.onmessageerror = () => close(new Error('Could not read the OCR worker reply.'));
  options.signal?.addEventListener('abort',onAbort,{once:true});
  deadline();
  try {
    await request('load',{options:{lstmOnly:true,corePath:new URL('core/',base).href,logging:false}});
    report(1/3);
    await request('loadLanguage',{langs:selected,options:{langPath:new URL('models/',base).href,gzip:false,cacheMethod:'none',lstmOnly:true}});
    report(2/3);
    await request('initialize',{langs:selected,oem:1,config:{}});
    if (failure) throw failure;
    clearDeadline(); report(1);
    if (failure) throw failure;
  } catch (error) {
    close(error instanceof Error ? error : new Error('OCR initialization failed.'));
    throw failure;
  }
  return {
    async recognize(raster: OcrRaster, signal?: AbortSignal): Promise<OcrPage> {
      if (failure) throw failure;
      if (busy) throw new Error('An OCR page is already being recognized.');
      if (signal?.aborted) { close(aborted()); throw failure; }
      // Take ownership before the first await. Callers retain their buffer and may
      // reuse the raster object while this page is pending.
      const admitted = {png:raster.png,width:raster.width,height:raster.height,dpi:raster.dpi};
      validateOcrRaster(admitted);
      const length = admitted.png.length;
      if (length > 32 * 1024 * 1024) throw new Error('Invalid OCR data: PNG byte limit.');
      const png = new Uint8Array(length);
      png.set(admitted.png.subarray(0,length));
      const input = {...admitted,png};
      // A shared backing buffer can change during the copy. Validate the owned
      // snapshot as well; subsequent submission and geometry use only this data.
      validateOcrRaster(input);
      busy = true; stage = 'recognizing';
      signal?.addEventListener('abort',onAbort,{once:true});
      removePageAbort = () => signal?.removeEventListener('abort',onAbort);
      deadline();
      try {
        await request('setParameters',{params:{user_defined_dpi:String(input.dpi)}});
        const raw = await request('recognize',{image:input.png,options:{rotateAuto:false},output:{text:true,blocks:true}});
        if (failure) throw failure;
        const result = normalizeOcrResult(raw,input.width,input.height);
        report(1);
        if (failure) throw failure;
        return result;
      } catch (error) {
        close(error instanceof Error ? error : new Error('OCR recognition failed.'));
        throw failure;
      } finally {
        busy = false; clearDeadline(); removePageAbort?.(); removePageAbort = undefined;
      }
    },
    dispose: () => close(aborted()),
  };
}
