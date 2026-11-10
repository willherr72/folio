import {describe,it,expect} from 'vitest';
import {emptyNavigationHistory,recordNavigation,travelNavigation,sameReadingLocation,canTravelNavigation,type ReadingLocation} from '../src/editor/navigation-history';
const at=(pageId:string,y=100,zoom=100):ReadingLocation=>({pageId,zoom,point:{x:75,y}});
describe('reading history',()=>{
 it('returns to the exact prior view, captures current scrolling, and branches',()=>{
  let history=recordNavigation(emptyNavigationHistory(),at('a',250,130),at('b'));
  const back=travelNavigation(history,at('b',320,90),'back',['a','b','c'])!;
  expect(back.target).toEqual(at('a',250,130));expect(back.history.forward).toEqual([at('b',320,90)]);
  const forward=travelNavigation(back.history,at('a',280,140),'forward',['a','b','c'])!;
  expect(forward.target).toEqual(at('b',320,90));expect(forward.history.back).toEqual([at('a',280,140)]);
  history=recordNavigation(back.history,at('a',250,130),at('c'));
  expect(history.forward).toEqual([]);expect(history.back).toEqual([at('a',250,130)]);
 });
 it('records same-page jumps but ignores unchanged views and invalid positions',()=>{
  const initial=emptyNavigationHistory();expect(recordNavigation(initial,at('a'),at('a'))).toBe(initial);
  expect(recordNavigation(initial,at('a'),at('a',400)).back).toEqual([at('a')]);
  expect(recordNavigation(initial,at('a',NaN),at('b'))).toBe(initial);
  expect(sameReadingLocation(at('a'),at('a',100.1))).toBe(true);
 });
 it('skips removed pages and same views without confusing reordered or duplicated pages',()=>{
  const history={back:[at('original'),at('deleted'),at('current')],forward:[]};
  expect(canTravelNavigation(history,'back',['current','duplicate','original'])).toBe(true);
  const result=travelNavigation(history,at('current'),'back',['current','duplicate','original'])!;
  expect(result.target.pageId).toBe('original');expect(result.history.back).toEqual([]);
  expect(canTravelNavigation({back:[at('deleted')],forward:[]},'back',['current'])).toBe(false);
  expect(travelNavigation(history,at('current'),'back',['duplicate'])).toBeNull();
 });
 it('bounds history and deduplicates adjacent recorded origins',()=>{
  let history=emptyNavigationHistory();
  for(let i=0;i<150;i++)history=recordNavigation(history,at(String(i)),at(String(i+1)));
  expect(history.back).toHaveLength(100);expect(history.back[0].pageId).toBe('50');
  expect(recordNavigation(history,at('149'),at('150')).back).toHaveLength(100);
 });
 it('consumes history when the reader has manually scrolled back to its only valid destination',()=>{
  const history={back:[at('deleted'),at('a')],forward:[]};
  const result=travelNavigation(history,at('a'),'back',['a','b']);
  expect(result?.target).toEqual(at('a'));expect(result?.history.back).toEqual([]);
  expect(result?.history.forward).toEqual([at('a')]);
 });
});
