#!/usr/bin/env python3
"""Render the app icon files from crates/marg/assets/marg.svg.

Writes marg-256.png (window icon), marg.ico (Windows .exe icon, with
PNG entries, which Windows 10/11 read at every size) and marg.icns (macOS
app bundle, PNG entries). Needs rsvg-convert.

    python3 scripts/make-icons.py
"""
import pathlib
import struct
import subprocess
import tempfile

ASSETS = pathlib.Path(__file__).resolve().parent.parent / "crates/marg/assets"
SVG = ASSETS / "marg.svg"
ICO_SIZES = [16, 24, 32, 48, 64, 128, 256]
# ICNS entry types holding PNGs, by pixel size (@2x types share sizes).
ICNS_TYPES = [
    (b"icp4", 16), (b"icp5", 32), (b"ic11", 32), (b"ic12", 64),
    (b"ic07", 128), (b"ic13", 256), (b"ic08", 256), (b"ic14", 512),
    (b"ic09", 512), (b"ic10", 1024),
]


def render(size, out):
    subprocess.run(
        ["rsvg-convert", "-w", str(size), "-h", str(size), str(SVG), "-o", str(out)],
        check=True,
    )
    return out.read_bytes()


def ico(pngs):
    """ICO header, one directory entry per image (0 means 256), then the PNGs."""
    head = struct.pack("<HHH", 0, 1, len(pngs))
    offset = 6 + 16 * len(pngs)
    entries = b""
    for size, png in pngs:
        entries += struct.pack("<BBBBHHII", size % 256, size % 256, 0, 0, 1, 32, len(png), offset)
        offset += len(png)
    return head + entries + b"".join(png for _, png in pngs)


def icns(pngs):
    """'icns' and the total length, then type, length and PNG per entry."""
    body = b"".join(t + struct.pack(">I", 8 + len(pngs[s])) + pngs[s] for t, s in ICNS_TYPES)
    return b"icns" + struct.pack(">I", 8 + len(body)) + body


with tempfile.TemporaryDirectory() as tmp:
    sizes = sorted(set(ICO_SIZES) | {s for _, s in ICNS_TYPES})
    pngs = {s: render(s, pathlib.Path(tmp) / f"{s}.png") for s in sizes}
render(256, ASSETS / "marg-256.png")
(ASSETS / "marg.ico").write_bytes(ico([(s, pngs[s]) for s in ICO_SIZES]))
(ASSETS / "marg.icns").write_bytes(icns(pngs))
