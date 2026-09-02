#!/usr/bin/env python3
"""Regenerate the size-specific favicon marks in assets/.

rsreadline's tab icon can't just be the main logo shrunk — four lines of text
turn to mush below ~48px. So each small size is drawn on its own pixel grid:

  16px  pure rectangles: a prompt line, one suggestion, one selected suggestion
  32px  each character is an abstract 2x3 blob; the selected row is reverse-video
  48px  same idea with 3x4 blobs, a little more detail

The first two blobs of every row reuse the same pattern as the typed "rs", so
the "it completes what you typed" story survives even when the glyphs aren't
legible.

Outputs (committed): assets/favicon-16.png, -32.png, -48.png, favicon.ico
Requires: inkscape, ImageMagick (`convert`). Run from anywhere.
"""
import os
import subprocess
import tempfile

BG = "#1B1B1B"    # terminal background
FG = "#E8E8E8"    # selected suggestion (reverse-video box)
TYPED = "#BFBFBF"  # the query you typed
MUTED = "#8A8A8A"  # unselected suggestions / prompt, so the selection pops
SIG = "#5F5F5F"    # dim prompt sigil

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
ASSETS = os.path.join(ROOT, "assets")


def rect(x, y, w, h, fill):
    return f'<rect x="{x}" y="{y}" width="{w}" height="{h}" fill="{fill}"/>'


def svg(n, body):
    return (f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {n} {n}" '
            f'width="{n}" height="{n}" shape-rendering="crispEdges">\n'
            f'{rect(0, 0, n, n, BG)}\n' + "\n".join(body) + "\n</svg>\n")


# --------------------------------------------------------------- 16px: lines
def make16():
    b = [
        rect(3, 2, 2, 2, SIG),     # sigil, left-aligned with the suggestions
        rect(6, 2, 4, 2, TYPED),   # typed 'rs'
        rect(3, 7, 6, 2, MUTED),   # suggestion 1
        rect(1, 10, 13, 4, FG),    # suggestion 2 selected: reverse-video box
        rect(3, 11, 9, 2, BG),     #   (2px sides / 1px top-bottom) around a 2px bar
    ]
    return svg(16, b)


# ----------------------------------------------- 32 / 48px: abstract glyphs
PAT23 = [["10", "11", "10"], ["11", "10", "01"], ["11", "11", "10"],
         ["10", "11", "11"], ["11", "10", "11"], ["01", "11", "01"]]
PAT34 = [["110", "101", "100", "100"], ["011", "110", "001", "110"],
         ["010", "111", "101", "101"], ["100", "110", "101", "110"],
         ["011", "100", "100", "011"], ["001", "011", "101", "011"]]
ORDER = [0, 1, 2, 3, 4, 5, 2, 3, 4, 5, 2, 3]  # idx 0/1 == r/s in every row


def glyph(px, py, pat, cw, fill):
    out = []
    for ry, row in enumerate(pat):
        rx = 0
        while rx < cw:
            if row[rx] == "1":
                x0 = rx
                while rx < cw and row[rx] == "1":
                    rx += 1
                out.append(rect(px + x0, py + ry, rx - x0, 1, fill))
            else:
                rx += 1
    return out


def make_glyphscene(n, pat, cw, ch, gap):
    margin = 2
    pitch = cw + 1
    mx = 3 if n <= 32 else 4          # content left edge (nudged right of margin)
    sel_pad = 1
    g = gap                          # even spacing between suggestions
    g_cmd = gap + (1 if n <= 32 else 2)  # a bit more air after the command line

    def fit(count):                  # cap a row so it keeps the right-side margin
        return min(count, (n - margin - mx) // pitch)

    total = ch + g_cmd + ch + g + (ch + 2 * sel_pad) + g + ch
    top = (n - total) // 2
    y_prompt = top
    y_s1 = y_prompt + ch + g_cmd
    y_box = y_s1 + ch + g
    y_s2 = y_box + sel_pad
    y_s3 = y_box + ch + 2 * sel_pad + g

    b = []
    # prompt: solid sigil (left-aligned with suggestions) + typed 'rs' + cursor
    b.append(rect(mx, y_prompt, cw, ch, SIG))
    for i in range(2):
        b += glyph(mx + (i + 1) * pitch, y_prompt, pat[ORDER[i]], cw, TYPED)
    b.append(rect(mx + 3 * pitch + 1, y_prompt - 1, cw + 1, ch + 2, TYPED))
    # suggestion 1 (muted)
    for i in range(fit(5)):
        b += glyph(mx + i * pitch, y_s1, pat[ORDER[i]], cw, MUTED)
    # suggestion 2 (selected): reverse-video box + knockout glyphs
    k = fit(10)
    box_x = mx - 1
    box_w = min(k * pitch + 1, n - margin - box_x)
    b.append(rect(box_x, y_box, box_w, ch + 2 * sel_pad, FG))
    for i in range(k):
        b += glyph(mx + i * pitch, y_s2, pat[ORDER[i]], cw, BG)
    # suggestion 3 (muted)
    for i in range(fit(7)):
        b += glyph(mx + i * pitch, y_s3, pat[ORDER[i]], cw, MUTED)
    return svg(n, b)


MARKS = {16: make16(), 32: make_glyphscene(32, PAT23, 2, 3, 2),
         48: make_glyphscene(48, PAT34, 3, 4, 3)}


def main():
    os.makedirs(ASSETS, exist_ok=True)
    pngs = []
    with tempfile.TemporaryDirectory() as tmp:
        for n, data in MARKS.items():
            src = os.path.join(tmp, f"favicon-{n}.svg")
            png = os.path.join(ASSETS, f"favicon-{n}.png")
            open(src, "w").write(data)
            subprocess.run(["inkscape", src, "--export-type=png",
                            f"--export-filename={png}", "-w", str(n), "-h", str(n)],
                           check=True, capture_output=True)
            pngs.append(png)
        subprocess.run(["convert", *pngs, os.path.join(ASSETS, "favicon.ico")],
                       check=True)
    print("wrote", ", ".join(os.path.basename(p) for p in pngs), "+ favicon.ico")


if __name__ == "__main__":
    main()
