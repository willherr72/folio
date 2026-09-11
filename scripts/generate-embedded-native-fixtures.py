"""Regenerate owned native issue-12 fixtures; run from the repository root.

Requires ReportLab 4.5.1 and fontTools 4.62.1. Test execution itself only
consumes the checked-in fixtures and needs neither Python package.
"""
from pathlib import Path

from fontTools import subset
from fontTools.ttLib import TTFont as FontToolsFont
from reportlab.pdfbase import pdfmetrics
from reportlab.pdfbase.ttfonts import TTFont
from reportlab.pdfgen import canvas

source = Path("tests/fixtures/corpus/fonts/DejaVuSerif.ttf")
output = Path("tests/fixtures/embedded-text")
output.mkdir(parents=True, exist_ok=True)

pdfmetrics.registerFont(TTFont("FixtureSerif", str(source)))
document = canvas.Canvas(str(output / "reportlab-subset.pdf"),
                         pagesize=(300, 400), pageCompression=0, invariant=1)
document.setFont("FixtureSerif", 16)
document.setFillColorRGB(.2, .4, .6)
document.drawString(30, 300, "Café old")
document.drawString(30, 240, "Neighbor")
document.save()

options = subset.Options()
options.recalc_timestamp = False
font = subset.load_font(str(source), options)
subsetter = subset.Subsetter(options=options)
subsetter.populate(text="Old Cafe Edit café Neighbor safe ")
subsetter.subset(font)
subset.save_font(font, str(output / "DejaVuSerif-subset.ttf"), options)

# Deliberately inconsistent Unicode subtables: the source word "Old" remains
# unchanged, but Windows maps a newly introduced E to the outline for O.
for kind in ("conflicting", "missing"):
    font = FontToolsFont(output / "DejaVuSerif-subset.ttf", recalcTimestamp=False)
    for table in font["cmap"].tables:
        if table.platformID == 3:
            table.cmap = table.cmap.copy()
            if kind == "conflicting":
                table.cmap[ord("E")] = table.cmap[ord("O")]
            else:
                del table.cmap[ord("E")]
    font.save(output / f"DejaVuSerif-{kind}-cmap.ttf")
