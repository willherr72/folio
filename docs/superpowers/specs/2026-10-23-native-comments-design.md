# Native sticky-note review and editing

Standard Text annotations with NoZoom/NoRotate flags remain native because converting them to Folio overlays would change their appearance. Show remaining native Text annotations alongside overlay comments in Review. Selecting one navigates to its page and opens a plain-text comment dialog with author and subject when available.

Keep immutable source ownership: editing produces a new full source, updates all workspace pages sharing that source, and is one undo step. Preserve existing overlays, page order, bookmarks, protection, recovery, and original files. Edit Contents only; retain flags, Rect, AP, popup, author, dates, and other metadata. No icon regeneration.

Initial editable native scope is visible plain-text notes with NoZoom or NoRotate. Locked, read-only, rich-text, hidden, reply/state annotations, and other unsupported native notes remain readable with a reason. This stable flag-based eligibility also prevents a replacement from unexpectedly becoming an imported overlay during recovery. Existing imported F4 comments retain their established editing behavior. Detect and exclude exactly the original annotation indices already imported into overlays.

Native listing is bounded and fails visibly on malformed metadata. Enforce annotation permissions in the native edit command; allow listing when annotation or copy permission is available. Reject signed/certified documents for rewriting. Check expected old text, validate replacement size (1,000,000 UTF-8 bytes) and no NUL, serialize, reload, and verify. Source identity plus original page/annotation indices identifies a note; no mutable index is reused across source versions.

Use existing Rust/lopdf/PDFium and React/Tauri dependencies. No version bump, package overwrite, commit, or publish for this source checkpoint. User authorized autonomous implementation; continue inline without approval gates.
