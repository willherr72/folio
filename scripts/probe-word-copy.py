"""Create diagnostic controls with an installed Microsoft Word PDF exporter.

Uses a new hidden COM instance, generated documents, and no clipboard operations.
Requires pywin32. Refuses an existing output directory.
"""
import argparse
import hashlib
import json
from pathlib import Path

import win32com.client

CASES = [('single', 'A B', 144), ('spaces', 'A  B', 144),
         ('soft', 'A B C D', 54), ('soft-spaces', 'A  B C D', 54),
         ('hard', 'A\nB', 144), ('hard-equivalent', 'A B\nC D', 54)]


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=False)
    word = document = None
    rows = []
    try:
        word = win32com.client.DispatchEx('Word.Application')
        word.Visible = False
        word.DisplayAlerts = 0
        word.AutomationSecurity = 3
        version, build = word.Version, word.Build
        print('Started hidden Word', version, build, flush=True)
        for name, logical, width in CASES:
            print('Creating', name, flush=True)
            document = word.Documents.Add()
            setup = document.PageSetup
            setup.PageWidth, setup.PageHeight = 240, 180
            setup.TopMargin = setup.BottomMargin = 36
            setup.LeftMargin, setup.RightMargin = 48, 240 - 48 - width
            word_text = logical.replace('\n', '\v')
            document.Content.Text = word_text
            document.Content.Font.Name = 'Courier New'
            document.Content.Font.Size = 20
            paragraph = document.Content.ParagraphFormat
            paragraph.SpaceBefore = paragraph.SpaceAfter = 0
            paragraph.LineSpacingRule, paragraph.LineSpacing = 4, 28
            paragraph.Alignment = 0
            document.RemovePersonalInformation = True
            document.Repaginate()
            actual = document.Content.Text
            assert actual == word_text + '\r', 'Word changed source text'
            pages, lines = document.ComputeStatistics(2), document.ComputeStatistics(1)
            assert pages == 1
            assert lines == (1 if name in ['single', 'spaces'] else 2), (name, lines)
            docx = output / (name + '.docx')
            print('Saving', name, flush=True)
            document.SaveAs2(str(docx), 16)
            for tags in [True, False]:
                pdf = output / (name + ('-tagged.pdf' if tags else '-untagged.pdf'))
                # PDF, closed after export, print quality, whole document, no
                # properties/IRM/bookmarks; vary only tags; no PDF/A conversion.
                document.ExportAsFixedFormat(str(pdf), 17, False, 0, 0, 1, 1, 0,
                                             False, False, 0, tags, True, False)
                rows.append({'name': name, 'expected': logical, 'wordText': actual,
                             'bodyWidthPt': width, 'wordLines': lines, 'wordPages': pages,
                             'tagged': tags, 'pdf': pdf.name, 'pdfSha256': sha(pdf),
                             'docx': docx.name, 'docxSha256': sha(docx)})
            document.Close(0)
            document = None
        manifest = {
            'description': 'Microsoft Word PDF-export control; engine evidence, not interactive clipboard acceptance.',
            'wordVersion': version, 'wordBuild': build, 'font': 'Courier New', 'fontSize': 20,
            'hardBreakMapping': 'Logical LF is authored as a Word manual line break (U+000B). Final Word paragraph mark is not logical source text.',
            'cases': rows,
        }
        (output / 'manifest.json').write_text(json.dumps(manifest, indent=2) + '\n', encoding='utf-8')
        print('Generated', len(rows), 'Word PDFs in', output, flush=True)
    finally:
        try:
            if document is not None:
                document.Close(0)
        finally:
            if word is not None:
                word.Quit(0)


if __name__ == '__main__':
    main()
