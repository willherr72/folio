import {expect,it,vi} from 'vitest';
import {invoke} from '@tauri-apps/api/core';
import {nativeAdapter,createDemoAdapter} from '../src/editor/adapter';
vi.mock('@tauri-apps/api/core',()=>({invoke:vi.fn()}));
it('maps native logical reading to the exact compact command and propagates denial',async()=>{
 expect(createDemoAdapter().getPageReadingText).toBeUndefined();
 expect(nativeAdapter.getPageReadingText).toBeTypeOf('function');
 const ipc=vi.mocked(invoke);ipc.mockResolvedValue('ff𝑴q\u0307\u0323 \r\n');
 expect(await nativeAdapter.getPageReadingText!('source',2)).toBe('ff𝑴q\u0307\u0323 \r\n');
 expect(ipc).toHaveBeenCalledWith('page_reading_text',{sourceId:'source',pageIndex:2});
 ipc.mockRejectedValueOnce(Error('Copy permission denied'));
 await expect(nativeAdapter.getPageReadingText!('source',2)).rejects.toThrow('Copy permission denied');
});
