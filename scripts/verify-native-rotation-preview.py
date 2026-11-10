"""Verify rotation preview pixels, target geometry and the saved PDF independently."""
import json,os,runpy
from pathlib import Path
from PIL import Image,ImageChops
import fitz
root=Path(os.environ.get('FOLIO_COMMENT_PAGE_ROOT','artifacts/native-rotation-desktop'))
os.environ['FOLIO_COMMENT_PAGE_ROOT']=str(root)
runpy.run_path('scripts/verify-comment-page-controls.py',run_name='__main__')
result=json.loads((root/'results.json').read_text(encoding='utf-8'))
for kind in ['raster','canvas']:
 a=Image.open(root/f'before-save-{kind}.png').convert('RGB');b=Image.open(root/f'after-reopen-{kind}.png').convert('RGB')
 assert a.size==b.size,(kind,a.size,b.size)
 difference=ImageChops.difference(a,b)
 if difference.getbbox():difference.save(root/f'{kind}-difference.png')
 assert difference.getbbox() is None,(kind,'preview differs from saved PDF')
with fitz.open(root/'Folio edited.pdf') as document:
 page=document[1];note=next(a for a in page.annots() if a.info['content']=='Review this paragraph');rect=note.rect*page.rotation_matrix
 for key,expected in [('x',rect.x0),('y',rect.y0),('width',rect.width),('height',rect.height)]:
  assert abs(result['previewHit'][key]-expected)<.02,(key,result['previewHit'],rect)
report={'passed':True,'native_raster_pixels_identical':True,'displayed_canvas_pixels_identical':True,'NoRotate_hit_matches_independent_saved_icon':True}
(root/'rotation-verification.json').write_text(json.dumps(report,indent=2),encoding='utf-8');print(json.dumps(report))
