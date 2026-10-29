#!/usr/bin/env python3
"""Builds the math font files that the web package ships.

1. A core cut of STIX Two Math, embedded in the Wasm. Glyph ids, advances,
   cmap, MATH constants, variants, kerning, and ssty alternates are identical
   to the full font. Glyphs not listed in math-core-glyphs.txt keep their exact
   bounding box but lose their outline (one line between the box corners), so
   layout, which reads only metrics and bounding boxes, is always identical.
   Glyphs latex-rust cannot reach are empty. Hints, subroutines, glyph names,
   GSUB features other than ssty, left side bearings, and tables latex-rust
   never reads are dropped.
2. Glyph chunks: the outlines left out of the core, grouped by alphabet or
   symbol family (`math-glyphs-N.bin`). Before drawing a dropped glyph the Wasm
   reports its group; the worker fetches that chunk and merges it into the face.
3. A group map, one byte per glyph id: 0 when the core draws the glyph,
   otherwise the chunk that holds its outline.

The CFF table is written by hand with its CharStrings INDEX last, and it is
the last table in the file, so merging a chunk only rewrites that INDEX.
Output is deterministic.

usage: scripts/math-font-web.py FULL.otf GLYPHS.txt CORE.otf GROUPS.bin CHUNK_DIR   (needs fonttools)
"""
import io
import os
import struct
import sys
from fontTools.ttLib import TTFont
from fontTools.pens.boundsPen import ControlBoundsPen
from fontTools.misc.psCharStrings import T2CharString
from fontTools import subset
from fontTools.ttLib.tables._c_m_a_p import CmapSubtable

full, glyph_list, out, groups_out, chunk_dir = sys.argv[1:6]
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


# Unreachable glyphs are never measured or drawn: empty them, then let the
# subsetter drop hints and inline subroutines (desubroutinized charstrings
# compress better and stand alone, which chunks need).
for gid, name in enumerate(order):
    if gid not in reachable and gid != 0:
        strings[name] = T2CharString(program=['endchar'], private=private, globalSubrs=cff.GlobalSubrs)
buffer = io.BytesIO()
font.save(buffer)
stripped = TTFont(io.BytesIO(buffer.getvalue()))
options = subset.Options(retain_gids=True, hinting=False, notdef_outline=True, glyph_names=True,
                         layout_features=['*'], name_IDs=['*'], drop_tables=[], layout_closure=False,
                         desubroutinize=True)
subsetter = subset.Subsetter(options)
subsetter.populate(gids=range(len(order)))
subsetter.subset(stripped)
top = stripped['CFF '].cff.topDictIndex[0]
cs = top.CharStrings
priv = top.Private


def bytecode(program):
    charstring = T2CharString(program=program, private=priv, globalSubrs=stripped['CFF '].cff.GlobalSubrs)
    charstring.compile()
    return charstring.bytecode


full_code = []
for gid, name in enumerate(order):
    charstring = cs[name]
    charstring.compile()
    full_code.append(charstring.bytecode)

# Core charstrings: kept outlines, bounding-box stubs for dropped reachable
# glyphs (ttf-parser computes CFF boxes from every point), empty otherwise.
core_code = []
for gid, name in enumerate(order):
    if gid in keep or gid not in reachable:
        core_code.append(full_code[gid])
        continue
    pen = ControlBoundsPen(glyph_set)
    glyph_set[name].draw(pen)
    if pen.bounds is None:
        core_code.append(bytecode(['endchar']))
    else:
        x0, y0, x1, y1 = pen.bounds
        core_code.append(bytecode([x0, y0, 'rmoveto', x1 - x0, y1 - y0, 'rlineto', 'endchar']))


def index(items, off_size=None):
    if not items:
        return b'\0\0'
    offsets = [1]
    for item in items:
        offsets.append(offsets[-1] + len(item))
    size = off_size or next(n for n in (1, 2, 3, 4) if offsets[-1] < 1 << (8 * n))
    out = struct.pack('>HB', len(items), size)
    for offset in offsets:
        out += offset.to_bytes(size, 'big')
    return out + b''.join(items)


def number(value):
    # Fixed-width integers keep the Top DICT's size independent of offsets.
    return b'\x1d' + struct.pack('>i', value)


def real(value):
    text = repr(float(value)).replace('e-', 'c').replace('e', 'b').replace('E', 'b')
    nibbles = []
    for char in text:
        nibbles.append({'.': 0xa, 'b': 0xb, 'c': 0xc, '-': 0xe}.get(char, None) if not char.isdigit() else int(char))
    nibbles.append(0xf)
    if len(nibbles) % 2:
        nibbles.append(0xf)
    return b'\x1e' + bytes(nibbles[i] << 4 | nibbles[i + 1] for i in range(0, len(nibbles), 2))


def cff_table(codes):
    standard = 391
    strings = [b'Adobe', b'Identity', b'MarkitMath']
    count = len(codes)
    private_dict = b''
    for op, key in ((20, 'defaultWidthX'), (21, 'nominalWidthX')):
        value = getattr(priv, key, 0)
        private_dict += (number(int(value)) if float(value).is_integer() else real(value)) + bytes([op])
    header = bytes([1, 0, 4, 4])
    names = index([b'MarkitMath'])

    def top_dict(charset, fdselect, fdarray, charstrings):
        return (number(standard) + number(standard + 1) + number(0) + b'\x0c\x1e'  # ROS
                + number(count) + b'\x0c\x22'  # CIDCount
                + number(charset) + b'\x0f'
                + number(fdselect) + b'\x0c\x25'
                + number(fdarray) + b'\x0c\x24'
                + number(charstrings) + b'\x11')
    size = len(top_dict(0, 0, 0, 0))
    start = len(header) + len(names) + len(index([b'\0' * size])) + len(index(strings)) + len(index([]))
    charset = bytes([2]) + struct.pack('>HH', 1, count - 2)  # gid n -> CID n
    fdselect = bytes([3]) + struct.pack('>HHBH', 1, 0, 0, count)
    charset_at = start
    fdselect_at = charset_at + len(charset)
    fdarray_at = fdselect_at + len(fdselect)
    font_dict_size = len(number(0) + b'\x0c\x26' + number(0) + number(0) + b'\x12')
    private_at = fdarray_at + len(index([b'\0' * font_dict_size]))
    font_dict = number(standard + 2) + b'\x0c\x26' + number(len(private_dict)) + number(private_at) + b'\x12'
    charstrings_at = private_at + len(private_dict)
    out = (header + names + index([top_dict(charset_at, fdselect_at, fdarray_at, charstrings_at)])
           + index(strings) + index([]) + charset + fdselect + index([font_dict]) + private_dict)
    assert len(out) == charstrings_at
    return out + index(codes)


# Group the dropped outlines by where they come from: each glyph joins the
# group of the first code point (in code point order) that reaches it; glyphs
# that only size variants or script alternates of core glyphs reach form one
# group of their own.
GROUPS = [
    ('fraktur', [(0x1D504, 0x1D537), (0x1D56C, 0x1D59F), (0x212D, 0x212D), (0x210C, 0x210C), (0x2111, 0x2111), (0x211C, 0x211C), (0x2128, 0x2128)]),
    ('script', [(0x1D49C, 0x1D503), (0x212C, 0x212C), (0x2130, 0x2131), (0x210A, 0x210B), (0x2110, 0x2110), (0x2112, 0x2113), (0x2118, 0x2118), (0x211B, 0x211B), (0x212F, 0x212F), (0x2133, 0x2134)]),
    ('double-struck', [(0x1D538, 0x1D56B), (0x1D7D8, 0x1D7E1), (0x2102, 0x2102), (0x210D, 0x210D), (0x2115, 0x2115), (0x2119, 0x211A), (0x211D, 0x211D), (0x2124, 0x2124), (0x213C, 0x2149)]),
    ('sans-mono', [(0x1D5A0, 0x1D6A3), (0x1D7E2, 0x1D7FF)]),
    ('math-alphanumeric', [(0x1D400, 0x1D7FF)]),
    ('latin-1', [(0x0000, 0x00FF)]),
    ('latin-extended', [(0x0100, 0x024F), (0x1E00, 0x1EFF)]),
    ('greek-cyrillic', [(0x0370, 0x052F), (0x1F00, 0x1FFF)]),
    ('phonetic-diacritics', [(0x0250, 0x036F), (0x1D00, 0x1DFF)]),
    ('arrows', [(0x2190, 0x21FF), (0x27F0, 0x27FF), (0x2900, 0x297F), (0x2B00, 0x2BFF)]),
    ('operators', [(0x2200, 0x22FF), (0x2A00, 0x2AFF)]),
    ('technical', [(0x2300, 0x23FF)]),
    ('shapes', [(0x2500, 0x25FF)]),
    ('symbols-dingbats', [(0x2600, 0x27BF)]),
    ('math-misc', [(0x27C0, 0x27EF), (0x2980, 0x29FF)]),
    ('other', [(0x0000, 0x10FFFF)]),
]
VARIANTS = len(GROUPS) + 1


def group_of(codepoint):
    for number_, (_, ranges) in enumerate(GROUPS, 1):
        if any(lo <= codepoint <= hi for lo, hi in ranges):
            return number_


origin = {}
for codepoint, name in sorted(font['cmap'].getBestCmap().items()):
    root = gid_of[name]
    seen = {root}
    frontier = [root]
    group = group_of(codepoint) if root not in keep else VARIANTS
    while frontier:
        gid = frontier.pop()
        origin.setdefault(gid, group if gid != root else group_of(codepoint))
        for target in edges.get(gid, ()):
            if target not in seen:
                seen.add(target)
                frontier.append(target)
groups = bytearray(len(order))
chunks = {}
for gid in range(len(order)):
    if gid in keep or gid not in reachable:
        continue
    group = origin.get(gid, VARIANTS)
    groups[gid] = group
    chunks.setdefault(group, []).append(gid)

# Tables latex-rust never reads go. Without a name table the cut also carries
# no Reserved Font Name (OFL 1.1); the license ships in licenses/.
font = TTFont(full, recalcBBoxes=False)
for tag in ('GPOS', 'GDEF', 'DSIG', 'post', 'name'):
    del font[tag]
# latex-rust reads only the ssty feature of GSUB and never a left side bearing.
gsub = font['GSUB'].table
ssty = [record for record in gsub.FeatureList.FeatureRecord if record.FeatureTag == 'ssty'][:1]
used = sorted({index_ for record in ssty for index_ in record.Feature.LookupListIndex})
gsub.LookupList.Lookup = [gsub.LookupList.Lookup[index_] for index_ in used]
gsub.LookupList.LookupCount = len(used)
for record in ssty:
    record.Feature.LookupListIndex = [used.index(index_) for index_ in record.Feature.LookupListIndex]
    record.Feature.LookupCount = len(record.Feature.LookupListIndex)
gsub.FeatureList.FeatureRecord = ssty
gsub.FeatureList.FeatureCount = len(ssty)
for script in gsub.ScriptList.ScriptRecord:
    for lang in [script.Script.DefaultLangSys] + [r.LangSys for r in script.Script.LangSysRecord]:
        if lang is not None:
            lang.ReqFeatureIndex = 0xFFFF
            lang.FeatureIndex = list(range(len(ssty)))
            lang.FeatureCount = len(ssty)
metrics = font['hmtx'].metrics
for name in metrics:
    metrics[name] = (metrics[name][0], 0)
# One full Unicode subtable maps what the five original subtables map. An empty
# format 4 subtable ahead of it keeps the one difference: its terminating
# segment maps U+FFFF to glyph 0, as the original BMP subtables do.
cmap = font['cmap']
terminator = CmapSubtable.newSubtable(4)
terminator.platformID, terminator.platEncID, terminator.language, terminator.cmap = 3, 1, 0, {}
cmap.tables = [terminator] + [table for table in cmap.tables if (table.platformID, table.platEncID, table.format) == (3, 10, 12)]
tables = {tag: font.getTableData(tag) for tag in ('head', 'hhea', 'maxp', 'OS/2', 'cmap', 'GSUB', 'MATH', 'hmtx')}
tables['CFF '] = cff_table(core_code)


def checksum(data):
    data += b'\0' * (-len(data) % 4)
    return sum(struct.unpack('>%dI' % (len(data) // 4), data)) & 0xFFFFFFFF


def sfnt(tables):
    tags = sorted(tables)
    order_ = [tag for tag in ('head', 'hhea', 'maxp', 'OS/2', 'cmap', 'GSUB', 'MATH', 'hmtx') if tag in tables] + ['CFF ']  # CFF data last
    count = len(tags)
    power = 1 << (count.bit_length() - 1)
    header = struct.pack('>4sHHHH', b'OTTO', count, power * 16, power.bit_length() - 1, count * 16 - power * 16)
    offset = 12 + 16 * count
    placed = {}
    body = b''
    for tag in order_:
        data = tables[tag]
        placed[tag] = (offset + len(body), len(data))
        body += data + b'\0' * (-len(data) % 4)
    directory = b''.join(struct.pack('>4sIII', tag.encode(), checksum(tables[tag]), *placed[tag]) for tag in tags)
    return header + directory + body


tables['head'] = tables['head'][:8] + b'\0\0\0\0' + tables['head'][12:]
blob = sfnt(tables)
adjust = (0xB1B0AFBA - checksum(blob)) & 0xFFFFFFFF
head_at = struct.unpack('>I', blob[12 + 16 * sorted(tables).index('head') + 8:][:4])[0]
blob = blob[:head_at + 8] + struct.pack('>I', adjust) + blob[head_at + 12:]
open(out, 'wb').write(blob)
open(groups_out, 'wb').write(bytes(groups))
os.makedirs(chunk_dir, exist_ok=True)
for name in os.listdir(chunk_dir):
    if name.startswith('math-glyphs-'):
        os.remove(os.path.join(chunk_dir, name))
for group, gids in sorted(chunks.items()):
    # MKGC v1: magic, version, glyph count, then per glyph: u16 gid, u32 length, charstring.
    data = b'MKGC' + struct.pack('>II', 1, len(gids))
    for gid in gids:
        data += struct.pack('>HI', gid, len(full_code[gid])) + full_code[gid]
    open(os.path.join(chunk_dir, 'math-glyphs-%d.bin' % group), 'wb').write(data)
    label = GROUPS[group - 1][0] if group <= len(GROUPS) else 'variants'
    print('group %2d %-22s %4d glyphs %7d bytes' % (group, label, len(gids), len(data)), file=sys.stderr)
