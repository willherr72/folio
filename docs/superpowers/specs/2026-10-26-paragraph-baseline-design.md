# Preserve fresh paragraph baselines

The supplied quiz shifts its edited first line upward3pt. Both the prior accepted
EXE and current choice checkpoint reproduce identical output. Fresh selection
bounds use source baseline + em while styled layout subtracts measured ascent.

Preserve the verified source first baseline when suggesting an area for a fresh
styled paragraph. Measure the actual first wrapped line at the proposed width;
set raw-PDF top to source baseline + its ascent. Keep the existing bottom unless
fitting needs more height. If width changes which styles occupy the first line,
recompute the top before checking page/obstacle containment and fit. Reuse the
same wrapping/extent algorithm as publication. Explicit user-positioned areas and
verified saved paragraph memory keep their existing semantics.

No font-specific offset, font substitution, weakened collision checks, changes to
source originals, or changes to page rotation/UserUnit handling. The initial-area
convenience remains optional when source metrics cannot be proven. Public preview
and saved output must retain baseline within0.001raw units for supported fixtures.

Validate uniform and mixed fonts, different first-line heights after wrapping,
0/90/180/270 rotation, crop origin, UserUnit1/1.7/2, obstacles, page edges, saved
re-edit and the original quiz. Verify unchanged surrounding text/pixels. Reading
order from appended PDF operators is a separate follow-up; recheck its marker
ordering after vertical placement is corrected.

Decision is coordinator-owned under the user's standing autonomous-work request.
The existing isolated dirty worktree and accepted executables must be preserved.
