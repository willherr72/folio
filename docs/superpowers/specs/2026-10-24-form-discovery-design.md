# Avoid empty form-text projection

The desktop requests top-level and reusable-form text for every selected page.
The 100/400/1600-piece corpus found that form discovery unnecessarily copies and
validates ordinary pages: at 1600 top-level pieces, top listing took 159 ms and
empty form listing another 159 ms in this debug build. Actual form listing took
620 ms. These synthetic measurements establish wasted work, not general latency.

Under the user's standing autonomous-work instruction, retain the existing native
worker and immutable sources. After full-source form admission and text permission
checks, inspect the requested native page's immediate object types. If it has no
Form XObject, return an empty successful list without creating a private page.
Any nested form necessarily has a top-level form ancestor. Do not traverse nested
objects on this fast path. Source tags/layers, permissions and malformed/over-budget
graphs must still be refused before traversal. Invalid pages/sources remain errors.
Pages with actual forms use the exact existing projection and validation path.

Alternatives: retain the extra copy (no behavior risk but measured wasted latency),
cache complete listing (memory/invalidation complexity), or combine desktop APIs
(larger integration change without eliminating unnecessary no-form validation).
Page presence detection is the selected bounded change. It may deliberately return
empty success where unrelated projection checks previously failed on a no-form
page; no editable text is authorized by an empty result.

Acceptance: deterministic ordinary/no-form tests including an unused form resource,
form on another page, annotation appearances, invalid pages, protected sources and
catalog restrictions. Existing form tests prove the nonempty route. Retained before
and after corpus timings include all counts/candidates and source immutability;
no flaky timing assertion. Verify the built desktop and record measured limits.

Nested-form grouping/paragraph work remains open. This optimization does not relax
any mutation proof, claim arbitrary producer compatibility, or establish parity.
