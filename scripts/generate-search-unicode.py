"""Generate shared document-search data from pinned Unicode17 original files.

No downloads occur here. --check verifies exact reproducibility without writes.
"""
from pathlib import Path
import argparse,hashlib,json

ROOT=Path(__file__).resolve().parents[1]
SOURCES={
 'CaseFolding-17.0.0.txt':('https://www.unicode.org/Public/17.0.0/ucd/CaseFolding.txt','ff8d8fefbf123574205085d6714c36149eb946d717a0c585c27f0f4ef58c4183'),
 'UnicodeData-17.0.0.txt':('https://www.unicode.org/Public/17.0.0/ucd/UnicodeData.txt','2e1efc1dcb59c575eedf5ccae60f95229f706ee6d031835247d843c11d96470c'),
 'LICENSE_UNICODE.txt':('https://www.unicode.org/license.txt','e7a93b009565cfce55919a381437ac4db883e9da2126fa28b91d12732bc53d96')}

def generate(inputs):
    raw={name:(inputs/name).read_bytes()for name in SOURCES}
    for name,(_,expected)in SOURCES.items():
        if hashlib.sha256(raw[name]).hexdigest()!=expected:raise ValueError('Pinned Unicode source changed: '+name)
    folds={}
    for line in raw['CaseFolding-17.0.0.txt'].decode('utf-8').splitlines():
        value=line.split('#',1)[0].strip()
        if not value:continue
        fields=[part.strip()for part in value.split(';')]
        if fields[1]not in {'C','F'}:continue
        cp=int(fields[0],16);mapping=''.join(chr(int(part,16))for part in fields[2].split())
        if cp in folds or not mapping:raise ValueError('Invalid full-fold entry')
        folds[cp]=mapping
    ranges=[];pending=None
    for line in raw['UnicodeData-17.0.0.txt'].decode('utf-8').splitlines():
        fields=line.split(';');cp=int(fields[0],16);name=fields[1];category=fields[2]
        if name.endswith(', First>'):
            assert pending is None;pending=(cp,category);continue
        start=cp
        if name.endswith(', Last>'):
            assert pending is not None and pending[1]==category and pending[0]<=cp
            start=pending[0];pending=None
        else:assert pending is None
        if category[0]in {'L','M','N'}or category=='Pc':ranges.append([start,cp])
    assert pending is None
    ranges.append([0x200c,0x200d]);merged=[]
    for start,end in sorted(ranges):
        if merged and start<=merged[-1][1]+1:merged[-1][1]=max(end,merged[-1][1])
        else:merged.append([start,end])
    data=dict(unicodeVersion='17.0.0',policy='default-full-C+F; words=L/M/N/Pc+ZWNJ/ZWJ; contextual apostrophes',sources={name:dict(url=url,sha256=digest)for name,(url,digest)in SOURCES.items()},folds=sorted(folds.items()),wordRanges=merged)
    return (json.dumps(data,ensure_ascii=True,separators=(',',':'))+'\n').encode(),raw['LICENSE_UNICODE.txt']

def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--inputs',type=Path,default=ROOT/'tests/fixtures/search/unicode')
    parser.add_argument('--output',type=Path,default=ROOT/'src/shared/search-unicode.json')
    parser.add_argument('--license-output',type=Path,default=ROOT/'src/shared/UNICODE-LICENSE.txt')
    parser.add_argument('--check',action='store_true');args=parser.parse_args()
    generated,license=generate(args.inputs)
    outputs=[(args.output,generated),(args.license_output,license)]
    if args.check:
        for path,expected in outputs:
            if path.read_bytes()!=expected:raise ValueError('Generated search data differs: '+str(path))
    else:
        if any(path.exists()for path,_ in outputs):raise ValueError('Preserve existing outputs; use --check to verify')
        for path,data in outputs:
            path.parent.mkdir(parents=True,exist_ok=True)
            with path.open('xb')as out:out.write(data)
    print(json.dumps(dict(passed=True,checked=args.check,unicodeVersion='17.0.0',dataSha256=hashlib.sha256(generated).hexdigest(),bytes=len(generated))))

if __name__=='__main__':main()
