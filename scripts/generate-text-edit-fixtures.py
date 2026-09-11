"""Generate owned standard-font and embedded-font PDFs for desktop editing checks."""
from pathlib import Path
import fitz
out=Path("artifacts/text-edit-desktop")
out.mkdir(parents=True,exist_ok=True)
for name,font in [("Original.pdf","helv"),("Unsupported.pdf","embedded")]:
    path=out/name
    if path.exists():
        raise SystemExit(f"Fixture already exists; use the retained run or a fresh worktree: {path}")
    doc=fitz.open()
    page=doc.new_page(width=400,height=500)
    if font=="embedded":
        page.insert_font(fontname=font,fontfile="tests/fixtures/corpus/fonts/DejaVuSerif.ttf")
    page.insert_text((50,100),"Original sentence",fontname=font,fontsize=16,color=(.1,.2,.6))
    page.insert_text((50,160),"Keep this neighbor",fontname="helv",fontsize=12)
    doc.save(path)
