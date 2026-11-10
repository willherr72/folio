# Bounded interactive form inspection

The new field-authoring path validates one widget within a budget, but existing
Fill forms enumeration and single-field filling still call unlimited inspection.
Shared indirect option strings can expand into many independent owned strings.
Existing limited_string charges only after allocation and omits vector capacities,
fallback labels and copied selections.

Use a default 8 MiB conservative metadata allocation budget for page enumeration
and individual field inspection. Honor smaller caller budgets (authoring/export).
Charge output vector capacity before reservation, option/selection storage, decoded
strings, fallback-label copies, kind/reason strings and duplicated selections.
Read UTF-16 into a bounded scratch buffer (existing 32768-unit limit). Check a
minimum retained cost before requesting payload, validate UTF-16 and measure exact
UTF-8 length before allocating the decoded String. Normalize text line endings in
owned storage. Invalid lengths, malformed/changing native strings and budget
overflow return errors; no partial/truncated field model escapes.

This bounds the Rust inspection model and its allocation work; PDF parsing, PDFium
internal allocations, rendering, source documents and save buffers are distinct
existing limits. Keep that distinction explicit. Preserve existing field support,
values, actions, fonts, geometry, source permissions and original bytes. An over-
budget page must not prevent opening/rendering its PDF or inspecting another page.
Lazy options/field pagination remain future work for unusually large valid forms.

Verify bounded native-getter behavior with unit callbacks, shared-string option
amplification through the public API, the existing maximum individual string size,
Unicode/line endings, filtered export, normal authored/existing filling and save/
reopen. Run one scoped review, final native/frontend/build checks, actual Windows
refusal/recovery and normal filling, independent output fidelity and owned-process
cleanup. No release/parity claim and no commits.
