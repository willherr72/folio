# Issue 15 acceptance status

v0.11.0 implements the first bounded stage: verified positioned single runs.

- B1: Same-font single-run edits preserve explicit spacing, affine transforms and
  inherited text state through a proven native or uniquely mapped stream path.
  Original encoding, font ownership, neighboring text, page geometry and visible
  output are checked before publishing a new immutable source.
- B2 remains open: explicitly selecting compatible adjacent groups. No grouping
  is inferred from proximity or a shared font name.
- B3 remains open: copying resources to isolate one selected nested-form
  occurrence. Text inside forms is still excluded; editing shared form resources
  globally is not enabled.

The page-level positioned route is intentionally conservative. Ambiguous operator
mapping and relative neighbor movement remain refusal cases. Font substitution
on explicitly spaced pages is not enabled. Existing shaped-text restrictions in
#14 also remain; this stage extends the verified unshaped Latin path.

See [editing behavior](existing-text-editing.md) and the
[independent reader report](issue15-reader-verification.json). Manual Foxit/IME
acceptance still belongs to the outstanding #14 work and is not inferred here.

The [repeated-edit resource check](issue15-resource-verification.json) performed
20 same-font positioned edits: each resulting PDF contained seven objects; byte
size stabilized at 947 after the initial 938-byte output. Closed source IDs became
unavailable and the original file stayed unchanged. These are fixture-level
ownership/object-growth checks, not a process working-set benchmark or an 8 GB
hardware acceptance result.
