"""Verify pinned licensed fonts and Unicode data; --fetch restores missing assets.

Only downloads the exact immutable manifest URLs. Validates size and SHA256
before writing. Existing mismatched files cause failure rather than overwrite.
"""
import argparse
import hashlib
import json
from pathlib import Path
import urllib.request

ROOT = Path(__file__).resolve().parents[1]
FIXTURES = ROOT / "tests/fixtures/shaped-text"
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("--fetch", action="store_true")
args = parser.parse_args()
manifest = json.loads((FIXTURES / "provenance.json").read_text(encoding="utf-8"))
for item in manifest["assets"]:
    path = FIXTURES / item["file"]
    assert path.resolve().parent == FIXTURES.resolve(), "Invalid fixture path"
    if path.exists():
        data = path.read_bytes()
    elif args.fetch:
        with urllib.request.urlopen(item["url"], timeout=30) as response:
            data = response.read(2 * 1024 * 1024 + 1)
    else:
        raise SystemExit(f"Missing {path}; use --fetch")
    assert len(data) == item["bytes"], f"Size mismatch: {path}"
    assert hashlib.sha256(data).hexdigest() == item["sha256"], f"Hash mismatch: {path}"
    if not path.exists():
        path.write_bytes(data)
    print(f"Verified {item['file']} ({len(data)} bytes)")

unicode_root = FIXTURES / "unicode"
unicode_manifest = json.loads((unicode_root / "provenance.json").read_text(encoding="utf-8"))
for item in unicode_manifest["assets"]:
    path = unicode_root / item["file"]
    assert path.resolve().parent == unicode_root.resolve(), "Invalid Unicode fixture path"
    if path.exists():
        data = path.read_bytes()
    elif args.fetch:
        with urllib.request.urlopen(item["source"], timeout=30) as response:
            data = response.read(16 * 1024 * 1024 + 1)
        assert len(data) == item["source_bytes"], f"Source size mismatch: {path}"
        assert hashlib.sha256(data).hexdigest() == item["source_sha256"], f"Source hash mismatch: {path}"
        if "selected_test_rows" in item:
            lines = data.decode("utf-8").splitlines()
            rows = [i for i, line in enumerate(lines) if line.strip() and not line.startswith("#")]
            count = len(rows)
            indices = sorted(set(range(16)) | set(range(count - 16, count)) |
                             {i * (count - 1) // 255 for i in range(256)})
            selected = [rows[i] for i in indices]
            assert [i + 1 for i in selected] == item["selected_source_line_numbers_1based"]
            comments = ["# Folio development subset: first/last16 test rows plus256 equally spaced row indices.",
                        "# Complete selection algorithm and source SHA256 are in provenance.json."]
            data = ("\n".join(lines[:rows[0]] + comments + [lines[i] for i in selected]) + "\n").encode("utf-8")
    else:
        raise SystemExit(f"Missing {path}; use --fetch")
    assert len(data) == item["bytes"], f"Size mismatch: {path}"
    assert hashlib.sha256(data).hexdigest() == item["sha256"], f"Hash mismatch: {path}"
    if not path.exists():
        path.write_bytes(data)
    print(f"Verified unicode/{item['file']} ({len(data)} bytes)")
