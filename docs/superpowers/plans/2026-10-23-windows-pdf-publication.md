# Windows PDF publication reliability plan

> **For agentic workers:** Use superpowers:executing-plans for inline execution.

**Goal:** Tolerate brief Windows denial of the final PDF rename without exposing
partial output, changing source protection, or hiding a persistent failure.

**Architecture:** Keep the same synced temporary file and atomic replacement path.
On Windows only, recover the temporary file from `PersistError` and retry the final
publication for access/sharing/lock violations, at most ten 25ms delays. Other errors
and non-Windows behavior return immediately. Exhaustion drops the owned temporary
and returns a contextual file error. Never retry composition, rewrite the existing
destination in place, or change permissions. User authorization covers this routine
reliability fix; no approval gate is needed.

**Spec/evidence:** This plan. During the memory optimization's full-suite gate,
`images_roundtrip_all_crop_page_workspace_and_image_rotations` repeatedly failed on
Windows error5. Diagnostic `image-publish-repeat-2.log` locates it in final persist,
with a writable existing destination. Exact cases and some whole-module runs pass;
the origin of the external denial is not proven. A controlled handle preventing
delete/rename will reproduce the same publication behavior before implementation.

The local tempfile3.27 Windows implementation uses `MoveFileExW` with replacement.
Microsoft documents that delete-sharing controls rename access and that replacement
requires access rights: [CreateFileW](https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-createfilew),
[MoveFileExW](https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-movefileexw).
The retry treats error5 as potentially temporary, not proof that its cause is a
sharing lock. A persistent access denial remains an error after the bounded delay.

## Tasks

- [x] Add real Windows tests in `pdf_output.rs`: briefly held destination handle,
  permanently held handle, and read-only destination. Check old bytes while locked,
  replacement after release, failure preservation and temporary-file cleanup.
  Record transient-lock RED before changing publication.
- [x] Add the bounded persist-only retry and retain operation-specific diagnostics.
- [x] Run output unit tests and the entire image module, including its64 rotation
  cases, then full native suite. Keep original failure logs; do not weaken image
  assertions or globally serialize unrelated tests to obtain a pass.
- [x] One scoped review of this new publication delta and tests. The separate
  export-memory lifetime review has already completed without findings.
- [x] Refresh source fingerprint; repeat memory verification and build/desktop
  save/reopen checks for the final combined source. Preserve previous executables.

## Review focus

No partial destination on failure; bounded attempts and allowed error codes;
temporary ownership after failed persist; unchanged source overwrite/rights checks;
permanent access failures; non-Windows behavior; realistic controlled lock tests.
