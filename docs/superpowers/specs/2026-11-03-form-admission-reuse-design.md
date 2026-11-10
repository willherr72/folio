# Reuse successful source form admission

Reduce repeated whole-document validation when selecting text on an unchanged PDF. The native profiling checkpoint identifies duplicate form admission across visible pages as the first measured target. This is an internal Rust optimization; no PDF eligibility or UI behavior changes are intended.

Each `OpenDocument` owns two positive-only admission slots, one per annotations mode. `editing_document` must run before consulting them, so current permissions and print-only restrictions always apply. A slot is set only after both existing form-graph preflight and source admission succeed. Errors retain their original type/message and are retried. A fresh opened/generated/edited source starts with empty slots, even if its bytes equal another source. Closing the source drops the slots. Retain no parsed PDF, errors, output buffers, or global cache entries.

Source bytes are immutable in production: all seven OpenDocument constructors install new Arc bytes; assignments elsewhere are test corruption fixtures. Both predicates depend only on those bytes and the mode. Per-page eligibility, native copying, raster/glyph validation, and publication guards remain unchanged.

## Approaches

Coordinator choice under the user's autonomous execution authorization: per-document successful checks, rather than a global hash cache (ownership/eviction overhead) or retained parsed graphs (memory and loader-budget risks). Keep modes separate even where one could logically imply the other. Do not optimize publication's all-page reopening in this milestone.

Acceptance requires deterministic cache behavior tests; native permission, mode, failure and new-source isolation regressions; affected native regressions; optimized same-workload fidelity/performance evidence; and one fresh-context review. No speedup claim before measurement, and no commercial parity claim. Preserve the accepted GUI and historical evidence. Use the dedicated D: Cargo/TEMP directories.
