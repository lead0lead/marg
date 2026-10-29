#!/usr/bin/env python3
"""Render the app icon files from crates/marg/assets/marg.svg.

Writes marg-256.png (window icon) and marg.ico (Windows .exe icon, with
PNG entries, which Windows 10/11 read at every size). Needs rsvg-convert.

    python3 scripts/make-icons.py
"""
import pathlib
import struct
import subprocess
import tempfile

ASSETS = pathlib.Path(__file__).resolve().parent.parent / "crates/marg/assets"
SVG = ASSETS / "marg.svg"
ICO_SIZES = [16, 24, 32, 48, 64, 128, 256]


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


with tempfile.TemporaryDirectory() as tmp:
    pngs = [(s, render(s, pathlib.Path(tmp) / f"{s}.png")) for s in ICO_SIZES]
render(256, ASSETS / "marg-256.png")
(ASSETS / "marg.ico").write_bytes(ico(pngs))
