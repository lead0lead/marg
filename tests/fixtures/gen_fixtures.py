#!/usr/bin/env python3
"""Generate marg's fixture PDFs. Standard library only; output is deterministic.

Usage: python3 tests/fixtures/gen_fixtures.py
"""

from pathlib import Path

HERE = Path(__file__).resolve().parent


def pdf_escape(s: str) -> str:
    return s.replace("\\", "\\\\").replace("(", "\\(").replace(")", "\\)")


def text_stream(lines, x=72, y=720, size=12, leading=16):
    out = [f"BT /F1 {size} Tf {leading} TL {x} {y} Td"]
    for line in lines:
        out.append(f"({pdf_escape(line)}) Tj T*")
    out.append("ET")
    return "\n".join(out).encode("latin-1")


def write_pdf(path: Path, objects: dict, root: int, info: int, doc_id: str):
    """objects: {num: bytes body (without 'obj'/'endobj')}; numbers 1..n contiguous."""
    buf = bytearray(b"%PDF-1.7\n%\xe2\xe3\xcf\xd3\n")
    offsets = {}
    for num in sorted(objects):
        offsets[num] = len(buf)
        buf += f"{num} 0 obj\n".encode() + objects[num] + b"\nendobj\n"
    xref = len(buf)
    n = max(objects) + 1
    buf += f"xref\n0 {n}\n0000000000 65535 f \n".encode()
    for num in range(1, n):
        buf += f"{offsets[num]:010d} 00000 n \n".encode()
    buf += (
        f"trailer\n<< /Size {n} /Root {root} 0 R /Info {info} 0 R /ID [<{doc_id}> <{doc_id}>] >>\n"
        f"startxref\n{xref}\n%%EOF\n"
    ).encode()
    path.write_bytes(bytes(buf))


def stream(data: bytes) -> bytes:
    return f"<< /Length {len(data)} >>\nstream\n".encode() + data + b"\nendstream"


def text_pdf():
    """Two portrait pages: text, outline, internal + external link, page labels, /ID."""
    page1 = text_stream(
        [
            "marg fixture: text page one",
            "",
            "The quick brown fox jumps over the lazy dog.",
            "Highlights, underlines and comments are saved",
            "into the PDF as standard annotations.",
            "",
            "Go to page two.",
            "Visit example.com.",
        ]
    )
    page2 = text_stream(
        [
            "marg fixture: text page two",
            "",
            "Second page with a little more text for search.",
        ]
    )
    # Text lines are at y = 720 - 16*i; "Go to page two." is line 6 -> y=624.
    objs = {
        1: b"<< /Type /Catalog /Pages 2 0 R /Outlines 9 0 R /PageMode /UseOutlines"
        b" /PageLabels << /Nums [0 << /S /r >> 1 << /S /D /St 1 >>] >> >>",
        2: b"<< /Type /Pages /Kids [3 0 R 4 0 R] /Count 2 >>",
        3: b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792]"
        b" /Resources << /Font << /F1 5 0 R >> >> /Contents 6 0 R /Annots [12 0 R 13 0 R] >>",
        4: b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792]"
        b" /Resources << /Font << /F1 5 0 R >> >> /Contents 7 0 R >>",
        5: b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding /WinAnsiEncoding >>",
        6: stream(page1),
        7: stream(page2),
        8: b"<< /Producer (marg gen_fixtures.py) /Title (marg text fixture) >>",
        9: b"<< /Type /Outlines /First 10 0 R /Last 11 0 R /Count 2 >>",
        10: b"<< /Title (Page one) /Parent 9 0 R /Next 11 0 R /Dest [3 0 R /XYZ 0 792 0] >>",
        11: b"<< /Title (Page two) /Parent 9 0 R /Prev 10 0 R /Dest [4 0 R /XYZ 0 792 0] >>",
        12: b"<< /Type /Annot /Subtype /Link /Rect [72 620 160 636] /Border [0 0 0]"
        b" /Dest [4 0 R /XYZ 0 792 0] >>",
        13: b"<< /Type /Annot /Subtype /Link /Rect [72 604 180 620] /Border [0 0 0]"
        b" /A << /S /URI /URI (https://example.com/) >> >>",
    }
    write_pdf(HERE / "text.pdf", objs, 1, 8, "6d617267746578746669787475726531")


def boxes_pdf():
    """Pages whose size comes from Rotate, CropBox, offset or inherited MediaBox, UserUnit."""
    objs = {
        1: b"<< /Type /Catalog /Pages 2 0 R >>",
        # Inherited MediaBox (A4) for pages without their own.
        2: b"<< /Type /Pages /Kids [3 0 R 4 0 R 5 0 R 6 0 R 7 0 R] /Count 5 /MediaBox [0 0 595 842] >>",
        # 1: rotated letter -> 792 x 612.
        3: b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Rotate 90"
        b" /Resources << /Font << /F1 8 0 R >> >> /Contents 9 0 R >>",
        # 2: CropBox inside MediaBox -> 500 x 600.
        4: b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 600 800] /CropBox [50 100 550 700]"
        b" /Resources << /Font << /F1 8 0 R >> >> /Contents 10 0 R >>",
        # 3: offset MediaBox -> 300 x 400; text near its top-left corner.
        5: b"<< /Type /Page /Parent 2 0 R /MediaBox [100 200 400 600]"
        b" /Resources << /Font << /F1 8 0 R >> >> /Contents 11 0 R >>",
        # 4: inherited MediaBox -> 595 x 842.
        6: b"<< /Type /Page /Parent 2 0 R"
        b" /Resources << /Font << /F1 8 0 R >> >> /Contents 9 0 R >>",
        # 5: UserUnit 2 -> 600 x 800.
        7: b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 300 400] /UserUnit 2"
        b" /Resources << /Font << /F1 8 0 R >> >> /Contents 9 0 R >>",
        8: b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding /WinAnsiEncoding >>",
        9: stream(text_stream(["marg fixture: page boxes"], x=20, y=380)),
        10: stream(text_stream(["marg fixture: cropped"], x=60, y=680)),
        11: stream(text_stream(["marg fixture: offset"], x=105, y=585)),
        12: b"<< /Producer (marg gen_fixtures.py) /Title (marg boxes fixture) >>",
    }
    write_pdf(HERE / "boxes.pdf", objs, 1, 12, "6d61726762786573666978747572653031")


def columns_pdf():
    """Two pages in two columns: reading order, paragraphs, sentences, a hyphenated line break."""
    left = [
        "Readers mark what matters. Most",
        "tools need a mouse for this; marg",
        "uses hints and a caret instead.",
        "",
        "The second paragraph starts here.",
        "It talks about annota-",
        "tions in the PDF itself.",
    ]
    right = [
        "The right column comes next in",
        "reading order. Does selection",
        "follow it? It should!",
        "",
        "A last paragraph ends the page.",
    ]
    page1 = b"\n".join(
        [
            text_stream(["Two columns: a reading order test"], x=72, y=740, size=14),
            text_stream(left, x=72, y=700, size=10, leading=14),
            text_stream(right, x=320, y=700, size=10, leading=14),
        ]
    )
    page2 = text_stream(["Page two has one column of text."], x=72, y=740, size=10)
    objs = {
        1: b"<< /Type /Catalog /Pages 2 0 R >>",
        2: b"<< /Type /Pages /Kids [3 0 R 4 0 R] /Count 2 >>",
        3: b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792]"
        b" /Resources << /Font << /F1 5 0 R >> >> /Contents 6 0 R >>",
        4: b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792]"
        b" /Resources << /Font << /F1 5 0 R >> >> /Contents 7 0 R >>",
        5: b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding /WinAnsiEncoding >>",
        6: stream(page1),
        7: stream(page2),
        8: b"<< /Producer (marg gen_fixtures.py) /Title (marg columns fixture) >>",
    }
    write_pdf(HERE / "columns.pdf", objs, 1, 8, "6d617267636f6c756d6e736669787431")


def figure_pdf():
    """One page with text and an image (a 4x4 color pattern drawn at 100,400 .. 300,550 in
    PDF space, i.e. top-left (100, 242) .. (300, 392) from the page's top-left corner)."""
    pixels = bytes(
        c
        for y in range(4)
        for x in range(4)
        for c in ((0xE0, 0x40, 0x40) if (x + y) % 2 else (0x40, 0x60, 0xE0))
    )
    image = (
        b"<< /Type /XObject /Subtype /Image /Width 4 /Height 4 /ColorSpace /DeviceRGB"
        b" /BitsPerComponent 8 /Length " + str(len(pixels)).encode() + b" >>\nstream\n"
        + pixels + b"\nendstream"
    )
    page = b"\n".join(
        [
            text_stream(["marg fixture: a figure", "", "Figure 1 is below."], x=72, y=720),
            b"q 200 0 0 150 100 400 cm /Im1 Do Q",
            text_stream(["Figure 1: a pattern."], x=100, y=380, size=10),
        ]
    )
    objs = {
        1: b"<< /Type /Catalog /Pages 2 0 R >>",
        2: b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>",
        3: b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792]"
        b" /Resources << /Font << /F1 4 0 R >> /XObject << /Im1 5 0 R >> >> /Contents 6 0 R >>",
        4: b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding /WinAnsiEncoding >>",
        5: image,
        6: stream(page),
        7: b"<< /Producer (marg gen_fixtures.py) /Title (marg figure fixture) >>",
    }
    write_pdf(HERE / "figure.pdf", objs, 1, 7, "6d61726766696775726566697874310a")


if __name__ == "__main__":
    text_pdf()
    boxes_pdf()
    columns_pdf()
    figure_pdf()
