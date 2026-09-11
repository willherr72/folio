"""Independent inspection of the owned desktop smoke output (PyMuPDF + pypdf)."""
from pathlib import Path
import hashlib
import json
import fitz
from pypdf import PdfReader

root = Path("artifacts/text-edit-desktop")
results = json.loads((root / "results.json").read_text())
original = root / "Moved original.pdf"
saved = root / "Folio edited.pdf"
assert hashlib.sha256(original.read_bytes()).hexdigest() == results["originalSha256"]
reader = PdfReader(saved)
text = reader.pages[0].extract_text()
assert "New sentence with more detail" in text
assert "Original sentence" not in text
assert "Keep this neighbor" in text
annotations = reader.pages[0]["/Annots"]
assert len(annotations) == 1
annotation = annotations[0].get_object()
font = annotation["/AP"]["/N"]["/Resources"]["/Font"]["/FolioFont"]["/BaseFont"]
assert font == "/Courier-BoldOblique"
before, after = fitz.open(original), fitz.open(saved)
old_bounds = before[0].search_for("Original sentence")[0]
new_bounds = after[0].search_for("New sentence with more detail")[0]
assert new_bounds.x1 > old_bounds.x1 + 20
allowed = old_bounds | new_bounds
allowed = fitz.Rect(allowed.x0 - 4, allowed.y0 - 4, allowed.x1 + 4, allowed.y1 + 4)
old = before[0].get_pixmap(alpha=False, annots=False)
new = after[0].get_pixmap(alpha=False, annots=False)
assert (old.width, old.height) == (new.width, new.height)
old_bytes, new_bytes = old.samples, new.samples
changed = outside = 0
for y in range(old.height):
    for x in range(old.width):
        index = (y * old.width + x) * 3
        if old_bytes[index:index + 3] != new_bytes[index:index + 3]:
            changed += 1
            outside += not allowed.contains(fitz.Point(x, y))
assert changed > 0 and outside == 0, (changed, outside)
after[0].get_pixmap(matrix=fitz.Matrix(1.5, 1.5)).save(root / "independent-render.png")
report = {
    "passed": True, "pypdfText": text, "annotationFont": font,
    "originalRightEdge": old_bounds.x1, "replacementRightEdge": new_bounds.x1,
    "changedPixels": changed, "changesOutsideTextBounds": outside, "originalUnchanged": True,
}
(root / "independent-verification.json").write_text(json.dumps(report, indent=2) + "\n")
print(json.dumps(report, indent=2))
