# Request-local parsing reuse for text-page preparation

The completed text-discovery profile measured four AcroForm source parses across two page preparations, with a median 240.98 ms exclusive parsing cost on repeated discovery of the 300-page fixture. Resource checks and form projection independently parse identical immutable bytes with the same strict parser and 64 MiB decompression limit.

Use one SourceForm for each text_transaction_page_copy call, enabling its existing bounded parse reuse. Pass that same source through resource admission/repair and both form routes. Keep the isolated route's shared-field refusal. Projection must continue to publish only through the enclosing complete-source transaction. Permissions, group checks, resource budgets, form validation and final text/raster checks remain unchanged.

Approaches: persistent parsed documents could save more but increase retained-history memory and require invalidation policy; combining context readers would change their stricter limits. Request-local SourceForm reuse fits the measured duplicate work without either change. This is the implementer's choice under the user's standing autonomous authorization.

Retention remains limited to encoded inputs <=8 MiB and <=100,000 parsed objects, ending at function return. This is not a heap-byte guarantee; large streams retain existing parser limits. Larger/object-dense inputs stay uncached. Never retain errors or bypass fresh mapping/graph validation. Refactor the two internal form helper signatures to consume the same SourceForm; require exactly one mapping to output page zero, so the source identity and page selection cannot disagree.

Proof: a test-only thread-local counter bound to the actual immutable source slice must show exactly one strict AcroForm parse per eligible page preparation, in both routes, with and without forms and resource repairs. Existing bounded-cache tests plus mapping/refusal cases must pass. Relevant complete-source, forms, resource preservation, text and paragraph integration tests must pass. Performance timing and GUI acceptance remain separate gates; reduced parse count alone is not a quantified desktop speedup.
