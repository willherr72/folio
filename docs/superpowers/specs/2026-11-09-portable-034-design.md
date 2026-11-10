# Folio 0.34 local development package

## Purpose and authority

Deliver the retained-source storage milestone in a combined Windows desktop and
CLI/MCP portable. The user has authorized continued autonomous development;
the choices here are coordinator decisions under that authority. Finish and seal
the retained-source acceptance before changing any product input.

The accepted 0.33 archive, extracted accepted binaries, failed runs, original
PDFs and current source-qualified GUI remain preserved. This is a local
development package, with no public release, push or merge.

## Approach

Use the existing versioned portable builder and verifier. A separate ad hoc GUI
copy would omit CLI/resources and obscure which version has the storage work.
A new installer/update mechanism would add unrelated behavior. Increment only
the five version metadata files from 0.33.0-dev.1 to 0.34.0-dev.1; qualify all
other implementation and renderer inputs byte-for-byte against the completed
storage milestone. Update distribution documentation and the checkout launcher
only at the appropriate preparation/acceptance stages.

Preserve the current miniz codec, 100-action history, four idle native readers,
64 MiB encoded backing budget, 128 MiB raw-cache target and existing large-source
exemption. There is no further engine or UI feature in this packaging milestone.

## Verification

- Bind the sealed storage checkpoint, correction red/green, complete native
  library/integration input membership and shared fixtures. Version-only reuse
  must explicitly distinguish the opt-level=0 Folio library correctness result
  from optimized integration/workload evidence. Never relabel old runs.
- Require an exact five-file version transition and unchanged remaining product,
  renderer and test membership. Reuse the existing qualifier's negative controls
  for implementation, metadata and membership drift. Run the complete frontend
  suite against the package preparation, with captured inputs.
- Build optimized GUI and CLI serially in the existing D: cache. Freeze product
  and distribution inputs during build and acceptance. Verify OCR assets,
  renderer provenance, licenses, ZIP membership and checksums; extract outside
  the checkout and test the extracted binaries.
- Exercise the established CLI/MCP inspection/edit/paragraph/font/form/session
  workflows, desktop text/paragraph/OCR and current-state recovery. Audit output
  membership, saved PDF semantics/paint and owned-process cleanup using the
  existing qualified helpers. A failed workflow blocks dependent acceptance.
- Include sustained Undo/Redo and preview ownership checks on the extracted
  application. Previously measured storage memory is bound to its original
  binary; packaging acceptance does not upgrade that evidence into a new
  performance measurement.
- Obtain one final packaging review after the concrete package and evidence
  exist. Correct consequential findings with reproductions; preserve all review
  reports and failures. Promote the checkout launcher only after acceptance.

## Boundaries

Broad producer compatibility, physical printing, low-memory hardware, signatures,
accessibility, whole-process peak limits and Adobe/Foxit parity remain separate
work. Durable recovery restores the current state, not historical Undo states or
power-loss durability. Direct sealing/write/flush fault injection remains a
recorded storage coverage gap. No new claim is inferred from a version bump.
