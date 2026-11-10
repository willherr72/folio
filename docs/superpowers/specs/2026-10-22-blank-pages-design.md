# Blank PDF creation and insertion

Provide New PDF from the empty workspace and top toolbar; Insert blank page from
Page actions. Native blank-page source creation returns one immutable page. Paper
choices are Letter, A4 and Legal with portrait/landscape orientation. Insertion
supports before/after the current page or at document end (default after).
New documents start named Untitled.pdf and unsaved. Inserting selects the new page
and is one undo step. Undo retains the source for redo; closing its tab releases it.

Native API create_blank_document(width,height) creates and saves a one-page PDF in
memory, reopens those exact bytes, and publishes a normal DocumentInfo. Validate
finite dimensions in 144..1440 pt (2..20 inches) before PDFium allocation. No disk
file is created. A unique absolute virtual temp path provides the existing recovery
source identity; recovery snapshots the immutable bytes as with edited sources.
No source overwrite protection is removed. Existing export, annotation and text-area
paths work on the new page. Late creation after unmount must release its source.

Modal blocks edits/navigation and retains settings on errors. Preset-only first
UI avoids ambiguous unit entry; arbitrary/custom paper sizes can follow. Desktop
creation only; browser demo remains explicit. Verify create/render/export/reopen,
recovery with no original disk file, insert positions, history, dirty state and late
source cleanup. No commits, external publishing or app restart required.
