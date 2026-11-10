"""Pin legacy finite fallbacks for out-of-range PDF widget border numbers."""
import argparse
import hashlib
import json
import math
from pathlib import Path
import runpy

from pypdf import PdfReader, PdfWriter
from pypdf.generic import ContentStream, FloatObject, NameObject, ArrayObject, NumberObject

HELPERS = runpy.run_path(str(Path(__file__).with_name('probe-pdfium-form-borders.py')))


class RawNumber(FloatObject):
    """Write an exact decimal token without Python float clamping/Infinity names."""
    def __new__(cls, token):
        value = super().__new__(cls, 0)
        value.token = token
        return value

    def write_to_stream(self, stream, encryption_key=None):
        stream.write(self.token.encode('ascii'))


def fixture(base, path, token, storage, style):
    writer = PdfWriter(clone_from=base)
    field = writer.pages[0]['/Annots'][0].get_object()
    field['/BS'][NameObject('/S')] = NameObject('/'+style)
    number = RawNumber(token)
    if storage == 'indirect':
        number = writer._add_object(number)
    if storage == 'border':
        field[NameObject('/Border')] = ArrayObject([NumberObject(0), NumberObject(0), number])
        field['/BS'].pop('/W', None)
    else:
        field['/BS'][NameObject('/W')] = number
    with path.open('xb') as output:
        writer.write(output)
    assert token.encode('ascii') in path.read_bytes(), 'exact numeric token lost'


def appearance(path):
    reader = PdfReader(path)
    appearances = reader.pages[0]['/Annots'][0].get_object()['/AP']
    streams = []
    for mode in sorted(appearances):
        value = appearances[mode]
        if hasattr(value, 'get_data'):
            streams.append(value)
        else:
            streams.extend(value[state].get_object() for state in sorted(value))
    result = []
    for ap in streams:
        raw = ap.get_data()
        for args, op in ContentStream(ap, reader).operations:
            assert op in [b'q', b'Q', b'g', b'G', b'rg', b'RG', b'k', b'K', b'w', b're',
                          b'm', b'l', b'c', b'h', b'f', b'f*', b'S', b'd', b'cm'], ('invalid appearance operator', op, raw)
            for arg in args:
                if isinstance(arg, (FloatObject, NumberObject)):
                    assert math.isfinite(float(arg)), ('nonfinite operand', raw)
        result.append(raw)
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--dll', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args(); args.output.mkdir(parents=True, exist_ok=False)
    bases = {}
    for kind in ['text', 'radio']:
        base = args.output/f'{kind}-base.pdf'; HELPERS['fixture'](base, kind, .5, 'bs', 1)
        bases[kind] = base
    engine = HELPERS['HELPERS']['HELPERS']['Pdfium'](args.dll)
    rows = []
    values = [('positive-overflow', '9'*39+'.0', '2147483647.0'),
              ('negative-overflow', '-'+'9'*39+'.0', '-2147483648.0'),
              ('large-finite', '3'+'0'*38+'.0', '2147483647.0'),
              ('negative-finite', '-3'+'0'*38+'.0', '-2147483648.0'),
              ('signed-limit', '2147483648.0', '2147483647.0'),
              ('unsigned-limit', '2147483648', '-2147483648.0'),
              ('small-fraction', '0.0000001', '0.0000001')]
    try:
        for kind, base in bases.items():
          for label, token, canonical in values:
            for storage in ['bs', 'border', 'indirect']:
                for style in ['S', 'D', 'B', 'I', 'U']:
                    name = f'{kind}-{label}-{storage}-{style}'
                    outputs = []
                    for variant, value in [('source', token), ('canonical', canonical)]:
                        source=args.output/f'{name}-{variant}.pdf'; target=args.output/f'{name}-{variant}-saved.pdf'
                        fixture(base, source, value, storage, style)
                        HELPERS['HELPERS']['native_save'](engine, source, target)
                        outputs.append(target)
                    try:
                        actual, expected = map(appearance, outputs)
                        assert actual == expected, ('legacy numeric fallback changed', actual, expected)
                        row = dict(case=name, passed=True)
                    except AssertionError as error:
                        row = dict(case=name, passed=False, error=str(error))
                    rows.append(row);print(name, row['passed'], row.get('error', ''), flush=True)
    finally:
        engine.close()
    report = dict(passed=all(row['passed'] for row in rows), cases=rows,
                  dllSha256=hashlib.sha256(args.dll.read_bytes()).hexdigest(),
                  scriptSha256=hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
                  limits=['Saved AP finite operands and legacy fallback equivalence; no screenshot of focused native editor.'])
    (args.output/'results.json').write_text(json.dumps(report,indent=2)+'\n',encoding='utf-8')
    raise SystemExit(0 if report['passed'] else 1)


if __name__ == '__main__':
    main()
