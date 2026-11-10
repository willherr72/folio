# Native image asset foundation implementation plan

> **For agentic workers:** Use superpowers:executing-plans inline; one fresh focused review at integration.

**Goal:** Provide immutable, bounded PNG/JPEG assets with independent native leases
as the first prerequisite for the complete image-placement workflow.

**Architecture:** A focused image_assets module parses original encoded bytes,
returns orientation-correct decoded pixels without alpha/bit-depth flattening, and
owns registry entries through opaque acquisition tokens. No editor controls yet.

**Tech stack:** Rust, image0.25.10 PNG decoder, locally patched jpeg-decoder0.3.2
with strict entropy consumption, existing crc32fast1.5.1, SHA256, UUID and Mutex/Arc.

**Spec:** `docs/superpowers/specs/2026-10-23-image-placement-design.md`.

## Constraints and review focus

- Retain original bytes; identical input shares identity but never shares release tokens.
- Per-image64 MiB,32M pixels,32,768 dimensions. PNG gets a best-effort256 MiB
  decoder hint; JPEG has no total-allocation limiter. These are admission bounds,
  not a guaranteed per-decode memory ceiling (especially progressive JPEG).
- Registry64 unique assets,256 MiB encoded,512 MiB decoded sample sizes,4096 leases.
- Validate complete batches before publication; releases are idempotent and do not
  invalidate active Arc borrows. No changes to packaged0.16 or existing image-page conversion.
- Reject malformed bytes despite valid headers; check dimensions/pixels before decode.
  Use strict JPEG entropy decoding and verify PNG framing/CRC through terminal IEND.
- Preserve alpha and16-bit PNG values, EXIF orientation and original profile metadata.

## Task1: Asset and ownership implementation

Files: new `src-tauri/src/image_assets.rs`, `src-tauri/tests/image_assets.rs`,
private capacity tests `src-tauri/src/image_assets_tests.rs`; exports in `lib.rs`.
Decoder review also requires the pinned JPEG fork/provenance, fixture generator and
independent reference corpus, plus license collection support for the fork's notices.

Interfaces: ImageAsset::parse(Vec<u8>), ImageAsset::read(&Path), info(), bytes(),
decode()->DecodedImage; ImageRegistry::register(Vec<u8>), register_assets(&[Arc<ImageAsset>]),
acquire(&str), get(&str), release(&str), release_all(&[ImageRegistration]), ids().
ImageRegistration {image:ImageInfo,token:String}; ImageInfo includes id,width,height,
mimeType,hasAlpha,bitsPerChannel,byteLength. Parsed asset fields are private.

- [x] Write public behavior tests and observe missing-module RED.
- [x] Implement bounded parsing/path read and original-byte identity; decode metadata
  and pixels without dropping alpha or16-bit samples. Observe parser tests GREEN.
- [x] Implement registry dedup/leases and atomic capacity accounting. Test duplicate
  and concurrent owners, stale releases, outstanding Arc reads and batch rollback.
- [x] Add private small-budget boundary tests for count/encoded/decoded/lease limits;
  confirm rejection leaves existing owners and capacity intact.

## Task2: Integration evidence and handoff

- [x] Run asset tests, existing image-page/font resource suites, and compile app.
- [x] Fresh review, reproduce/fix material findings, rerun relevant checks.
- [x] Record exact completed foundation and remaining native persistence/UI work;
  preserve the full image-placement spec and broader goal.

Completed evidence:13 public asset tests,3 capacity tests,4 image-page tests and6
font-registry tests pass; Windows custom-protocol app build passes. Fifteen original
JPEG fixtures match independent Pillow/libjpeg pixels within3 channel values. Fresh
review's malformed-image findings reproduced and fixed; additional focused review
of the local entropy patch found no blockers. Pinned archive/source verification and
all five license/provenance notice copies pass (566 dependency records).
See `artifacts/image-placement-progress.md` and `artifacts/image-assets-verification.json`.
No image-placement UI, native PDF appearance or recovery integration is claimed.
