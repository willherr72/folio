# Mixed Word multi-tab recovery acceptance

Continue the user-authorized quality loop with the two immutable Word exports from
the accepted mixed-word-layout milestone, on the same verified 0.26 package.
The prior pass proves individual edit/save/replay. This pass combines independent
dirty tabs, keyboard switching, history isolation and durable recovery.

Use separate owned copies named A.pdf (untagged) and B.pdf (tagged). Insert ` this`
inside A's highlighted instruction and ` this this` in B's, with respective widths
340 and 300 pt and height 110 pt. Record exact native requests, text and rendered
PNG hashes. Undo in one tab must not change the other. Save reference edits, then
rotate only page one by 90 degrees in A and 180 degrees in B. Both are now dirty;
wait for a durable checkpoint containing both distinct source owners, rotations,
dirty flags and the active tab, and preserve its encrypted bytes and decoded
test-only manifest. No private documents are involved.

Stop only the owned test process without using Folio's graceful-close action.
Move only the two exact-hash test copies to preserved names inside this artifact
directory. Restart the same binary against the SAME recovery data directory,
using a fresh WebView profile and separate process/evidence record. Restore through
the normal dialog. Both documents must retain their own text, formatting, rotations
and dirty status. Save each restored document; verify that saving one does not clear
the other's dirty state. Reopen and preview/cancel unchanged paragraphs where the
existing rotation support permits, recording any unsupported behavior honestly.

Independent audit compares restored PDFs with the saved unrotated reference after
normalizing only the intentional page rotation in memory: all page text, glyph
styles/positions, pixels at 1x/2x/4x, font programs, links, bookmarks and tagged
graphs. Validate source envelopes/checksums and prove recovery did not need the
missing original paths. Add swapped-tab, wrong-rotation and changed-pixel negative
controls. This is a durable-checkpoint/process-termination test, not power loss,
full undo-history persistence, performance acceptance, or commercial parity.

Alternatives considered: a copied-checkpoint restoration repeats an existing
synthetic result and does not exercise restart in the original data location;
editing only one real-producer tab cannot expose cross-document ownership bugs.
The combined two-tab same-data restart is the selected implementer approach.
No dependencies, product changes, builds or releases unless a concrete defect is
reproduced. Such a defect gets a focused failing regression before correction.
