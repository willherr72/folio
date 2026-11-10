"""Independent saved-link metadata, MuPDF navigation and unchanged-content oracle."""
import argparse
import hashlib
import json
from pathlib import Path

import fitz
import pypdf
from pypdf.generic import IndirectObject, NullObject


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def reader(path):
    doc = pypdf.PdfReader(path)
    if doc.is_encrypted:
        assert doc.decrypt('reader'), path
    return doc


def native(path):
    doc = fitz.open(path)
    if doc.needs_pass:
        assert doc.authenticate('reader'), path
    return doc


def links(page):
    return {str(a.get_object()['/NM']): a.get_object() for a in page['/Annots']
            if a.get_object()['/Subtype'] == '/Link'}


def destination(doc, value, expected, mode=None, parameters=None):
    value = value.get_object()
    assert isinstance(value[0], IndirectObject), value
    target = doc.pages[expected].indirect_reference
    assert (value[0].idnum, value[0].generation) == (target.idnum, target.generation), value
    if mode is not None:
        assert str(value[1]) == mode, value
    if parameters is not None:
        actual = [None if isinstance(v, NullObject) else float(v) for v in value[2:]]
        assert actual == parameters, (actual, parameters)


def verify(corpus, case, order, label, desktop=False):
    output = corpus / f'{case}-output.pdf'
    sources = {key: corpus / f'{case}-{key}.pdf' for key, _ in order}
    paths = [*sources.values(), output]
    hashes = {p.name: digest(p) for p in paths}
    saved = reader(output)
    originals = {key: reader(path) for key, path in sources.items()}
    assert len(saved.pages) == len(order)
    checks = []
    with native(output) as rendered:
        for index, (key, old_index) in enumerate(order):
            before = originals[key].pages[old_index]
            after = saved.pages[index]
            old_links, new_links = links(before), links(after)
            assert list(old_links) == list(new_links) == ['direct', 'named', 'uri', 'self', 'chain']
            other = next((i for i, item in enumerate(order) if item == (key, 1 - old_index)), None)
            for name, link in new_links.items():
                for field in ['/Rect', '/Border', '/C', '/NM']:
                    assert link[field].get_object() == old_links[name][field].get_object(), (case, index, name, field)
                assert link.raw_get('/P') == after.indirect_reference
            if other is None:
                assert '/Dest' not in new_links['direct'] and '/A' not in new_links['named']
            else:
                mode, params = ('/XYZ', [None, 73, None]) if case == 'legacy' and old_index == 0 else ('/Fit', [])
                destination(saved, new_links['direct']['/Dest'], other, mode, params)
                destination(saved, new_links['named']['/A']['/D'], other, mode, params)
            destination(saved, new_links['self']['/Dest'], index, '/XYZ', [20, None, 1.25])
            uri = new_links['uri']['/A']
            assert uri['/S'] == '/URI' and uri['/URI'] == f'https://example.invalid/{label[key]}'
            assert uri['/IsMap'].value is False
            chain = new_links['chain']['/A']
            assert chain['/S'] == '/URI' and chain['/URI'] == 'https://example.invalid/chain'
            following = chain['/Next']
            assert len(following) == (1 if other is None else 2)
            if other is not None:
                destination(saved, following[0]['/D'], other, mode, params)
            remote = following[-1]
            assert remote['/S'] == '/GoToR' and remote['/F'] == 'external.pdf'
            assert remote['/D'] == 'remote-target' and remote['/NewWindow'].value is True
            destination(saved, new_links['chain']['/AA']['/E']['/D'], index, '/FitH', [None])

            # Fit and XYZ-with-null may be exposed by MuPDF as named-style link
            # records with one-based page strings. Both forms must resolve to the
            # same actual page already checked through independent object IDs.
            parsed = {}
            for link in rendered[index].get_links():
                # MuPDF omits all xref/name fields when some Link annotations
                # are disabled. Bind using the distinct, verified rectangles.
                names = [name for name, item in new_links.items()
                         if (fitz.Rect(item['/Rect']) * rendered[index].transformation_matrix) == link['from']]
                assert len(names) == 1, link
                assert names[0] not in parsed
                parsed[names[0]] = link
            for name, target in [('direct', other), ('named', other), ('self', index)]:
                if target is None:
                    assert name not in parsed or parsed[name]['kind'] == fitz.LINK_NONE
                else:
                    link = parsed[name]
                    assert link['kind'] in (fitz.LINK_GOTO, fitz.LINK_NAMED)
                    assert int(link['page']) - (1 if isinstance(link['page'], str) else 0) == target
            assert parsed['uri']['uri'] == uri['/URI']
            assert after.extract_text() == before.extract_text()
            with native(sources[key]) as original:
                assert rendered[index].get_text() == original[old_index].get_text()
                for scale in [1, 2]:
                    a = rendered[index].get_pixmap(matrix=fitz.Matrix(scale, scale))
                    b = original[old_index].get_pixmap(matrix=fitz.Matrix(scale, scale))
                    assert (a.width, a.height, a.samples) == (b.width, b.height, b.samples), (case, index, scale)
            checks.append(dict(page=index, source=key, sourcePage=old_index, otherTarget=other, selfTarget=index))
    if saved.is_encrypted:
        original = next(iter(originals.values()))
        assert original.is_encrypted
        assert saved.trailer['/Encrypt']['/P'] == original.trailer['/Encrypt']['/P']
    assert hashes == {p.name: digest(p) for p in paths}
    return dict(case=case, passed=True, pages=checks, sha256=hashes, desktop=desktop)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--corpus', required=True, type=Path)
    parser.add_argument('--output', required=True, type=Path)
    parser.add_argument('--desktop', action='store_true')
    args = parser.parse_args()
    args.output.mkdir(parents=True, exist_ok=False)
    standard = [('source', 0), ('source', 1)]
    cases = [('save', standard, {'source': 'Original'})] if args.desktop else [
        ('save', standard, {'source': 'Original'}),
        ('reorder', [('source', 1), ('source', 0), ('source', 1)], {'source': 'Original'}),
        ('extract', [('source', 0)], {'source': 'Original'}),
        ('combined', [('a', 1), ('b', 1), ('a', 0), ('b', 0)], {'a': 'A', 'b': 'B'}),
        ('legacy', [('source', 1), ('source', 0)], {'source': 'Legacy'}),
        ('indirect', standard, {'source': 'Indirect'}),
        ('protected', standard, {'source': 'Protected'}),
    ]
    records = [verify(args.corpus, *case, desktop=args.desktop) for case in cases]
    result = dict(passed=True, cases=records, mupdf=fitz.VersionBind, pypdf=pypdf.__version__,
                  checks='Local and remote destinations, null/view parameters, actions, annotation appearance, permissions, exact text/raster at 1x/2x, input hashes; no actions executed')
    (args.output / 'result.json').write_text(json.dumps(result, indent=2) + '\n', encoding='utf-8')
    print(json.dumps(dict(passed=True, cases=len(records), pages=sum(len(r['pages']) for r in records))))


if __name__ == '__main__':
    main()
