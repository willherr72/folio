# Folio editor upgrade implementation plan
> For agentic workers: use superpowers:subagent-driven-development and requesting-code-review.

Goal: deliver the user's requested drawing, modal, zoom and reading settings in a runnable desktop update.
Architecture: retain PDFium engine and overlay IPC. Extend page interaction, isolate viewport and modal/preferences components, integrate in App.
Tech stack: React, TypeScript, Tauri, Rust/PDFium.
Spec: docs/superpowers/specs/2026-09-10-editor-upgrades-design.md

## Global constraints
All PDF work remains local. Preserve existing source safety and vector export. Changes are authorized; implement reversible choices without approval stalls. Each owner edits only assigned files.

## Task 1 — Drawing and viewport
Files: src/components/PageView.tsx, new DocumentViewport.tsx, optional editor/drawing.ts, viewport/drawing tests.
- [ ] Write failing behavior tests for rotated drawing, cancel/end and page routing/scroll.
- [ ] Extend PageView with draw tool, draft path, pointer capture, safe cancellation and optional page activation/interaction disabling.
- [ ] Implement continuous/single viewport with lazy full-page rendering, scroll-selected page, explicit navigation requests and anchored Ctrl+wheel zoom.
- [ ] Validate targeted tests/build and commit; report interface and evidence.

Interface: DocumentViewport props adapter,pages,selectedPageId,selectedOverlayId,zoom,onZoomChange,viewMode,tool,penColor,penWidth,pendingSignature,interactionDisabled,navigationRequest:{pageId,revision}|null. Page-scoped callbacks onSelectPage(id),onSelectOverlay(pageId,id|null),onAddText(pageId,point),onPlaceSignature(pageId,point),onMoveOverlay(pageId,id,x,y,phase),onDraw(pageId,path). PageView existing callbacks retained; add optional onDraw(path), drawColor,drawWidth,onActivate,interactionDisabled. tool adds draw.

## Task 2 — Modal, settings and theme
Files: new components/Modal.tsx, ConfirmDialog.tsx, SettingsDialog.tsx, editor/preferences.ts, styles.css; SignaturePad modal wrapper; own tests.
- [ ] Write failing preference/focus/modal behavior tests.
- [ ] Provide validated persistent settings hook and reusable accessible centered modal.
- [ ] Build settings UI and consistent dark/light styles; keep PDF paper colors intact.
- [ ] Adapt signature pad to shared modal; validate tests and commit.

Interface: usePreferences() returns {preferences,setPreferences,resetPreferences}; Preferences {theme:'system'|'light'|'dark',viewMode:'continuous'|'single',defaultZoom:number,penColor:string,penWidth:number}. SettingsDialog props preferences,onChange(nextPreferences),onReset,onClose. ConfirmDialog props title,description,confirmLabel,onConfirm,onCancel; cancel default focus. Shared Modal props title,children,onClose,className?,description?,initialFocusRef?; wraps a centered dialog with focus trap.

## Task 3 — App integration and delivery (parent)
Files: App.tsx, app tests, scripts, docs, versions, root launcher after merge.
- [ ] Add failing app tests for modal cancellation, settings, drawing and page identity.
- [ ] Remove engine badge, integrate asynchronous modal actions/close, block background shortcuts, wire page-scoped state/history callbacks, settings and Draw toolbar/properties.
- [ ] Run full frontend/native checks and browser/native smoke, review, resolve substantive findings.
- [ ] Merge local branch, package versioned 0.2 update, verify and update launcher; commit handoff.

No native payload change needed. Page IDs identify edits even if scroll selection changes. Preferences do not dirty PDFs. Save cancellation preserves dirty state. Latest file/dialog cancellation never closes current source prematurely.
