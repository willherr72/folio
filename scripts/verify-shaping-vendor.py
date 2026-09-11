"""Verify the pinned HarfRust inventory and the two documented local changes.

This is an offline integrity check. The provenance also records the downloaded
crate archive hash and upstream commit used for the independent source review.
"""
import hashlib
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
VENDOR = ROOT / "src-tauri/vendor/harfrust-0.13.3"
manifest = json.loads((VENDOR / "FOLIO_PROVENANCE.json").read_text(encoding="utf-8"))
expected = dict(manifest["upstream_files_sha256"])
for change in manifest["changed_files"]:
    if expected.get(change["file"]) != change["upstream_sha256"]:
        raise SystemExit(f"Inconsistent patch provenance: {change['file']}")
    expected[change["file"]] = change["patched_sha256"]

owned = {"FOLIO_PROVENANCE.json", "FOLIO_PATCH.patch", "FOLIO_VENDOR.md"}
actual = {p.relative_to(VENDOR).as_posix() for p in VENDOR.rglob("*") if p.is_file()}
if actual != set(expected) | owned:
    raise SystemExit(f"Vendor inventory mismatch: {sorted(actual ^ (set(expected) | owned))}")
for name, digest in expected.items():
    path = VENDOR / name
    if not path.resolve().is_relative_to(VENDOR.resolve()):
        raise SystemExit("Vendor manifest contains an invalid path")
    if hashlib.sha256(path.read_bytes()).hexdigest() != digest:
        raise SystemExit(f"Vendor checksum mismatch: {name}")
print(f"Verified {len(expected)} upstream files with {len(manifest['changed_files'])} documented patches.")
