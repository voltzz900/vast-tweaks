#!/usr/bin/env python3
"""Draws icons/vasttweaks.png (256x256): a script page and a crafting arrow out of it."""
import struct
import zlib
from pathlib import Path

W = 32


def png(pixels):
    raw = b"".join(b"\x00" + bytes(c for px in row for c in px) for row in pixels)

    def chunk(kind, data):
        return struct.pack(">I", len(data)) + kind + data + struct.pack(">I", zlib.crc32(kind + data) & 0xFFFFFFFF)

    return (b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", len(pixels[0]), len(pixels), 8, 6, 0, 0, 0))
            + chunk(b"IDAT", zlib.compress(raw, 9)) + chunk(b"IEND", b""))


def main():
    bg, page, edge, ink, accent = (0x1B, 0x1E, 0x24), (0xE8, 0xDF, 0xC6), (0x9C, 0x8E, 0x6A), (0x4A, 0x52, 0x60), (0xE0, 0xA2, 0x34)
    px = [[(*bg, 255)] * W for _ in range(W)]
    for y in range(4, 28):                      # the page, its corner folded
        for x in range(5, 21):
            if x - 5 + (27 - y) < 1:
                continue
            fold = x >= 17 and y <= 7 and (x - 17) > (y - 4)
            px[y][x] = (*(edge if fold or x in (5, 20) or y in (4, 27) else page), 255)
    for row, length in ((8, 9), (11, 12), (14, 7), (17, 11), (20, 8), (23, 10)):   # lines of script
        for x in range(7, 7 + length):
            px[row][x] = (*(accent if row == 14 else ink), 255)
    for x in range(22, 29):                     # the arrow out: what the script makes
        px[16][x] = (*accent, 255)
    for k in range(1, 4):
        px[16 - k][28 - k] = px[16 + k][28 - k] = (*accent, 255)
    big = [[px[y // 8][x // 8] for x in range(W * 8)] for y in range(W * 8)]
    out = Path(__file__).resolve().parent.parent / "icons/vasttweaks.png"
    out.write_bytes(png(big))
    print(out)


if __name__ == "__main__":
    main()
