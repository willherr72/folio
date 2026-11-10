# Unsaved paragraph edits across document tabs

The integrated editing audit verified clean-state tab switches. The next
coordinator-selected check covers unsaved work: an edit must remain attached to
its document, and Undo, Redo, close confirmation and Save must affect the intended
tab. This follows the user's standing autonomous quality goal; it is not a new
feature request or a promise of background editing.

Use the accepted 0.22.0-dev.1 desktop and the documented owned WebView2/native-dialog
workflow. The full native run exposed a fixture-only failure, now corrected with
eight focused passes and unchanged product hashes. The desktop audit may run
concurrently against that accepted binary; both full native and desktop acceptance
are required before the final checkpoint. Make no product changes while the native
run is active. Concurrent execution is not a performance benchmark. Preserve all original
PDFs, packages, prior audit evidence and user processes. No commits or publication.

Approaches considered: mocked App tests cover some state transitions but cannot
prove native candidate ownership and saved output. A broad manual tour gives weak
repeatability. Choose a bounded native desktop audit using two distinct copies of
the same one-page quiz, named Quiz A.pdf and Quiz B.pdf. Their identical original
content makes explicit tab/page identity essential. Insert `this` after `vaping`
in A and `term` after `vaping` in B, giving distinguishable native candidates.

At 1280x900 and 900x600 CSS viewports, with newly opened clean copies for each:

1. Record each tab ID, page ID, full first text layer and valid image digest.
   The two page IDs must differ. Scope every assertion to the active named tab.
2. Preview the A edit. Confirm both tabs and close buttons are disabled. Send
   Ctrl+Tab and Ctrl+Shift+Tab; A and its exact draft must remain selected/intact.
   Cancel must restore A's original text/image and leave both tabs clean.
3. Apply the A edit, record its complete resulting text and valid changed image
   digest. Switch to B and verify B's original text/image and clean indicator.
   Apply the distinct B edit and record its complete resulting text/image.
4. Switch repeatedly between A and B, checking their respective exact recorded
   applied text/image and dirty indicators. Undo A, verify A's baseline, then
   verify B remains edited. Undo B and verify its baseline. Redo A and B separately;
   each must recover its own recorded candidate and dirty indicator.
5. While B is active, request closing dirty A. The confirmation must name A.
   Keep editing must retain both tabs, B's active identity, both edits and history.
6. Save A to a fresh file. A becomes clean; B remains dirty and unchanged. Reopen
   the saved A output in a third tab and verify A's exact edited text. Close this
   clean third tab, recheck B, undo B to baseline, then close both clean source tabs.

Reuse the corrected valid-digest observation helper. Capture the digest that
satisfies each poll; never compare null hashes or perform a second unchecked read.
Record all identity/state assertions and intermediate observations incrementally.
On failure, retain the error, completed steps and screenshot; do not weaken the
assertion or overwrite evidence. Inspect both narrow and wide screenshots.

Verify the two saved A outputs independently with the existing quiz reader oracle:
expected paragraph text and original style, plus unchanged pixels outside the
padded original/edited paragraph area. Recompute input/package hashes and stop only
the owned app after checking executable path and start time. A demonstrated product
defect requires root-cause analysis, a failing regression, a minimal correction and
relevant reruns. Otherwise no product change is justified.

One final independent scoped review will assess the harness, evidence and any
correction. This audit does not prove recovery after a crash, arbitrary overlapping
async jobs, every PDF producer, full keyboard-only entry, all-page fidelity,
physical hardware performance, printing or commercial parity. It adds no timing
or memory benchmark; the previous audit retains those separately qualified data.
