# Image pages

Create a PDF from ordered PNG/JPEG files, or insert them before/after the current
page or at the end of an existing PDF. Each image becomes one page. Users can add,
remove and reorder files before conversion. Letter, A4 and Legal paper, portrait,
landscape or automatic orientation, and zero/half-inch margins are available.
Images fit within the available area without cropping or changing aspect ratio.
Automatic orientation follows the image after EXIF orientation is applied.
Transparency is composited onto white. Conversion uses decoded RGB samples;
print-oriented ICC color management and individual image-object editing are later work.

The native engine validates all settings and bounded input, decodes sequentially,
applies EXIF orientation, creates a PDF in memory, and publishes an immutable source
only after every page succeeds. Limits: 32 images, 64 MiB compressed per image,
32 million pixels per image, 80 million pixels total. No input file is changed.
Generated source bytes participate in existing export, recovery and close ownership.

Creation opens an unsaved tab. Insertion is one undo step, selects the first new
page, closes active Find, and retains its source for redo. Canceling file selection
preserves the list; conversion failure preserves settings. Pending operations
prevent duplicate submission/dismissal. Late sources after unmount are closed.

Acceptance includes PNG transparency, JPEG EXIF rotation, file order, fit/margins,
invalid input and bounds, export/reopen, recovery without original images, UI
reordering/removal, insertion undo/redo, error/retry, cancellation and desktop smoke.
