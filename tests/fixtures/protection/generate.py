"""Synthetic PDFs only. Re-generation changes randomized encryption bytes/hashes."""
from pathlib import Path
import hashlib, io, json
import fitz, pypdf
from pypdf import PdfReader, PdfWriter
from pypdf.constants import UserAccessPermissions as P

root=Path(__file__).parent
doc=fitz.open()
page=doc.new_page(width=360,height=480)
page.insert_text((35,60),'Protected Folio fixture',fontsize=16)
page.insert_text((35,95),'Original paragraph. Neighbors stay unchanged.',fontsize=11)
page.draw_rect(fitz.Rect(35,130,250,180),color=(0.1,0.3,0.7),width=2)
doc.set_metadata({'title':'Synthetic protection fixture','author':'Folio tests'})
plain=doc.tobytes();(root/'plain.pdf').write_bytes(plain)
permissions=P(0xFFFFF0C0)|P.PRINT|P.ADD_OR_MODIFY|P.FILL_FORM_FIELDS|P.EXTRACT_TEXT_AND_GRAPHICS
cases=[(a.lower().replace('-','_'),a,'reader','owner') for a in ['RC4-40','RC4-128','AES-128','AES-256-R5','AES-256']]
cases += [('empty_user','AES-256','','owner'),('unicode','AES-256','café-读者','拥有者-é')]
cases += [('edit_only_'+a.lower().replace('-','_'),a,'reader','owner') for a in ['RC4-40','RC4-128','AES-128','AES-256-R5','AES-256']]
records=[]
for name,algorithm,user,owner in cases:
    w=PdfWriter(clone_from=PdfReader(io.BytesIO(plain)))
    rights=(P(0xFFFFF0C0)|P.MODIFY) if name.startswith('edit_only_') else permissions
    w.encrypt(user,owner,algorithm=algorithm,permissions_flag=rights)
    path=root/(name+'.pdf');w.write(path)
    records.append({'file':path.name,'algorithm':algorithm,'user':user,'owner':owner,'sha256':hashlib.sha256(path.read_bytes()).hexdigest()})
compressed=doc.tobytes(use_objstms=1,deflate=True,encryption=fitz.PDF_ENCRYPT_AES_256,permissions=int(permissions)-(1<<32),user_pw='reader',owner_pw='owner')
(root/'object_streams.pdf').write_bytes(compressed)
records.append({'file':'object_streams.pdf','algorithm':'MuPDF AES-256 object streams','user':'reader','owner':'owner','sha256':hashlib.sha256(compressed).hexdigest()})
(root/'manifest.json').write_text(json.dumps({'generator':'generate.py','pypdf':pypdf.__version__,'pymupdf':fitz.VersionBind,'fixtures':records},indent=2,ensure_ascii=False),encoding='utf-8')
print('Generated',len(records),'synthetic protected fixtures')
