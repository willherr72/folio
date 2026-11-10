from pathlib import Path
import hashlib,json
import fitz
from pypdf import PdfReader

root=Path(__file__).resolve().parent.parent
fixtures=root/'tests/fixtures/protection'
output=root/'artifacts/protection-codec-output'
manifest=json.loads((fixtures/'manifest.json').read_text(encoding='utf-8'))
for entry in manifest['fixtures']:
    assert hashlib.sha256((fixtures/entry['file']).read_bytes()).hexdigest()==entry['sha256']
results=[]
for name in ['rc4_40','rc4_128','aes_128','aes_256_r5','aes_256','object_streams','metadata_clear','stream_dictionary']:
    path=output/(name+'.pdf')
    source=fixtures/((name if name not in ['metadata_clear','stream_dictionary'] else 'plain')+'.pdf')
    original=PdfReader(source)
    if original.is_encrypted:assert original.decrypt('reader')
    result=PdfReader(path);assert result.is_encrypted
    assert not result.decrypt('wrong')
    assert result.decrypt('reader')
    assert len(result.pages)==len(original.pages)==1
    if name in ['rc4_40','rc4_128','aes_128','aes_256_r5','aes_256']:
        for key in ['/R','/V','/P']:
            assert result.trailer['/Encrypt'][key]==original.trailer['/Encrypt'][key],(name,key)
        assert result.trailer['/ID'][0].original_bytes==original.trailer['/ID'][0].original_bytes
        assert result.metadata.title=='Edited protected copy'
    if name=='metadata_clear':
        assert result.trailer['/Encrypt']['/EncryptMetadata'].value is False
        assert b'private-metadata-dictionary-value' not in path.read_bytes()
        metadata=result.trailer['/Root']['/Metadata']
        assert metadata['/FolioSecretLabel']=='private-metadata-dictionary-value'
        # pypdf6.14 decrypts every stream body, including Identity overrides.
        # Check its dictionary decoding; MuPDF independently checks metadata bytes below.
        assert metadata['/Filter']=='/Crypt'
        assert metadata['/DecodeParms']['/Name']=='/Identity'
    if name=='stream_dictionary':
        assert b'private-stream-dictionary-value' not in path.read_bytes()
        streams=result.pages[0]['/Contents']
        assert any(s.get_object().get('/FolioSecretLabel')=='private-stream-dictionary-value' for s in streams)
    for password in ['reader','owner']:
        r=PdfReader(path);assert r.decrypt(password)
        assert 'Protected Folio fixture' in r.pages[0].extract_text()
        with fitz.open(source) as before,fitz.open(path) as after:
            if before.is_encrypted:assert before.authenticate('reader')
            assert after.authenticate(password)
            if name=='metadata_clear':
                _,reference=after.xref_get_key(after.pdf_catalog(),'Metadata')
                xref=int(reference.split()[0])
                assert after.xref_get_key(xref,'FolioSecretLabel')==('string','private-metadata-dictionary-value')
                assert b'deliberately clear metadata' in after.xref_stream(xref)
            assert 'Protected Folio fixture' in after[0].get_text()
            a=before[0].get_pixmap(alpha=False);b=after[0].get_pixmap(alpha=False)
            assert (a.width,a.height)==(b.width,b.height)
            assert a.samples==b.samples,(name,password,'raster changed')
    results.append({'file':path.name,'bothPasswordsWork':True,'wrongPasswordRejected':True,'pixelsIdentical':True,'sha256':hashlib.sha256(path.read_bytes()).hexdigest()})
(root/'artifacts/protection-independent-verification.json').write_text(json.dumps(results,indent=2))
print(json.dumps(results,indent=2))
