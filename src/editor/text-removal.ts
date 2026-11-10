import {invoke} from '@tauri-apps/api/core';
import type {AnnotationRect,DocumentInfo,PagePlan,DocumentBookmark} from './types';

/** Coordinates refer to this exact immutable source, not a workspace PagePlan. */
export interface TextRemovalMark {pageIndex:number;rect:AnnotationRect}
export interface TextRemovalPage {pageIndex:number;width:number;height:number;selectedGlyphCount:number;extent:AnnotationRect}
export interface TextRemovalPreview {
 previewId:string;sourceId:string;candidate:DocumentInfo;removedCharacters:number;
 affectedBlocks:number;omittedMetadata:string[];expiresInSeconds:number;pages:TextRemovalPage[];
}
export interface TextRemovalBounds {pageIndex:number;offset:number;total:number;boxes:AnnotationRect[]}
export interface TextRemovalSource {document:DocumentInfo;omittedMetadata:string[]}
export interface ContentRemovalPage {
 pageIndex:number;width:number;height:number;removedCharacters:number;removedPaths:number;
 affectedImages:number;removedImagePixels:number;extendsBeyondMarks:boolean;extent:AnnotationRect|null;
}
export interface ContentRemovalPreview {
 previewId:string;sourceId:string;candidate:DocumentInfo;removedCharacters:number;removedPaths:number;
 affectedImages:number;omittedMetadata:string[];expiresInSeconds:number;pages:ContentRemovalPage[];
}
export type RemovalPreview=TextRemovalPreview|ContentRemovalPreview;
export const isContentRemoval=(preview:RemovalPreview):preview is ContentRemovalPreview=>'affectedImages' in preview;
export interface ContentRemovalRaster {
 sourceId:string;previewId:string|null;candidateId:string|null;pageIndex:number;
 pageWidth:number;pageHeight:number;width:number;height:number;
 sourcePng:number[];candidatePng:number[]|null;disclosurePng:number[]|null;
}
export interface ContentRemovalAdapter {
 previewSourceContentRemoval(sessionId:string,sourceId:string,marks:TextRemovalMark[]):Promise<ContentRemovalPreview>;
 renderContentRemoval(sessionId:string,sourceId:string,previewId:string|null,pageIndex:number,width:number):Promise<ContentRemovalRaster>;
}
/** Internal transport foundation; UI must compose and bind current history first. */
export interface TextRemovalAdapter {
 prepareTextRemovalSource(sessionId:string,pages:PagePlan[],bookmarks?:DocumentBookmark[]):Promise<TextRemovalSource>;
 beginTextRemovalSession():Promise<string>;
 previewSourceTextRemoval(sessionId:string,sourceId:string,marks:TextRemovalMark[]):Promise<TextRemovalPreview>;
 textRemovalPreviewBounds(sessionId:string,previewId:string,pageIndex:number,offset:number,limit:number):Promise<TextRemovalBounds>;
 applyTextRemovalPreview(sessionId:string,previewId:string):Promise<DocumentInfo>;
 discardTextRemovalPreview(sessionId:string,previewId:string):Promise<void>;
 releaseTextRemovalSession(sessionId:string):Promise<void>;
}
export const nativeTextRemovalAdapter:TextRemovalAdapter&ContentRemovalAdapter={
 prepareTextRemovalSource:(sessionId,pages,bookmarks)=>invoke('prepare_text_removal_source',{sessionId,request:{pages,flatten:false},...(bookmarks===undefined?{}:{bookmarks})}),
 beginTextRemovalSession:()=>invoke('begin_text_removal_session'),
 previewSourceTextRemoval:(sessionId,sourceId,marks)=>invoke('preview_source_text_removal',{sessionId,sourceId,marks}),
 textRemovalPreviewBounds:(sessionId,previewId,pageIndex,offset,limit)=>invoke('text_removal_preview_bounds',{sessionId,previewId,pageIndex,offset,limit}),
 applyTextRemovalPreview:(sessionId,previewId)=>invoke('apply_text_removal_preview',{sessionId,previewId}),
 discardTextRemovalPreview:(sessionId,previewId)=>invoke('discard_text_removal_preview',{sessionId,previewId}),
 releaseTextRemovalSession:(sessionId)=>invoke('release_text_removal_session',{sessionId}),
 previewSourceContentRemoval:(sessionId,sourceId,marks)=>invoke('preview_source_content_removal',{sessionId,sourceId,marks}),
 renderContentRemoval:(sessionId,sourceId,previewId,pageIndex,width)=>invoke('render_content_removal',{sessionId,sourceId,previewId,pageIndex,width}),
};
