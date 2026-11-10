/* Folio wrapper for the pinned Tesseract.js worker. No runtime remote fallback. */
(() => {
  'use strict';
  const base=new URL('./',self.location.href);
  const files=new Set(['upstream/worker.min.js','core/tesseract-core-lstm.wasm.js',
    'core/tesseract-core-simd-lstm.wasm.js','core/tesseract-core-lstm.wasm',
    'core/tesseract-core-simd-lstm.wasm','models/eng.traineddata','models/fra.traineddata',
    'models/spa.traineddata']);
  const allowed=(input,embedded=false)=>{
    const url=new URL(input,base);
    // Emscripten's pinned .wasm.js includes its own WASM as an inline data URL.
    if(embedded&&url.href.startsWith('data:application/octet-stream;base64,'))return url.href;
    if(url.origin!==base.origin||url.search||url.hash||!url.pathname.startsWith(base.pathname)
      ||!files.has(url.pathname.slice(base.pathname.length)))throw new TypeError('Only local OCR assets may be loaded.');
    return url.href;
  };
  const originalImport=self.importScripts.bind(self),originalFetch=self.fetch.bind(self);
  self.importScripts=(...urls)=>originalImport(...urls.map(url=>allowed(url)));
  self.fetch=(input,options)=>{
    const request=typeof input==='object'&&input!==null&&'url' in input?input:undefined;
    const method=options?.method??request?.method??'GET';
    if(method.toUpperCase()!=='GET')throw new TypeError('Only local OCR asset GET requests are allowed.');
    const url=allowed(request?.url??input,true);
    return originalFetch(url,{...options,method:'GET',credentials:'omit',redirect:'error'});
  };
  self.importScripts(new URL('upstream/worker.min.js',base).href);
})();
