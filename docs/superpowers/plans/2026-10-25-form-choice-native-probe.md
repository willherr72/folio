# Isolated native choice appearances

Execute inline using the existing choice-editor spec and TDD. Preserve prior
evidence, input PDFs and binaries. No commits, no concurrent Cargo/builds, and no
public endpoint until the complete publication proof exists.

## Design

Generate one widget in a private one-page/one-field PDF. Use the validated source
Context and its inspected bounded ChoiceModel. Resolve the widget's actual inherited DA, use
its saved normal-appearance font resource before the form DR, and copy the exact
font graph with Copier. Admit one explicit DA font and the supported ordered color
operators. Copy only widget presentation settings (Rect/MK/BS/Border/F), never
source actions, page tree or field ownership. Encode normalized option labels and
top index, but preserve staged raw V/I/DV operands and absence in the probe. A
missing combo V can display DV; it is not equivalent to explicit null. NeedAppearances=true belongs only to
this private document; the original form's global policy remains unchanged.

Open via NativeForm so page loading creates missing appearances. Add a bounded
private snapshot operation, avoiding focus/fill hacks for empty combos. The
existing fill path must still restore its masked actions. Reuse BoundedWriter in
the native callback; record callback failures even if the native save reports
success. Cap probes to32MiB; graph copy limits remain16MiB and parsed AP256KiB.

Return a crate-private generated document and normal appearance plus verified
font-name mapping. Every generated font resource and every used Tf must bind to
the original selected font graph via Equivalence. Missing/fallback/changed fonts
fail precisely. This proves only fonts present in the generated appearance; it
does not prove unseen option coverage, no-op pixels, BBox/Matrix equivalence or
source NeedAppearances admission. The future publication caller must do all of
those, then use ChoiceTextBlock to transplant text. Do not copy entire generated
APs into the source. Empty list APs may have no Tx block; handle that explicitly at
the integration layer rather than confusing missing text with generation failure.

## Task1 — native generator and bounded snapshot

Files: new form_choice_probe.rs and engine_choice_probe_tests.rs; modify lib.rs,
engine.rs test wiring, form_fill.rs snapshot/callback implementation, and the
form_choice_draft.rs default-fallback admission/repaint correction.
Interfaces: generate_choice_appearance(bindings,Context,widget_id)
returns private GeneratedChoiceAppearance{document,appearance,font_names};
NativeForm::snapshot_private(limit) returns sanitized private bytes only.

- [x] Archive baseline and observe missing-API RED for real native tests.
- [x] Implement bounded native save and isolated probe with original-font binding.
- [x] Verify generated dropdown/list/empty/free-text appearances, linked widget
  locations, local-vs-global font conflicts, fallback refusal, byte-budget refusal,
  complete embedded font identity and unchanged source graphs.
- [x] Compare no-op/changed text transplantation in native and independent readers;
  rerun library/choice/property and affected fill/reflow regressions.

Review focus: never use a font name as identity; preserve unrelated widget fonts;
no script/action execution; bounded output failures cannot return truncated bytes;
empty selection must survive without a focus event; an unused newly introduced
character is not covered by a current-appearance proof. Whole-feature review is
still pending until UI/history/recovery/publication integration is complete.
