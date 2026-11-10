# Integrated paragraph editing audit

The accepted 0.22 combined package covers isolated editing and saved outputs. The
next coordinator-selected milestone checks an everyday session: repeat edits,
cancel previews, undo/redo, switch tabs, then close documents. The user's priority
remains reliable existing-PDF editing. Their standing instruction authorizes
autonomous implementation and verification without routine permission questions.

Use the accepted packaged desktop executable, a new owned process/profile, copies
of the user's quiz and packaged welcome PDF, and the repository's documented
WebView2/CDP/native-dialog workflow. Preserve all originals, packages and evidence.
Do not rebuild or change product source until the audit demonstrates a defect.

Approaches considered: another feature-specific suite would repeat the previous
checkpoint; a broad unstructured tour would not give repeatable evidence. Choose a
bounded integrated workflow with explicit assertions and natural-idle measurements.
This is the coordinator's choice, not a new user requirement.

Run at 1280x900 and 900x600 CSS viewports. At each size, edit the original quiz
paragraph through its supported editor, type the same insertion, preview/cancel
five times, then apply/undo/redo/undo five times. Each cancellation must retain the
original text and clean document state. Each undo restores original text; redo
restores the insertion. Switch to the welcome tab and back between operations and
verify the other document remains unchanged. At narrow size, verify actual keyboard
focus can reach the enabled footer actions and that focused controls are visible;
record geometry and screenshots rather than assume successful synthetic clicks
prove usability. No document-wide horizontal overflow is allowed.

Install metadata-only URL create/revoke instrumentation before opening documents.
Record created/revoked counts, live URL sizes, JS heap/DOM counters, native and
descendant process private/working-set snapshots before opening, after each block,
after all clean tabs close, and after ten seconds idle. All tracked URLs must be
released after closing. Do not force GC, retain image payloads, or infer a leak from
working-set growth alone. Timings include automation/IPC and are diagnostic, not
an absolute performance SLA or physical 8 GB certification.

Functional failures or retained URLs require a preserved reproduction and root
cause before any fix; corrections get targeted failing regression coverage and
the affected suites/runtime rerun. A clean audit still records observed resource
behavior and limits. Keep one final independent scoped review of the audit and any
correction, then record the checkpoint. The overall parity goal remains active.
