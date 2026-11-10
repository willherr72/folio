# Controlled paragraph-preview memory diagnosis

Continue the active quality goal by distinguishing image-content effects from other
repeated-preview costs on the verified optimized Folio 0.27 package. This is a
diagnostic milestone, not a product fix or commercial-parity acceptance.

## Approaches and decision

Repeating the historical observer/idle experiments would not add a discriminating
variable. Changing image disposal before reproducing a fault would risk masking
the cause. Instead, compare two fresh owned application sessions with identical
workflow and sampling: repeated identical text versus distinct permutations of
the same six letters. This is the coordinator's choice under the user's standing
authorization to proceed autonomously.

## Protocol

- Use the exact verified extracted 0.27 package, original public Word fixture,
  1280 by 900 viewport, unchanged zoom, and 63 preview/cancel cycles per arm.
- Replace `plan` with `plan tthiss` in the constant arm. In the varied arm, append
  63 distinct permutations of `tthiss`. Equal length and character multiplicity
  control natural advance width; record actual PNG dimensions and fitting bounds
  so this assumption is observable. Artificial words are diagnostic inputs only.
- Three warmups and 20 measured cycles retain all original budgets: preview P95
  2000 ms; cancel P95 1000 ms; native growth 64 MiB; process-tree growth 128 MiB;
  maximum sampled native 512 MiB; maximum sampled tree 1 GiB. Forty extra cycles
  diagnose later growth without changing the original calculation. Retain the
  historical 10-second pause after measured cycle 20 in both arms.
- Snapshot every five measured cycles, after close, and after ordinary 10- and
  30-second cumulative idle. Never force GC or change the thresholds to pass.
- Prove each candidate closes, every Cancel restores source text and raster,
  originals remain unchanged, and no unsaved state remains. Record actual preview
  URL hashes, PNG hashes, dimensions and byte counts, not inferred cardinality.
- Use creation-time-bound process ancestry and revalidation. Add role labels from
  the same CIM snapshot, never a later unbound PID lookup. Preserve unknown roles.
- Capture helpers and inputs before launch, verify them afterward, and stop only
  the owned exact process identity. Preserve all failures and older packages.

## Interpretation and limits

Different PNG cardinalities are a prerequisite for the comparison. Different
memory trajectories are a lead, not causal proof: two sequential runs are subject
to GPU allocator and host scheduling variation. If the direction warrants it,
reverse order before proposing a fix. A passing finite run is not a leak proof.
Report latency separately from memory and functional correctness. If no defect
is reproduced, retain the historical failure and record the narrower evidence;
do not invent a product change. No build or full product regression is needed
for diagnostic-only files. Any demonstrated product correction requires its own
failing regression and appropriate product validation before packaging.

## Follow-up discriminator: modal compositing

The constant arm reproduced excessive growth with identical image content, and
the varied arm shows the same direction. Source inspection identifies a full-window
`backdrop-filter: blur(3px)` mounted whenever the existing-text dialog opens.
After the first pair completes, run the identical constant workflow with only
that backdrop blur disabled through a captured diagnostic stylesheet. Record and
assert its computed value while the dialog is open. Do not change shipped files.
If this sharply changes growth, repeat the stock constant arm to test ordering;
otherwise retain the negative result and investigate auxiliary image decoding.

## Follow-up discriminator: auxiliary bitmap decoding

The no-modal-blur run still exceeded the first-20 growth budget. The original
harness creates an extra `ImageBitmap` each cycle in addition to the SVG image
actually displayed by Folio, then closes it. Test removal of only that auxiliary
decode with stock UI styles. Preserve PNG/hash/size checks, enabled Apply, candidate
closure, exact Cancel restoration and two animation frames. This diagnostic does
not independently prove decoded display on each cycle; it specifically tests the
measurement workload. Preview timing in this arm excludes auxiliary decoding and
must not be presented as a product latency improvement. If growth remains, do
not infer an instrumentation cause; inspect a different boundary next.

## Allocation trace

The no-extra-decode arm also exceeds the same first-20 growth threshold. Stop
trying speculative UI changes. Capture Chromium memory-infra allocation dumps
in one stock constant arm at warmup, measured 20, measured 60, and closed idle 30s.
Use the running WebView's archived protocol schema. Set `deterministic:false`
explicitly (the protocol documents that true forces GC), no automatic triggers,
and no screenshot tracing. Persist the raw bounded trace and dump IDs, bind trace
PIDs to sampled process creation identities, and attribute growth only to fields
actually present. This is an instrumented diagnosis; its timings and totals do
not replace untraced acceptance. If pools are absent or attribution incomplete,
record that limitation rather than calling the cause established.
