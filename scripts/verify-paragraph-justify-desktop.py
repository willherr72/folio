import hashlib,json,os
from pathlib import Path
import fitz
from pypdf import PdfReader
from PIL import Image,ImageChops,ImageDraw
from fontTools.ttLib import TTFont
from fontTools.pens.boundsPen import BoundsPen
root=Path(os.environ.get('FOLIO_PARAGRAPH_JUSTIFY_ROOT','artifacts/paragraph-justify-acceptance'))
result=json.loads((root/'results.json').read_text(encoding='utf-8'));assert result['passed']
sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
assert sha(root/'Original paragraph.pdf')==result['sourceHash'];assert sha(root/'Substitute.ttf')==result['fontHash']
def spans(page):return [s for b in page.get_text('dict')['blocks'] for line in b.get('lines',[]) for s in line['spans']]
def notes(filename):return [(a.get_object().get('/Rect'),a.get_object().get('/F'),a.get_object().get('/Contents')) for a in PdfReader(root/filename).pages[0].get('/Annots',[]) if a.get_object().get('/Subtype')=='/Text']
def rect(box):return fitz.Rect(box['x'],box['y'],box['x']+box['width'],box['y']+box['height'])
def image(page):
 p=page.get_pixmap(matrix=fitz.Matrix(2,2),colorspace=fitz.csRGB,alpha=False);return Image.frombytes('RGB',(p.width,p.height),p.samples)
def outside_equal(first,second,rects):
 a,b=image(first),image(second);assert a.size==b.size;diff=ImageChops.difference(a,b);assert diff.getbbox();draw=ImageDraw.Draw(diff)
 for r in rects:draw.rectangle((int(r.x0*2)-3,int(r.y0*2)-3,int(r.x1*2)+3,int(r.y1*2)+3),fill=(0,0,0))
 assert diff.getbbox() is None,diff.getbbox()
before=fitz.open(root/'Before justification.pdf');first=fitz.open(root/'Justified paragraph.pdf');second=fitz.open(root/'Substituted justified paragraph.pdf');original=next(s for s in spans(before[0]) if s['text']=='Original words.');neighbor=next(s for s in spans(before[0]) if s['text']=='Neighbor stays.')
geometry=[]
font=TTFont(root/'Substitute.ttf');glyphset=font.getGlyphSet();cmap=font.getBestCmap();units=font['head'].unitsPerEm
def metrics(c,size):
 if size==12:return (7.2,0.,7.2)
 name=cmap[ord(c)];pen=BoundsPen(glyphset);glyphset[name].draw(pen)
 bounds=pen.bounds or (0,0,0,0);scale=size/units
 return (font['hmtx'][name][0]*scale,bounds[0]*scale,bounds[2]*scale)
for name,doc,size,expected in [('Justified paragraph.pdf',first,12,result['firstText']),('Substituted justified paragraph.pdf',second,14,result['secondText'])]:
 assert len(doc)==1 and doc[0].rotation==90
 reader=PdfReader(root/name);plain=reader.pages[0].extract_text();assert 'Original words.' not in plain and 'Neighbor stays.' in plain
 # Both readers must preserve word order and Unicode, independent of line breaks.
 for content in [plain,doc[0].get_text()]:
  stripped=content.replace('Neighbor stays.','');assert ' '.join(stripped.split())==expected,(name,stripped)
 changed=[s for s in spans(doc[0]) if s['text']!='Neighbor stays.'];assert len(changed)>=2
 assert all(abs(s['size']-size)<0.01 and s['color']==original['color'] for s in changed)
 other=next(s for s in spans(doc[0]) if s['text']=='Neighbor stays.')
 for key in ['origin','bbox','size','font','color']:assert other[key]==neighbor[key]
 assert notes(name)==notes('Before justification.pdf') and len(notes(name))==1
 # Extract glyph origins and widths; trailing spaces are retained in PDF text but
 # do not count toward the justified right edge. MuPDF span bounds include them.
 rows=[]
 for b in doc[0].get_text('rawdict')['blocks']:
  for line in b.get('lines',[]):
   chars=[c for s in line['spans'] for c in s['chars']]
   if ''.join(c['c'] for c in chars)=='Neighbor stays.':continue
   # MuPDF can split a justified row at a large gap. Reassemble geometric rows,
   # preserving the actual glyph origins and encoded spaces from each segment.
   if chars:
    existing=next((row for row in rows if abs(row[0]['origin'][1]-chars[0]['origin'][1])<0.02),None)
    if existing is None:rows.append(chars)
    else:existing.extend(chars)
 rows.sort(key=lambda row:row[0]['origin'][1])
 for row in rows:
  row.sort(key=lambda c:c['origin'][0])
  while row and row[-1]['c']==' ':row.pop()
 # Advance boxes do not include negative bearings. Use the independent fontTools
 # outlines and advances to measure the substituted font's complete extent.
 lefts=[min(min(c['origin'][0],c['origin'][0]+metrics(c['c'],size)[1]) for c in row) for row in rows]
 rights=[max(c['origin'][0]+max(metrics(c['c'],size)[0],metrics(c['c'],size)[2]) for c in row) for row in rows]
 baselines=[row[0]['origin'][1] for row in rows]
 assert all(abs(x-60)<0.6 for x in lefts),(name,lefts)
 assert all(abs(x-240)<0.6 for x in rights[:-1]),(name,rights)
 assert rights[-1]<239,(name,rights)
 assert all(abs(b-a-size*1.5)<0.02 for a,b in zip(baselines,baselines[1:])),baselines
 expansions=[]
 for index,row in enumerate(rows):
  visible=[i for i,c in enumerate(row) if c['c']!=' '];gaps=[]
  for a,b in zip(visible,visible[1:]):
   residual=row[b]['origin'][0]-row[a]['origin'][0]-sum(metrics(c['c'],size)[0] for c in row[a:b])
   if b>a+1:gaps.append(residual)
   else:assert abs(residual)<0.04,(name,index,'within-word',residual)
  if index<len(rows)-1:assert gaps and min(gaps)>0.05 and max(gaps)-min(gaps)<0.04,(name,index,gaps)
  else:assert all(abs(g)<0.04 for g in gaps),(name,index,gaps)
  expansions.append(gaps)
 if size==14:
  programs=[]
  for font in reader.pages[0]['/Resources']['/Font'].values():
   for descendant in font.get_object().get('/DescendantFonts',[]):
    descriptor=descendant.get_object().get('/FontDescriptor')
    if descriptor and '/FontFile2' in descriptor.get_object():programs.append(hashlib.sha256(descriptor.get_object()['/FontFile2'].get_data()).hexdigest())
  assert result['fontHash'] in programs
 else:assert all(s['font']==original['font'] for s in changed)
 geometry.append({'file':name,'lefts':lefts,'rights':rights,'baselineGaps':[b-a for a,b in zip(baselines,baselines[1:])],'wordGapExpansions':expansions,'size':size,'lastLineNatural':True})
before[0].set_rotation(90);outside_equal(before[0],first[0],[rect(result[key])*before[0].rotation_matrix for key in ['old','new']]);outside_equal(first[0],second[0],[rect(result['second'])])
report={'passed':True,'readers':['MuPDF','pypdf'],'geometry':geometry,'nativeNoteAndNeighborPreserved':True,'outsideEditedAreasPixelsUnchanged':True,'sourceAndFontUnchanged':True}
(root/'independent-results.json').write_text(json.dumps(report,indent=2));print(json.dumps(report,indent=2))
