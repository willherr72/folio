# Reading navigation history

Previous goal turn made progress: toolbar visibility fixed,588 frontend tests,
production build, browser checks and focused review pass. The overarching parity
goal remains active. User authorizes implementation without routine questions.

## Decision

Use per-tab Back/Forward stacks of reading locations (stable workspace page ID,
PDF-space point at viewport center, PDF zoom). Whole-window pixel offsets break
when pages reorder; page-only history loses position inside long pages. Do not
record scrolling or zoom changes individually: capture the actual view just before
an explicit jump, and compare the resulting view before adding an entry. Bound each
stack to100 entries. History is ephemeral, separate from edit undo/dirty state and
recovery format; recovered documents start with empty navigation history.

## Behavior

- Bookmark, thumbnail, page-jump, previous/next page, search result and review-list
  navigation record the preceding view. Same-page jumps count if position changes.
  No-op/repeated views do not add duplicate steps.
- Back/Forward restores page and zoom and anchors the saved PDF point at viewport
  center, constrained by current scroll bounds. Works in continuous and single-page
  mode; keep current view mode. Rotation/reorder preserves original page identity.
- Store the current actual view on the opposite stack during traversal, so scrolling
  and zooming after a jump are retained. A new jump clears forward history.
- Deleted destinations are skipped; duplicate pages have distinct identities. Edit
  undo remains independent. Missing or invalid destinations never select phantom pages.
- History belongs to a tab and vanishes on close; other tabs and source bytes are
  unaffected. Busy/modal states block controls and shortcuts. Back/Forward close
  Find so delayed results cannot override restored navigation; preserve its query.
- Accessible Back/Forward buttons near page navigation, with Alt+Left/Right shortcuts
  outside typing fields. Prevent browser navigation when handling these keys. Do not
  intercept modified text editing shortcuts or disabled/modal actions.
- Preserve toolbar visibility at900x600; no new native PDF writes or package overwrite.

## Verification

Pure history tests: branching, cap, no-op, missing pages and stable IDs. Real viewport
unit tests: point capture/restore, zoom, rotations, single-page layout and clamping.
App tests: bookmark/search/thumbnail/review jumps, tab isolation, dirty/edit-undo,
modal/keyboard guards and deletion. Real browser workflow on multipage fixture,
including zoom, long-page scrolling, reorder/delete, tabs, search and900x600.
Full frontend suite and production build; one fresh focused final review. Native
code remains unchanged. Actual Windows build/smoke if practical; explicitly record
any scope not exercised. Package0.17 stays intact until a later combined release.

## Implementation findings

Toolbar zoom previously retained document-global pixels and could move the reader
to a preceding page. It now uses the same captured PDF center without adding a
history step. Mouse-wheel zoom keeps its existing cursor anchor. Find tracks the
last followed result per tab so modal dismissal and tab remount do not repeat an
old jump; new query/result or reopening Find may navigate normally.
