"""Render the README benchmark charts as light and dark SVGs.

Reads the per-trial CSVs in docs/benchmarks/results, takes medians, and writes
docs/assets/benchmarks/{headline,size,tradeoff}-{light,dark}.svg. No
third-party packages.

    python scripts/benchmark/make_charts.py
"""

from __future__ import annotations

import csv
import math
import statistics
from collections import defaultdict
from pathlib import Path
from xml.sax.saxutils import escape

ROOT = Path(__file__).resolve().parents[2]
RESULTS = ROOT / "docs" / "benchmarks" / "results"
OUT = ROOT / "docs" / "assets" / "benchmarks"
STAMP = "2026-10-31-i7-14700f"
# Other tools come from the baseline run; MRAR/2 from its own runs. The random
# corpus was re-run alone (see results/README.md).
SOURCES = [
    (f"{STAMP}-baseline.csv", lambda r: r["tool"] != "mrar"),
    (f"{STAMP}-mrar-v2.csv", lambda r: r["corpus"] != "random"),
    (f"{STAMP}-mrar-v2-random.csv", lambda r: True),
]
MIB = 1024 * 1024
FONT = 'system-ui, -apple-system, "Segoe UI", sans-serif'

TOOLS = {
    "mrar": "MRAR/2",
    "7z-mx9": "7-Zip -mx9",
    "7z-mx5": "7-Zip -mx5",
    "tar-xz9": "tar + xz -9",
    "tar-zstd19": "tar + zstd -19",
    "tar-zstd3": "tar + zstd -3",
    "zip-7z": "ZIP Deflate",
}
CORPORA = {
    "text": "Text, 256 MiB",
    "small-files": "4,096 small files, 64 MiB",
    "mixed": "Mixed text and random, 256 MiB",
    "node_modules": "Real node_modules, 1,550 files, 71 MiB",
}

THEMES = {
    "light": {
        "surface": "#fcfcfb",
        "ink": "#0b0b0b",
        "ink2": "#52514e",
        "muted": "#898781",
        "grid": "#e1e0d9",
        "axis": "#c3c2b7",
        "accent": "#2a78d6",
        "other": "#a9a79f",
        "win": "#006300",
        "loss": "#b42f2f",
        "border": "rgba(11,11,11,0.10)",
    },
    "dark": {
        "surface": "#1a1a19",
        "ink": "#f0efec",
        "ink2": "#c3c2b7",
        "muted": "#898781",
        "grid": "#2c2c2a",
        "axis": "#383835",
        "accent": "#3987e5",
        "other": "#6b6a64",
        "win": "#0ca30c",
        "loss": "#e66767",
        "border": "rgba(255,255,255,0.10)",
    },
}


def load() -> dict[tuple[str, str], dict[str, float]]:
    trials: dict[tuple[str, str], dict[str, list[float]]] = defaultdict(lambda: defaultdict(list))
    for name, keep in SOURCES:
        with open(RESULTS / name, newline="", encoding="utf-8") as f:
            for row in csv.DictReader(f):
                if not keep(row):
                    continue
                cell = trials[(row["corpus"], row["tool"])]
                for key in ("archive_bytes", "create_s", "extract_s", "create_peak_mib", "input_bytes"):
                    cell[key].append(float(row[key]))
    return {k: {m: statistics.median(v) for m, v in cell.items()} for k, cell in trials.items()}


def text(x, y, s, size=13, fill="ink", weight=400, anchor="start", t=None):
    return (
        f'<text x="{x:.1f}" y="{y:.1f}" font-size="{size}" font-weight="{weight}" '
        f'fill="{t[fill]}" text-anchor="{anchor}">{escape(s)}</text>'
    )


def svg(w, h, t, body, title, desc):
    return (
        f'<svg xmlns="http://www.w3.org/2000/svg" width="{w}" height="{h}" viewBox="0 0 {w} {h}" '
        f'font-family=\'{FONT}\' role="img" aria-labelledby="t d">'
        f"<title id=\"t\">{escape(title)}</title><desc id=\"d\">{escape(desc)}</desc>"
        f'<rect x="0.5" y="0.5" width="{w - 1}" height="{h - 1}" rx="12" fill="{t["surface"]}" '
        f'stroke="{t["border"]}"/>' + "".join(body) + "</svg>\n"
    )


def bar(x, y, w, h, fill):
    """Horizontal bar, square at the baseline, 4px rounded at the data end."""
    r = min(4, w / 2, h / 2)
    return (
        f'<path d="M{x:.1f},{y:.1f}h{w - r:.1f}a{r},{r} 0 0 1 {r},{r}v{h - 2 * r:.1f}'
        f'a{r},{r} 0 0 1 -{r},{r}h-{w - r:.1f}z" fill="{fill}"/>'
    )


def fmt_mib(v):
    return f"{v:.3f}" if v < 1 else f"{v:.1f}" if v >= 100 else f"{v:.2f}"


def pct(a, b):
    return (a / b - 1) * 100


def headline(d, t):
    text_m, text_7 = d[("text", "mrar")], d[("text", "7z-mx9")]
    small_m = d[("small-files", "mrar")]["archive_bytes"]
    small_best = min(v["archive_bytes"] for (c, k), v in d.items() if c == "small-files" and k != "mrar")
    nm_m, nm_7 = d[("node_modules", "mrar")], d[("node_modules", "7z-mx9")]
    tiles = [
        (f"{-pct(text_m['archive_bytes'], text_7['archive_bytes']):.0f}%", "smaller than 7-Zip -mx9", "256 MiB of text", "win"),
        (f"{text_7['create_s'] / text_m['create_s']:.0f}×", "faster to create", "same text, vs 7-Zip -mx9", "win"),
        (f"{-pct(small_m, small_best):.0f}%", "smaller than the best other tool", "4,096 small files", "win"),
        (f"+{pct(nm_m['archive_bytes'], nm_7['archive_bytes']):.0f}%", "larger than 7-Zip -mx9", "real node_modules", "loss"),
    ]
    w, h, pad, gap = 880, 176, 24, 12
    tw = (w - 2 * pad - 3 * gap) / 4
    body = [text(pad, 34, "MRAR/2 at a glance", 15, "ink", 600, t=t),
            text(w - pad, 34, "Median of 3 trials · i7-14700F · Windows 11", 12, "muted", anchor="end", t=t)]
    for i, (big, line1, line2, tone) in enumerate(tiles):
        x = pad + i * (tw + gap)
        body.append(f'<rect x="{x:.1f}" y="50" width="{tw:.1f}" height="102" rx="10" fill="none" stroke="{t["grid"]}"/>')
        body.append(f'<rect x="{x:.1f}" y="50" width="4" height="102" rx="2" fill="{t[tone]}"/>')
        body.append(text(x + 18, 96, big, 34, "ink", 650, t=t))
        body.append(text(x + 18, 120, line1, 13, "ink2", 500, t=t))
        body.append(text(x + 18, 139, line2, 12, "muted", t=t))
    desc = "; ".join(f"{b} {l1} ({l2})" for b, l1, l2, _ in tiles)
    return svg(w, h, t, body, "MRAR/2 benchmark headline", desc)


SIZE_TOOLS = ["mrar", "7z-mx9", "tar-xz9", "7z-mx5", "tar-zstd19"]


def size_chart(d, t):
    w, pad = 880, 24
    pw, gap_x = (w - 2 * pad - 32) / 2, 32
    row_h, bar_h = 26, 14
    ph = 40 + len(SIZE_TOOLS) * row_h + 14
    top = 78
    h = top + 2 * ph + 20 + 28
    label_w = 112
    body = [
        text(pad, 34, "Archive size: lower is better", 15, "ink", 600, t=t),
        text(pad, 56, "MiB, median of 3 trials. Every archive was extracted and checked byte for byte with SHA-256.", 12, "muted", t=t),
    ]
    for i, corpus in enumerate(CORPORA):
        px = pad + (i % 2) * (pw + gap_x)
        py = top + (i // 2) * (ph + 20)
        rows = [(k, d[(corpus, k)]["archive_bytes"] / MIB) for k in SIZE_TOOLS]
        rows.sort(key=lambda r: r[1])
        vmax = max(v for _, v in rows)
        mrar = d[(corpus, "mrar")]["archive_bytes"]
        ref = d[(corpus, "7z-mx9")]["archive_bytes"]
        delta = pct(mrar, ref)
        tone = "win" if delta < -0.5 else "loss" if delta > 0.5 else "ink2"
        sign = "−" if delta < 0 else "+"
        body.append(text(px, py + 14, CORPORA[corpus], 13, "ink", 600, t=t))
        body.append(text(px + pw, py + 14, f"MRAR/2 {sign}{abs(delta):.1f}% vs 7-Zip -mx9", 12, tone, 600, "end", t=t))
        bx, bw = px + label_w, pw - label_w - 58
        body.append(f'<line x1="{bx:.1f}" y1="{py + 30}" x2="{bx:.1f}" y2="{py + 30 + len(rows) * row_h - 6}" stroke="{t["axis"]}"/>')
        for j, (k, v) in enumerate(rows):
            y = py + 30 + j * row_h
            hl = k == "mrar"
            body.append(text(bx - 10, y + 11.5, TOOLS[k], 12, "ink" if hl else "ink2", 650 if hl else 400, "end", t=t))
            body.append(bar(bx, y, max(bw * v / vmax, 2), bar_h, t["accent"] if hl else t["other"]))
            body.append(text(bx + bw * v / vmax + 6, y + 11.5, fmt_mib(v), 12, "ink" if hl else "ink2", 650 if hl else 400, t=t))
    body.append(text(pad, h - 18, "Random data (not shown): every tool stores 128 MiB; MRAR/2 is the fastest to create, at 0.4 s.", 12, "muted", t=t))
    desc = "; ".join(
        f"{CORPORA[c]}: " + ", ".join(f"{TOOLS[k]} {d[(c, k)]['archive_bytes'] / MIB:.3f} MiB" for k in SIZE_TOOLS)
        for c in CORPORA
    )
    return svg(w, h, t, body, "Archive size by corpus", desc)


# Label placement per point: (dx, dy, anchor), tuned so labels do not collide.
PLACE = {
    "text": {
        "mrar": (10, -10, "start"), "7z-mx9": (10, -6, "start"), "tar-xz9": (10, 14, "start"),
        "7z-mx5": (0, -12, "middle"), "tar-zstd19": (-10, 4, "end"), "tar-zstd3": (10, 4, "start"),
        "zip-7z": (-10, 4, "end"),
    },
    "node_modules": {
        "mrar": (-12, -6, "end"), "7z-mx9": (6, 22, "start"), "tar-xz9": (10, -8, "start"),
        "7z-mx5": (-10, 18, "end"), "tar-zstd19": (10, 4, "start"), "tar-zstd3": (10, 4, "start"),
        "zip-7z": (10, 4, "start"),
    },
}


def tradeoff_chart(d, t):
    w, pad = 880, 24
    gap_x = 40
    pw = (w - 2 * pad - gap_x) / 2
    top, ph = 84, 290
    h = top + ph + 74
    body = [
        text(pad, 34, "Size against create time: down and to the left is better", 15, "ink", 600, t=t),
        text(pad, 56, "Each dot is one tool. Dashed line: the best size reachable at each speed (Pareto frontier). Time axis is logarithmic.", 12, "muted", t=t),
    ]
    for i, corpus in enumerate(PLACE):
        px = pad + i * (pw + gap_x)
        pts = {k: (d[(corpus, k)]["create_s"], d[(corpus, k)]["archive_bytes"] / MIB) for k in TOOLS}
        ys = [v for _, v in pts.values()]
        lo, hi = min(ys), max(ys)
        span = hi - lo
        lo, hi = lo - span * 0.08, hi + span * 0.08
        x0, x1 = 0.1, 1000.0
        ax, ay, aw, ah = px + 44, top + 36, pw - 56, ph - 64

        def sx(v):
            return ax + aw * (math.log10(v) - math.log10(x0)) / (math.log10(x1) - math.log10(x0))

        def sy(v):
            return ay + ah * (hi - v) / (hi - lo)

        body.append(text(px, top + 4, CORPORA[corpus], 13, "ink", 600, t=t))
        for tick in (0.1, 1, 10, 100, 1000):
            x = sx(tick)
            body.append(f'<line x1="{x:.1f}" y1="{ay}" x2="{x:.1f}" y2="{ay + ah}" stroke="{t["grid"]}"/>')
            body.append(text(x, ay + ah + 18, f"{tick:g} s", 11, "muted", anchor="middle", t=t))
        step = 5 if span > 12 else 2 if span > 4 else 1
        tick = math.ceil(lo / step) * step
        while tick <= hi:
            y = sy(tick)
            body.append(f'<line x1="{ax}" y1="{y:.1f}" x2="{ax + aw}" y2="{y:.1f}" stroke="{t["grid"]}"/>')
            body.append(text(ax - 8, y + 4, f"{tick:g}", 11, "muted", anchor="end", t=t))
            tick += step
        body.append(f'<line x1="{ax}" y1="{ay + ah}" x2="{ax + aw}" y2="{ay + ah}" stroke="{t["axis"]}"/>')
        body.append(text(ax + aw, ay + ah + 36, "Create time", 11, "muted", anchor="end", t=t))
        body.append(text(px, ay - 8, "MiB", 11, "muted", t=t))
        # Pareto frontier: points no other point beats on both time and size.
        order = sorted(pts.items(), key=lambda kv: kv[1][0])
        frontier, best = [], math.inf
        for k, (x, y) in order:
            if y < best:
                frontier.append((x, y))
                best = y
        path = " ".join(f"{'M' if j == 0 else 'L'}{sx(x):.1f},{sy(y):.1f}" for j, (x, y) in enumerate(frontier))
        path += f" L{ax + aw:.1f},{sy(frontier[-1][1]):.1f}"
        body.append(f'<path d="{path}" fill="none" stroke="{t["accent"]}" stroke-opacity="0.45" stroke-width="2" stroke-dasharray="5 4"/>')
        for k in sorted(pts, key=lambda k: k == "mrar"):
            x, y = pts[k]
            hl = k == "mrar"
            r = 7 if hl else 5
            body.append(f'<circle cx="{sx(x):.1f}" cy="{sy(y):.1f}" r="{r}" fill="{t["accent"] if hl else t["other"]}" stroke="{t["surface"]}" stroke-width="2"/>')
            dx, dy, anchor = PLACE[corpus][k]
            label = f"{TOOLS[k]} {fmt_mib(y)}" if hl else TOOLS[k]
            body.append(text(sx(x) + dx, sy(y) + dy, label, 12 if hl else 11, "ink" if hl else "ink2", 650 if hl else 400, anchor, t=t))
    body.append(text(pad, h - 18, "Create time includes MRAR's built-in verification pass. 7-Zip and tar rows are their own CLI timings.", 12, "muted", t=t))
    desc = "; ".join(
        f"{CORPORA[c]}: " + ", ".join(
            f"{TOOLS[k]} {d[(c, k)]['create_s']:.2f} s, {d[(c, k)]['archive_bytes'] / MIB:.2f} MiB" for k in TOOLS
        )
        for c in PLACE
    )
    return svg(w, h, t, body, "Archive size against create time", desc)


def main():
    data = load()
    OUT.mkdir(parents=True, exist_ok=True)
    for name, fn in (("headline", headline), ("size", size_chart), ("tradeoff", tradeoff_chart)):
        for mode, theme in THEMES.items():
            path = OUT / f"{name}-{mode}.svg"
            path.write_text(fn(data, theme), encoding="utf-8", newline="\n")
            print(path.relative_to(ROOT))


if __name__ == "__main__":
    main()
