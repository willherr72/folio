import { describe, expect, it } from "vitest";
import { documentToPages } from "../src/editor/adapter";
import { cloneOverlay, createHistory, commit, duplicatePage, removeOverlay, undo, redo } from "../src/editor/model";
import { resizeInk } from "../src/editor/geometry";
import type { EditorDocument, InkOverlay, Overlay } from "../src/editor/types";
const highlight: Overlay = {type:"highlight",id:"h",rects:[{x:10,y:20,width:50,height:12}],color:"#FFE066"};
const comment: Overlay = {type:"comment",id:"c",x:20,y:40,text:"Review this",color:"#FFE066"};
describe("review annotation state",()=>{
 it("imports editable annotations into an independent page plan",()=>{
  const pages=documentToPages({id:"source",name:"notes.pdf",pages:[{width:200,height:300,overlays:[highlight,comment]}]});
  expect(pages[0].overlays).toEqual([highlight,comment]);
  expect(pages[0].overlays[0]).not.toBe(highlight);
 });
 it("duplicates annotation geometry without sharing it and restores delete via undo/redo",()=>{
  const doc:EditorDocument={name:"notes",pages:documentToPages({id:"s",name:"n",pages:[{width:200,height:300,overlays:[highlight,comment]}]}),selectedPageId:null,selectedOverlayId:"h"};
  const original=doc.pages[0];
  const duplicate=duplicatePage(doc,original.id,"copy");
  const copied=duplicate.pages[1].overlays[0];
  expect(copied).toEqual(highlight);
  if(copied.type==="highlight" && original.overlays[0].type==="highlight") expect(copied.rects[0]).not.toBe(original.overlays[0].rects[0]);
  const deleted=commit(createHistory(doc),v=>removeOverlay(v,original.id,"h"));
  expect(deleted.present.pages[0].overlays).toEqual([comment]);
  expect(undo(deleted).present).toEqual(doc);
  expect(redo(undo(deleted)).present).toEqual(deleted.present);
  expect(cloneOverlay(comment)).toEqual(comment);
 });
});
describe("proportional placed ink resizing",()=>{
 const ink:InkOverlay={type:"ink",id:"signature",paths:[[{x:20,y:40},{x:120,y:90}]],color:"#000000",strokeWidth:2};
 it("keeps the anchor and aspect ratio while fitting the page",()=>{
  expect(resizeInk(ink,200,500,500).paths).toEqual([[{x:20,y:40},{x:220,y:140}]]);
  expect(resizeInk(ink,200,150,100).paths).toEqual([[{x:20,y:40},{x:140,y:100}]]);
  expect(ink.paths[0][1]).toEqual({x:120,y:90});
 });
 it("leaves invalid or degenerate requests unchanged",()=>{
  expect(resizeInk(ink,NaN,500,500)).toBe(ink);
  expect(resizeInk({...ink,paths:[]},100,500,500).paths).toEqual([]);
 });
});