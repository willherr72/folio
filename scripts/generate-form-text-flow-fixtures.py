"""Reproducible ReportLab text-object/form compatibility fixtures, no private inputs."""
from pathlib import Path
import hashlib,json
import reportlab
from reportlab.pdfgen import canvas

root=Path('tests/fixtures/form-text-flow');root.mkdir(parents=True,exist_ok=True)
for mode in ['draw-string','text-lines','explicit-lines']:
    for selected in [None,0,1]:
        suffix='original' if selected is None else f'line-{selected}-reference'
        c=canvas.Canvas(str(root/f'{mode}-{suffix}.pdf'),pagesize=(600,600),invariant=1,pageCompression=0)
        for name,changed in [('Shared',False),('First',selected is not None)]:
            lines=['First sentence','Second sentence']
            if changed:lines[selected]=['First longer sentence','Second longer sentence'][selected]
            c.beginForm(name,0,0,260,120)
            if mode=='draw-string':
                c.setFont('Helvetica',16)
                c.drawString(10,80,lines[0]);c.drawString(10,45,lines[1])
            elif mode=='text-lines':
                t=c.beginText(10,80);t.setFont('Helvetica',16);t.setLeading(35)
                for line in lines:t.textLine(line)
                c.drawText(t)
            else:
                c._code.append(f'BT /F1 16 Tf 1 0 0 1 10 80 Tm ({lines[0]}) Tj 1 0 0 1 10 45 Tm ({lines[1]}) Tj ET')
            c.endForm()
        for x,name in [(20,'First'),(320,'Shared')]:
            c.saveState();c.translate(x,300);c.doForm(name);c.restoreState()
        c.showPage();c.doForm('Shared');c.save()
manifest={'generator':'scripts/generate-form-text-flow-fixtures.py','reportlab':reportlab.Version,'source':'Locally generated original text and geometry; no third-party documents.','files':{p.name:hashlib.sha256(p.read_bytes()).hexdigest() for p in sorted(root.glob('*.pdf'))}}
(root/'manifest.json').write_text(json.dumps(manifest,indent=2)+'\n')
print(json.dumps(manifest))
