import {invoke} from '@tauri-apps/api/core';
import type {AnnotationRect,PagePlan,PageSearchOptions} from './types';
import type {WorkspaceReplacementResult} from './workspace-replacement';

export interface WorkspaceReplacementIntent {query:string;replacement:string;options:PageSearchOptions;selected?:Array<{pageId:string;matchIndex:number}>|null}
export interface WorkspaceReplacementMatch {matchIndex:number;overlayId:string|null;rects:AnnotationRect[];selected:boolean;blocker:string|null}
export interface WorkspaceReplacementPlan {
 planId:string;query:string;replacement:string;options:PageSearchOptions;
 pages:Array<{pageId:string;matches:WorkspaceReplacementMatch[]}>;
 selectedCount:number;blockedCount:number;affectedPageIds:string[];batchBlocker:string|null;canApply:boolean;
}
export interface NativeWorkspaceReplacementPreview {previewId:string;result:WorkspaceReplacementResult;affectedPageIds:string[];expiresInSeconds:number}
export interface WorkspaceReplacementAdapter {
 beginWorkspaceReplacement(pages:PagePlan[]):Promise<string>;
 planWorkspaceReplacement(sessionId:string,intent:WorkspaceReplacementIntent):Promise<WorkspaceReplacementPlan>;
 previewWorkspaceReplacement(sessionId:string,planId:string):Promise<NativeWorkspaceReplacementPreview>;
 applyWorkspaceReplacement(sessionId:string,previewId:string):Promise<WorkspaceReplacementResult>;
 cancelWorkspaceReplacement(sessionId:string):Promise<void>;
 releaseWorkspaceReplacement(sessionId:string):Promise<void>;
}
export const nativeWorkspaceReplacementAdapter:WorkspaceReplacementAdapter={
 beginWorkspaceReplacement:pages=>invoke('begin_workspace_replacement',{pages}),
 planWorkspaceReplacement:(sessionId,intent)=>invoke('plan_workspace_replacement',{sessionId,intent}),
 previewWorkspaceReplacement:(sessionId,planId)=>invoke('preview_workspace_replacement',{sessionId,planId}),
 applyWorkspaceReplacement:(sessionId,previewId)=>invoke('apply_workspace_replacement',{sessionId,previewId}),
 cancelWorkspaceReplacement:sessionId=>invoke('cancel_workspace_replacement',{sessionId}),
 releaseWorkspaceReplacement:sessionId=>invoke('release_workspace_replacement',{sessionId}),
};
