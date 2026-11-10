"""Verify pinned pdfium-render sources; --refresh records intentional local source changes.

Uses a cached crates.io archive or --archive PATH. Never downloads or extracts files.
"""
from pathlib import Path
import argparse
import difflib
import hashlib
import json
import os
import tarfile

ROOT = Path(__file__).resolve().parent.parent
VENDOR = ROOT / "src-tauri/vendor/pdfium-render-0.9.4"
ARCHIVE_HASH = "8948a803616a9e936b15a6637af2cd48c5fb8ae0fcdeb3c32eac3a540e255a19"
ADDED = [{"path": "LICENSE-APACHE",
          "source": "https://www.apache.org/licenses/LICENSE-2.0.txt",
          "sha256": "cfc7749b96f63bd31c3c42b5c471bf756814053e847c10f3eb003417bc523d30"}]


def digest(data):
    return hashlib.sha256(data).hexdigest()


def require(condition, message):
    if not condition:
        raise SystemExit(message)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--archive", type=Path)
    parser.add_argument("--refresh", action="store_true")
    args = parser.parse_args()
    archive = args.archive
    if archive is None:
        cargo_home = Path(os.environ.get("CARGO_HOME", Path.home() / ".cargo"))
        archive = next((cargo_home / "registry/cache").glob("*/pdfium-render-0.9.4.crate"), None)
    require(archive is not None, "Pass --archive PATH to the pinned pdfium-render-0.9.4.crate.")
    require(digest(archive.read_bytes()) == ARCHIVE_HASH, "Pinned archive checksum mismatch.")
    patch, records = [], []
    with tarfile.open(archive) as package:
        for member in package.getmembers():
            if not member.isfile():
                continue
            relative = Path(*Path(member.name).parts[1:])
            require(not relative.is_absolute() and ".." not in relative.parts, "Invalid archive path.")
            original = package.extractfile(member).read()
            current = (VENDOR / relative).read_bytes()
            if current == original:
                continue
            # Patch text is portable LF; provenance records exact source bytes.
            before = original.decode("utf-8").replace("\r\n", "\n").splitlines(keepends=True)
            after = current.decode("utf-8").replace("\r\n", "\n").splitlines(keepends=True)
            patch.extend(difflib.unified_diff(before, after,
                fromfile="a/" + relative.as_posix(), tofile="b/" + relative.as_posix()))
            records.append({"path": relative.as_posix(), "upstreamSha256": digest(original),
                            "patchedSha256": digest(current)})
    for entry in ADDED:
        require(digest((VENDOR / entry["path"]).read_bytes()) == entry["sha256"],
                "Added file checksum mismatch.")
    provenance = {"package": "pdfium-render", "version": "0.9.4", "archiveSha256": ARCHIVE_HASH,
                  "source": "https://crates.io/api/v1/crates/pdfium-render/0.9.4/download",
                  "modifiedFiles": records, "addedFiles": ADDED}
    patch_text = "".join(patch)
    if args.refresh:
        (VENDOR / "FOLIO_PATCH.patch").write_text(patch_text, encoding="utf-8", newline="\n")
        (VENDOR / "FOLIO_PROVENANCE.json").write_text(
            json.dumps(provenance, indent=2) + "\n", encoding="utf-8", newline="\n")
    else:
        require(json.loads((VENDOR / "FOLIO_PROVENANCE.json").read_text(encoding="utf-8"))
                == provenance, "Provenance mismatch; inspect changes before --refresh.")
        require((VENDOR / "FOLIO_PATCH.patch").read_text(encoding="utf-8") == patch_text,
                "Recorded patch differs from maintained source.")
    print(f"Verified pinned archive, {len(records)} modified files and {len(ADDED)} added files.")


if __name__ == "__main__":
    main()
