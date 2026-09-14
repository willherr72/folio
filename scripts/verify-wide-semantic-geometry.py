"""Model the pinned PDFium normal glyph bbox path; never modifies glyphs or PDFs."""
import argparse,ctypes,hashlib,io,json,math,re
from pathlib import Path
from fontTools.ttLib import TTFont
from pypdf import PdfReader
from pypdf.generic import ContentStream,ByteStringObject,TextStringObject
ROOT=Path(__file__).resolve().parents[1]
parser=argparse.ArgumentParser(description=__doc__)
parser.add_argument('--input',type=Path,default=ROOT/'artifacts/issue14-wide/scale16-consistent')
parser.add_argument('--output',type=Path,default=ROOT/'artifacts/issue14-wide/scale16-consistent-model.json')
args=parser.parse_args()
report=json.loads((args.input/'results.json').read_text(encoding='utf-8'))
rows=[]
def f32(v): return ctypes.c_float(v).value
for row in report['cases']:
 path=args.input/row.get('pdf',f"{row['case']}-{row['rotation']}.pdf")
 assert hashlib.sha256(path.read_bytes()).hexdigest()==row['output_sha256'], 'PDF/report hash mismatch'
 reader=PdfReader(path); page=reader.pages[0]
 fonts=[ref.get_object() for ref in page['/Resources']['/Font'].values() if '/ToUnicode' in ref.get_object()]
 assert len(fonts)==1 and fonts[0]['/Subtype']=='/Type0', 'Expected one wide semantic font'
 f=fonts[0]; cid=f['/DescendantFonts'][0].get_object()
 tt=TTFont(io.BytesIO(cid['/FontDescriptor']['/FontFile2'].get_data())); upm=tt['head'].unitsPerEm
 widths=list(map(float,cid['/W'][1])); predicted=[]; visual=[]
 cmap={int(a,16):bytes.fromhex(b).decode('utf-16-be') for body in re.findall(r'beginbfchar(.*?)endbfchar',f['/ToUnicode'].get_data().decode(),re.S) for a,b in re.findall(r'<([0-9A-Fa-f]+)>\s*<([0-9A-Fa-f]+)>',body)}
 for values,op in ContentStream(page.get_contents(),reader).operations:
  if op==b'Tf': size=f32(float(values[1]))
  elif op==b'Tm': origin_x,y=[f32(float(v)) for v in values[-2:]]; x=0.0
  elif op==b'TJ':
   for item in values[0]:
    if isinstance(item,(ByteStringObject,TextStringObject)):
     data=bytes(item) if isinstance(item,ByteStringObject) else item.original_bytes
     for i in range(0,len(data),2):
      code=int.from_bytes(data[i:i+2],'big'); visual.append(cmap[code]); glyph=tt['glyf'][tt.getGlyphName(code)]
      # Normalize with +0.5 then truncation, matching measured negative metrics.
      a,b,c,d=[math.trunc(v*1000/upm+0.5) for v in (glyph.xMin,glyph.xMax,glyph.yMin,glyph.yMax)]
      d+=math.trunc(d/64)
      predicted.append([f32(origin_x+f32(x+f32(f32(a*size)/1000))),f32(origin_x+f32(x+f32(f32(b*size)/1000))),f32(y+f32(f32(c*size)/1000)),f32(y+f32(f32(d*size)/1000))]); x=f32(x+f32(f32(widths[code-1]*size)/1000))
    else: x=f32(x-f32(f32(f32(float(item))*size)/1000))
 # Pure-direction probe only; resolve order from actual CMap content and source.
 if ''.join(visual)!=row['source']:
  assert ''.join(reversed(visual))==row['source'], 'Mixed direction cannot use this pure-direction model'
  predicted.reverse()
 observed=[c['box'] for c in row['characters']['pdfium']]
 assert len(predicted)==len(observed)==len(row['expected'])
 assert ''.join(c['text'] for c in row['characters']['pdfium'])==row['source']
 errors=[max(abs(a-b) for a,b in zip(p,q)) for p,q in zip(predicted,observed)]
 groups={}
 for actual,expected in zip(observed,row['expected']): groups.setdefault(tuple(expected['range_utf8']),[]).append(actual)
 coincidence=max(max(max(abs(a[i]-b[i]) for i in range(4)) for a in boxes for b in boxes) for boxes in groups.values())
 residual=max(max(abs(a[i]-e['cluster_box'][i]) for i in range(3)) for a,e in zip(observed,row['expected']) if e['ink_box'])
 rows.append({'case':row['case'],'rotation':row['rotation'],'model_max_error_points':max(errors),'max_cluster_coincidence_error_points':coincidence,'max_non_top_native_edge_error_points':residual,'predicted':predicted,'observed':observed})
 print(row['case'],row['rotation'],max(errors),coincidence)
args.output.write_text(json.dumps({'sources':[{'url': 'https://pdfium.googlesource.com/pdfium/+/refs/heads/chromium/8044/core/fpdfapi/page/cpdf_textobject.cpp', 'sha256': '08574f792521883975b369696b6cd2f8faafee6f1eead850ae6340775dc3e6fd'}, {'url': 'https://pdfium.googlesource.com/pdfium/+/refs/heads/chromium/8044/core/fxge/cfx_face.cpp', 'sha256': '43d1af62002f3ab42b5283f4856679f8be604e90c67a8b8d2ba38abdf48f5094'}, {'url': 'https://pdfium.googlesource.com/pdfium/+/refs/heads/chromium/8044/core/fxge/fx_font.cpp', 'sha256': '7d7aee65265fbbae85e782ea347adf2515c1035096cd129112d164047f30dce1'}],'position_arithmetic':'CPDF_TextObject::CalcPositionDataInternal float32 cursor starts at zero; add text matrix origin after local character geometry.','formula':'rect top += trunc(rect top/64), after trunc(value*1000/upm+0.5) normalization to 1000 font units; no glyph modification','cases':rows},indent=2),encoding='utf-8')

assert all(r["model_max_error_points"]<=.001 and r["max_cluster_coincidence_error_points"]<=.02 and r["max_non_top_native_edge_error_points"]<=.006 for r in rows), "Source-modeled geometry failed"
