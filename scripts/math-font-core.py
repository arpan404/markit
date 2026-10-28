#!/usr/bin/env python3
"""Builds the core STIX Two Math font that the web package ships.

Every table except CFF is copied unchanged, so glyph ids, advances, kerning,
MATH constants, variants, and GSUB alternates are identical to the full font.
Glyphs not listed in math-core-glyphs.txt keep their exact bounding box but
lose their outline: the outline becomes one line between the box corners.
Hints and glyph names are dropped: the CFF is CID-keyed.
Layout reads only metrics and bounding boxes, so every layout is identical.
Only drawing a dropped glyph would differ. The Wasm build checks each layout
for dropped glyphs and loads the full font before drawing one.

usage: scripts/math-font-core.py FULL.otf GLYPHS.txt OUT.otf OUT-GLYPHS.bin   (needs fonttools)
"""
import io
import sys
from fontTools.ttLib import TTFont
from fontTools.pens.boundsPen import ControlBoundsPen
from fontTools.misc.psCharStrings import T2CharString
from fontTools import subset
from fontTools.cffLib import FDArrayIndex, FDSelect, FontDict, IndexedStrings
from fontTools.ttLib.tables.DefaultTable import DefaultTable
from fontTools.ttLib.tables._c_m_a_p import CmapSubtable

full, glyph_list, out, bitset = sys.argv[1:5]
font = TTFont(full)
keep = {int(line) for line in open(glyph_list) if line.strip()} | {0}
order = font.getGlyphOrder()
glyph_set = font.getGlyphSet()
cff = font['CFF '].cff
top = cff[cff.fontNames[0]]
strings = top.CharStrings
private = top.Private
# Glyphs that latex-rust can reach: cmap targets, closed over MATH size
# variants and assembly parts and over ssty script alternates. The rest are
# never measured, so they are left empty.
gid_of = {name: gid for gid, name in enumerate(order)}
reachable = {gid_of[name] for table in font['cmap'].tables for name in table.cmap.values()}
edges = {}
variants = font['MATH'].table.MathVariants
for coverage, constructions in ((variants.VertGlyphCoverage, variants.VertGlyphConstruction),
                                (variants.HorizGlyphCoverage, variants.HorizGlyphConstruction)):
    for name, construction in zip(coverage.glyphs, constructions):
        targets = [record.VariantGlyph for record in construction.MathGlyphVariantRecord]
        if construction.GlyphAssembly:
            targets += [part.glyph for part in construction.GlyphAssembly.PartRecords]
        edges.setdefault(gid_of[name], set()).update(gid_of[t] for t in targets)
gsub = font['GSUB'].table
for record in gsub.FeatureList.FeatureRecord:
    if record.FeatureTag == 'ssty':
        for index in record.Feature.LookupListIndex:
            for subtable in gsub.LookupList.Lookup[index].SubTable:
                for name, alternates in getattr(subtable, 'alternates', {}).items():
                    edges.setdefault(gid_of[name], set()).update(gid_of[t] for t in alternates)
frontier = list(reachable)
while frontier:
    for target in edges.get(frontier.pop(), ()):
        if target not in reachable:
            reachable.add(target)
            frontier.append(target)

for gid, name in enumerate(order):
    if gid in keep:
        continue
    if gid not in reachable:
        strings[name] = T2CharString(program=['endchar'], private=private, globalSubrs=cff.GlobalSubrs)
        continue
    pen = ControlBoundsPen(glyph_set)
    glyph_set[name].draw(pen)
    if pen.bounds is None:
        program = ['endchar']
    else:
        x0, y0, x1, y1 = pen.bounds
        program = [x0, y0, 'rmoveto', x1 - x0, y1 - y0, 'rlineto', 'endchar']
    strings[name] = T2CharString(program=program, private=private, globalSubrs=cff.GlobalSubrs)
# Hints do not move outline points, and ttf-parser ignores them. The subsetter
# drops hints and unused subroutines; only its CFF table is kept, so the other
# tables stay byte-identical. (CFF2 would also drop names, but ttf-parser does
# not close the last contour of a CFF2 glyph, which would change SVG paths.)
buffer = io.BytesIO()
font.save(buffer)
stripped = TTFont(io.BytesIO(buffer.getvalue()))
options = subset.Options(retain_gids=True, hinting=False, notdef_outline=True, glyph_names=True,
                         layout_features=['*'], name_IDs=['*'], drop_tables=[], layout_closure=False)
subsetter = subset.Subsetter(options)
subsetter.populate(gids=range(len(order)))
subsetter.subset(stripped)
stripped['CFF '].cff.remove_unused_subroutines()
# CID-keyed CFF stores no glyph names. Its charset is one range of CIDs.
top = stripped['CFF '].cff.topDictIndex[0]
cids = ['.notdef'] + ['cid%05d' % gid for gid in range(1, len(order))]
fd = FontDict()
fd.setCFF2(False)
fd.Private = top.Private
fd.FontName = 'MarkitMathCore'
top.FDArray = FDArrayIndex()
top.FDArray.append(fd)
top.FDSelect = FDSelect()
top.FDSelect.format = 3
top.FDSelect.gidArray = [0] * len(order)
top.ROS = ('Adobe', 'Identity', 0)
top.CIDCount = len(order)
for key in ('Private', 'Encoding', 'Notice', 'Copyright', 'FullName', 'FamilyName', 'Weight'):
    top.rawDict.pop(key, None)
    if hasattr(top, key):
        delattr(top, key)
strings = top.CharStrings
index = {cids[gid]: strings.charStrings[name] for gid, name in enumerate(order)}
strings.charStrings = index
top.charset = cids
stripped.setGlyphOrder(cids)
stripped['CFF '].cff.strings = IndexedStrings()
cff_bytes = stripped['CFF '].compile(stripped)

font = TTFont(full)
# Tables latex-rust never reads. Without a name table the cut also carries no
# Reserved Font Name (OFL 1.1); the license ships in licenses/.
for tag in ('GPOS', 'GDEF', 'DSIG', 'post', 'name'):
    del font[tag]
# One full Unicode subtable maps what the five original subtables map. An empty
# format 4 subtable ahead of it keeps the one difference: its terminating
# segment maps U+FFFF to glyph 0, as the original BMP subtables do.
cmap = font['cmap']
terminator = CmapSubtable.newSubtable(4)
terminator.platformID, terminator.platEncID, terminator.language, terminator.cmap = 3, 1, 0, {}
cmap.tables = [terminator] + [table for table in cmap.tables if (table.platformID, table.platEncID, table.format) == (3, 10, 12)]
raw = DefaultTable('CFF ')
raw.data = cff_bytes
font['CFF '] = raw
font.recalcTimestamp = False
font.save(out)
# One bit per glyph id, set when the core font keeps the glyph's outline.
bits = bytearray((len(order) + 7) // 8)
for gid in keep:
    bits[gid // 8] |= 1 << (gid % 8)
open(bitset, 'wb').write(bytes(bits))
