# Raw dictionary-number fidelity

The current Document retains parsed values, not original numeric spellings.
Re-serializing it cannot prove original dictionary semantics. Extend the existing
opt-in bounded loader with a raw numeric-token predicate, then use it in all
five private redaction byte-loading routes. Ordinary PDF loading and unrelated
bounded loads remain unchanged.

The loader first proves the numeric parser consumed a whole PDF token, rather
than a prefix such as 1 from 1e2. The predicate receives the exact token plus its parsed
Integer/Real object. Run it after numeric object recognition, not on reference
IDs, xref offsets, names, strings or stream payloads. Dictionary arrays, scalar
indirect objects, trailers and decoded object streams use the same parser hook.
Latch predicate refusal in the existing bounded-load scope so backtracking or
ignored malformed objects cannot conceal it. Call predicates outside the
thread-local RefCell borrow; unwind and nested-load behavior preserve existing
scope guarantees. Keep aggregate allocation, node, decoder and cancellation
limits. The callback is opt-in and does not retain tokens or add source copies.

Redaction accepts integer dictionary values in signed32 range, including lengths
larger than the content-number one-million bound. Within that range both readers
and canonical writer preserve signed numeric values. Real dictionary numbers
use the established finite token/canonical-spelling proof. This global load
check may conservatively refuse an irrelevant incompatible numeric object;
record that compatibility debt rather than claiming a selective resource proof.

Add a common load_pdf helper and route source loading, re-opening and composition
through it. Before final fresh-graph publication, walk direct numeric objects
(including stream dictionaries and trailer) with depth/node limits and prove the
canonical spelling; do not decode stream bytes. Content numbers keep their
separate raw and final preflight checks. References need no recursive following
because all emitted indirect objects are visited. Raw sources must enter through
the proved loader before private preparation; owned in-memory test graphs have
canonical spelling.

Test direct and compressed dictionaries, inherited page boxes, safe long
spellings, bad underflow/truncation numbers, skipped string/name/stream bytes,
large integer lengths, rejected oversized signed integers, predicate refusal
latching, nested bounded calls, unwind cleanup and thread isolation. Demonstrate
failure before implementation and success after. Audit owned raw PDF page-box
cases with actual readers, plus full native redaction and prior corpus hashes.
Because a shared vendor parser is changed, run existing bounded-loader and
controlled-content lifecycle tests as well.

No JPEG public admission, package update or parity claim. Renderer sizing and
fractional/grid-fit/inverse placement, opaque paint and final phase3 review remain.
