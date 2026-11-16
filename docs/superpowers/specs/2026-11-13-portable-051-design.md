# Folio0.51 development candidate integration

Deliver the current paragraph editing developments in a complete testable local
GUI/CLI portable candidate. This is progress toward the broad quality goal,
not public release or Adobe/Foxit parity. User opted out of routine approvals;
executor chooses the integration below. The previous Find goal turn made
verified progress: source31eb0aebd1021d20721b0e4abe6bdedd7be7c3aa1c343d7396b6ea0e69e68954,
1630frontend/234files and reviewed corrected compiled acceptance.

## Approaches and decision

Keep isolated diagnostic executables, publish/select as fully accepted, or
build a complete development candidate with the known clipboard gate explicit.
Choose the third: it gives the user an executable with both improvements while
preserving selected0.50 and the unresolved full paste acceptance requirement.
No automatic launcher selection until that requirement is reconciled separately.
Do not remove paste or weaken existing tests to manufacture a passing candidate.

## Content and identity

Current paragraph Find/Replace (literal/simple-fold/Whole words, local Undo,
keyboard/Escape fixes) is a completed reviewed component. Multiline Paste as
paragraph has corrected implementation and automated event-path acceptance,
but real system CtrlV/IME acceptance remains OPEN. New readonlySTAclipboard
probe now opens (sequence unchanged), but the managed preservation probe finds
one unsupported format and three memory streams, so clipboard was NOT modified.
No content/type names were recorded. Do not overwrite user clipboard for QA.

The selected0.50 standalone7018pin checkpoint passes. Current product differs
from it in exactly10declared frontend paths (three added, seven modified); no
native/renderer/dependency/shared-test change. Bind all paths to exact Find
completion/current source and Paste qualification snapshots, not an allowlist
of arbitrary changed bytes. Apply only six050to051version tokens across five
manifests; preserve every other byte/member/line ending. Native1596passed/11ignored
library correctness (Folioopt0/releasedependencies) plus120ordinary optimized
integration results retain qualified reuse through selected050rawproof; never
call them fresh051or all-project native tests.

## Build and acceptance

Fresh full frontend: two workers, default5000ms. Normal release GUI/default CLI,
26unchanged renderer resources, complete OCR/licenses/docs/examples/archive.
Use strict existing ZIP validation and fresh outside-checkout extraction; bind
source/executable/archive/resources/dependencies/doc membership. No installer,
network dependency update, unsigned-public-release or user-app automatic launch.

Retain the selected05028stage runtime and8additional acceptance assertions,
including105edits/100Undo/100Redo,300independent saved pages,512MiBnative/1GiBtree
limits at natural scaling, cancellation/credential/form/CFF/automation behavior.
Adapt helper identities to the actual extracted051binaries without relaxing
thresholds/timeouts or replacing samples. Preserve existing private-data and
original-source boundaries; new fixtures public, no private data or clipboard
contents in package/review. Add exact extracted-candidate paragraph Find and
multiline event-path scenarios from accepted components and independent PDF
controls. Call injected ClipboardEvent path what it is; no realCtrlV claim.

README/changelog/status must distinguish candidate validation from selected050,
current-paragraph scope from whole-document replacement, and incomplete paste
transport from completed Find acceptance. No unrelated feature changes.

One final combined review after candidate acceptance; one Important fix pass
with RED/GREEN, no re-review. Preserve failures/oldpackages/dirty worktree. Build
standalone raw evidence and corruption/omission/path/identity/relocation/archive
checks for the candidate. Candidate completion must keep transport gate open,
selected050unchanged and broadgoalactive. No commit/merge/push/deletion/public
release. Cargoonejob/D:cache+temp; serialize frontend/build/native/GUI.
