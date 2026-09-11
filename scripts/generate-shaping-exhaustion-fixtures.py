"""Build small hostile OpenType probes from the licensed DejaVu test font.

Requires fontTools 4.62.1; never run against fonts for normal distribution.
The resulting fonts contain intentionally recursive or excessive GSUB lookups.
"""
from pathlib import Path
import copy
import hashlib
import json
from fontTools import subset
from fontTools.ttLib import TTFont, newTable
from fontTools.ttLib.tables import otTables
from fontTools.otlLib.builder import buildLookup, buildCoverage

root = Path(__file__).resolve().parents[1]
output = root / "tests/fixtures/shaped-text/exhaustion"
output.mkdir(parents=True, exist_ok=True)

for mode in ("recursive", "budget", "expansion"):
    font = TTFont(root / "tests/fixtures/corpus/fonts/DejaVuSerif.ttf", recalcTimestamp=False)
    options = subset.Options()
    options.recalc_timestamp = False
    subsetter = subset.Subsetter(options=options)
    subsetter.populate(text="AB ")
    subsetter.subset(font)
    for tag in ("GSUB", "GPOS"):
        if tag in font:
            del font[tag]
    context = otTables.ContextSubst()
    context.Format = 3
    context.GlyphCount = 1
    context.Coverage = [buildCoverage(["A"], font.getReverseGlyphMap())]
    context.SubstCount = 1 if mode == "recursive" else 8192
    context.SubstLookupRecord = []
    for index in range(context.SubstCount):
        record = otTables.SubstLookupRecord()
        record.SequenceIndex = 0
        record.LookupListIndex = 0 if mode == "recursive" else (2 if index == context.SubstCount - 1 else 1)
        context.SubstLookupRecord.append(record)
    lookups = [buildLookup([context])]
    if mode == "budget":
        identity = otTables.SingleSubst()
        identity.mapping = {"A": "A"}
        lookups.append(buildLookup([identity]))
        final = otTables.SingleSubst()
        final.mapping = {"A": "B"}
        lookups.append(buildLookup([final]))
    if mode == "expansion":
        lookups = []
        for _ in range(9):
            multiply = otTables.MultipleSubst()
            multiply.mapping = {"A": ["A", "A"]}
            lookups.append(buildLookup([multiply]))
        for _ in range(9):
            collapse = otTables.LigatureSubst()
            ligature = otTables.Ligature()
            ligature.LigGlyph = "A"
            ligature.Component = ["A"]
            ligature.CompCount = 2
            collapse.ligatures = {"A": [ligature]}
            lookups.append(buildLookup([collapse]))

    gsub = newTable("GSUB")
    gsub.table = otTables.GSUB()
    gsub.table.Version = 0x10000
    gsub.table.ScriptList = otTables.ScriptList()
    script = otTables.ScriptRecord()
    script.ScriptTag = "DFLT"
    script.Script = otTables.Script()
    script.Script.LangSysRecord = []
    script.Script.LangSysCount = 0
    language = otTables.LangSys()
    language.LookupOrder = None
    language.ReqFeatureIndex = 0xFFFF
    language.FeatureIndex = [0]
    language.FeatureCount = 1
    script.Script.DefaultLangSys = language
    gsub.table.ScriptList.ScriptRecord = [script]
    gsub.table.ScriptList.ScriptCount = 1
    gsub.table.FeatureList = otTables.FeatureList()
    feature = otTables.FeatureRecord()
    feature.FeatureTag = "liga"
    feature.Feature = otTables.Feature()
    feature.Feature.FeatureParams = None
    feature.Feature.LookupListIndex = list(range(len(lookups))) if mode == "expansion" else [0]
    feature.Feature.LookupCount = len(feature.Feature.LookupListIndex)
    gsub.table.FeatureList.FeatureRecord = [feature]
    gsub.table.FeatureList.FeatureCount = 1
    gsub.table.LookupList = otTables.LookupList()
    gsub.table.LookupList.Lookup = lookups
    gsub.table.LookupList.LookupCount = len(lookups)
    font["GSUB"] = gsub
    destination = output / f"{mode}.ttf"
    font.save(destination)
    print(f"{destination.name}: {destination.stat().st_size} bytes")

# A small acyclic composite DAG expands to 16,384 copies of the original A.
# Disable fontTools' own bounds/maxp traversal; each component has zero offset.
font = TTFont(output / "expansion.ttf", recalcTimestamp=False, recalcBBoxes=False)
del font["GSUB"]
from fontTools.ttLib.tables._g_l_y_f import Glyph, GlyphComponent
order = font.getGlyphOrder()
font["glyf"]["leaf"] = copy.deepcopy(font["glyf"]["A"])
font["hmtx"]["leaf"] = font["hmtx"]["A"]
order.append("leaf")
child = "leaf"
for index in range(14):
    name = "A" if index == 13 else f"branch{index}"
    glyph = Glyph()
    glyph.numberOfContours = -1
    glyph.xMin, glyph.yMin, glyph.xMax, glyph.yMax = (-12, 0, 1499, 1493)
    glyph.components = []
    for _ in range(2):
        component = GlyphComponent()
        component.glyphName, component.x, component.y, component.flags = child, 0, 0, 0
        glyph.components.append(component)
    font["glyf"][name] = glyph
    font["hmtx"][name] = font["hmtx"]["A"]
    if name != "A":
        order.append(name)
    child = name
font.setGlyphOrder(order)
font["maxp"].numGlyphs = len(order)
font.save(output / "composite.ttf")

source = root / "tests/fixtures/corpus/fonts/DejaVuSerif.ttf"
manifest = {
    "generator": "scripts/generate-shaping-exhaustion-fixtures.py",
    "fontTools": "4.62.1",
    "source": "../../corpus/fonts/DejaVuSerif.ttf",
    "source_sha256": hashlib.sha256(source.read_bytes()).hexdigest(),
    "license": "LICENSE_DEJAVU.txt",
    "files": {p.name: {"sha256": hashlib.sha256(p.read_bytes()).hexdigest(), "bytes": p.stat().st_size}
              for p in sorted(output.glob("*.ttf"))},
}
(output / "manifest.json").write_text(json.dumps(manifest, indent=2) + "\n", encoding="utf-8")
