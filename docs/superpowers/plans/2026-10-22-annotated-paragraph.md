# Annotated paragraph editing
Use superpowers:executing-plans inline; autonomous user authorization.
Spec: ../specs/2026-10-22-annotated-paragraph-design.md
1. Native render_page_plan request/API/command + adapter. RED -> GREEN tests for
   composed appearances, source immutability, geometry and validation errors.
2. Dialog/preview accept overlays and use native composition; remove App blanket
   refusal only when compositor available. RED -> GREEN app/dialog tests for ownership,
   preserved overlays, original/candidate plans and failed/late render cleanup.
3. Targeted native suites, full frontend, build, browser/desktop acceptance and one
   fresh review. Fix material findings with regressions; update capability record.
Review focus: exact overlay appearance and retained custom fonts; no duplicated
annotations on save; preview coordinate axes; source immutability/temp cleanup;
late raster and candidate ownership; failure never enables Apply; per-tab history.
