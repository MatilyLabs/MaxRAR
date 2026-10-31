"""MRAR benchmark harness: archive size, create/extract wall time, peak memory
and a SHA-256 round-trip check for MRAR and competing archivers.

    python scripts/benchmark/mrar_bench.py --corpora target/bench-corpora \
        --cli target/release/maxrar-cli.exe --trials 5 --out docs/benchmarks/results

Each corpus is a folder (every subfolder of --corpora, or --corpus name=path).
Every tool archives the whole folder, extracts it into a fresh directory and the
harness compares SHA-256 of every file against the source. Times are wall clock
including I/O; memory is the peak resident set of the process tree, sampled every
10 ms. One warm-up run per cell is discarded. Tools that are not installed are
skipped and reported. Results: raw per-trial CSV, a JSON with environment and
medians, and a Markdown table.
"""
import argparse
import csv
import hashlib
import json
import os
import platform
import shutil
import statistics
import subprocess
import sys
import tempfile
import time
from dataclasses import dataclass, field
from datetime import datetime, timezone
from pathlib import Path

import psutil

PEAZIP = Path(os.environ.get("ProgramFiles", r"C:\Program Files")) / "PeaZip/res/bin"


def find(name: str, *extra: Path) -> str | None:
    for candidate in extra:
        if candidate.is_file():
            return str(candidate)
    return shutil.which(name)


@dataclass
class Tool:
    id: str
    label: str
    ext: str
    create: list  # argv template: {archive} {src_parent} {src_name} {out}
    extract: list
    env: dict = field(default_factory=dict)
    pipe: bool = False  # create/extract templates are shell pipelines


def tools(cli: str, threads: int) -> tuple[list[Tool], list[str]]:
    sevenz = find("7z", PEAZIP / "7z/7z.exe")
    zstd = find("zstd", PEAZIP / "zstd/zstd.exe")
    xz = shutil.which("xz")
    # Windows' bsdtar understands drive-letter paths; MSYS tar treats "C:" as a host.
    tar = find("tar", Path(os.environ.get("SystemRoot", r"C:\Windows")) / "System32/tar.exe")
    out, missing = [], []

    def add(ok, tool):
        (out.append(tool) if ok else missing.append(tool.id))

    cli = str(Path(cli).resolve())
    mt = {"MAXRAR_THREADS": str(threads)}
    add(Path(cli).is_file(), Tool("mrar", "MRAR (MaxRAR)", "mrar",
        [cli, "create", "--output", "{archive}", "{src}"],
        [cli, "extract", "{archive}", "--destination", "{out}"], env=mt))
    add(sevenz, Tool("7z-mx5", "7-Zip LZMA2 -mx5", "7z",
        [sevenz, "a", "-t7z", "-mx=5", f"-mmt={threads}", "-bd", "-bso0", "{archive}", "{src}"],
        [sevenz, "x", "-bd", "-bso0", "-y", "-o{out}", "{archive}"]))
    add(sevenz, Tool("7z-mx9", "7-Zip LZMA2 -mx9", "7z",
        [sevenz, "a", "-t7z", "-mx=9", f"-mmt={threads}", "-bd", "-bso0", "{archive}", "{src}"],
        [sevenz, "x", "-bd", "-bso0", "-y", "-o{out}", "{archive}"]))
    add(sevenz, Tool("zip-7z", "ZIP Deflate (7-Zip)", "zip",
        [sevenz, "a", "-tzip", "-mx=5", f"-mmt={threads}", "-bd", "-bso0", "{archive}", "{src}"],
        [sevenz, "x", "-bd", "-bso0", "-y", "-o{out}", "{archive}"]))
    for level in (3, 19):
        add(zstd and tar, Tool(f"tar-zstd{level}", f"tar + zstd -{level} -T{threads}", "tar.zst",
            f'"{tar}" -cf - -C "{{src_parent}}" "{{src_name}}" | "{zstd}" -q -{level} -T{threads} -o "{{archive}}"',
            f'"{zstd}" -q -dc "{{archive}}" | "{tar}" -xf - -C "{{out}}"', pipe=True))
    add(xz and tar, Tool("tar-xz9", f"tar + xz -9 -T{threads}", "tar.xz",
        f'"{tar}" -cf - -C "{{src_parent}}" "{{src_name}}" | "{xz}" -9 -T{threads} -c > "{{archive}}"',
        f'"{xz}" -dc "{{archive}}" | "{tar}" -xf - -C "{{out}}"', pipe=True))
    return out, missing


def long_path(path: Path) -> Path:
    """Windows extended-length form, so deep trees beyond 260 characters are not skipped."""
    path = path.resolve()
    if os.name == "nt" and not str(path).startswith("\\\\?\\"):
        return Path("\\\\?\\" + str(path))
    return path


def tree_hashes(root: Path) -> dict[str, str]:
    result = {}
    root = long_path(root)
    for f in sorted(root.rglob("*")):
        if f.is_file():
            h = hashlib.sha256()
            with open(f, "rb") as fh:
                for chunk in iter(lambda: fh.read(1 << 20), b""):
                    h.update(chunk)
            result[f.relative_to(root).as_posix()] = h.hexdigest()
    return result


def run(cmd, env, pipe: bool) -> tuple[float, int]:
    """Run to completion; return wall seconds and peak RSS of the process tree."""
    start = time.perf_counter()
    proc = subprocess.Popen(cmd, shell=pipe, env={**os.environ, **env},
                            stdout=subprocess.DEVNULL, stderr=subprocess.PIPE)
    root, peak = psutil.Process(proc.pid), 0
    while proc.poll() is None:
        try:
            rss = sum(p.memory_info().rss for p in [root, *root.children(recursive=True)])
            peak = max(peak, rss)
        except psutil.Error:
            pass
        time.sleep(0.01)
    elapsed = time.perf_counter() - start
    err = proc.stderr.read().decode(errors="replace")
    if proc.returncode != 0:
        raise RuntimeError(f"exit {proc.returncode}: {err.strip()[:400]}")
    return elapsed, peak


def fill(template, **values):
    if isinstance(template, str):
        return template.format(**values)
    return [part.format(**values) for part in template]


def one_trial(tool: Tool, src: Path, expected: dict, work: Path) -> dict:
    archive = work / f"archive.{tool.ext}"
    out = work / "out"
    out.mkdir()
    values = dict(archive=archive, src=src, src_parent=src.parent, src_name=src.name, out=out)
    create_s, create_rss = run(fill(tool.create, **values), tool.env, tool.pipe)
    size = archive.stat().st_size
    extract_s, extract_rss = run(fill(tool.extract, **values), tool.env, tool.pipe)
    got = out / src.name
    ok = got.is_dir() and tree_hashes(got) == expected
    return dict(archive_bytes=size, create_s=create_s, extract_s=extract_s,
                create_peak_mib=create_rss / 2**20, extract_peak_mib=extract_rss / 2**20,
                roundtrip_ok=ok)


def environment(threads: int) -> dict:
    return {
        "date": datetime.now(timezone.utc).isoformat(timespec="seconds"),
        "os": platform.platform(),
        "cpu": platform.processor(),
        "logical_cpus": psutil.cpu_count(),
        "ram_gib": round(psutil.virtual_memory().total / 2**30, 1),
        "python": sys.version.split()[0],
        "threads": threads,
    }


def version_of(tool: Tool) -> str:
    exe = tool.create[0] if isinstance(tool.create, list) else tool.create.split('"')[3]
    for args in (["--version"], ["i"] if "7z" in exe else []):
        if not args:
            continue
        try:
            text = subprocess.run([exe, *args], capture_output=True, text=True, timeout=20).stdout
            line = next((l.strip() for l in text.splitlines() if l.strip()), "")
            if line:
                return line[:120]
        except (OSError, subprocess.SubprocessError):
            pass
    return "unknown"


def main() -> None:
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    p.add_argument("--corpora", help="folder whose subfolders are corpora")
    p.add_argument("--corpus", action="append", default=[], help="name=path (repeatable)")
    p.add_argument("--cli", default=str(Path(os.environ.get("CARGO_TARGET_DIR", "target")) / "release" / "maxrar-cli.exe"))
    p.add_argument("--tools", help="comma-separated tool ids (default: all found)")
    p.add_argument("--threads", type=int, default=psutil.cpu_count())
    p.add_argument("--trials", type=int, default=5)
    p.add_argument("--no-warmup", action="store_true")
    p.add_argument("--work", default=tempfile.gettempdir())
    p.add_argument("--out", default="docs/benchmarks/results")
    p.add_argument("--label", default="run")
    a = p.parse_args()

    corpora = {}
    if a.corpora:
        corpora |= {d.name: d for d in sorted(Path(a.corpora).iterdir()) if d.is_dir()}
    for spec in a.corpus:
        name, _, path = spec.partition("=")
        corpora[name] = Path(path)
    if not corpora:
        p.error("no corpora given")
    selected, missing = tools(a.cli, a.threads)
    if a.tools:
        wanted = a.tools.split(",")
        selected = [t for t in selected if t.id in wanted]
    print(f"tools: {', '.join(t.id for t in selected)}; not found: {', '.join(missing) or 'none'}")

    rows = []
    for cname, src in corpora.items():
        src = src.resolve()
        expected = tree_hashes(src)
        in_bytes = sum((long_path(src) / k).stat().st_size for k in expected)
        for tool in selected:
            for trial in range(-0 if a.no_warmup else -1, a.trials):
                with tempfile.TemporaryDirectory(prefix="mrb-", dir=a.work) as work:
                    try:
                        r = one_trial(tool, src, expected, Path(work))
                    except (RuntimeError, OSError) as e:
                        print(f"  {cname:12} {tool.id:12} FAILED: {e}")
                        r = dict(error=str(e))
                if trial < 0:
                    continue
                row = dict(corpus=cname, input_bytes=in_bytes, files=len(expected), tool=tool.id,
                           label=tool.label, trial=trial, **r)
                rows.append(row)
                if "error" not in r:
                    print(f"  {cname:12} {tool.id:12} #{trial} {r['archive_bytes'] / 2**20:9.3f} MiB"
                          f" create {r['create_s']:7.2f}s extract {r['extract_s']:7.2f}s"
                          f" mem {r['create_peak_mib']:7.0f} MiB {'ok' if r['roundtrip_ok'] else 'ROUNDTRIP MISMATCH'}")

    summary = []
    for cname in corpora:
        for tool in selected:
            rs = [r for r in rows if r["corpus"] == cname and r["tool"] == tool.id and "error" not in r]
            if not rs:
                continue
            med = lambda k: statistics.median(r[k] for r in rs)
            summary.append(dict(corpus=cname, tool=tool.id, label=tool.label, trials=len(rs),
                                input_bytes=rs[0]["input_bytes"], archive_bytes=rs[0]["archive_bytes"],
                                ratio=rs[0]["archive_bytes"] / rs[0]["input_bytes"],
                                create_s=med("create_s"), create_min=min(r["create_s"] for r in rs),
                                create_max=max(r["create_s"] for r in rs), extract_s=med("extract_s"),
                                create_peak_mib=med("create_peak_mib"), extract_peak_mib=med("extract_peak_mib"),
                                roundtrip_ok=all(r["roundtrip_ok"] for r in rs),
                                deterministic_size=len({r["archive_bytes"] for r in rs}) == 1))

    out = Path(a.out)
    out.mkdir(parents=True, exist_ok=True)
    stem = f"{datetime.now():%Y-%m-%d}-{platform.node().lower()}-{a.label}"
    with open(out / f"{stem}.csv", "w", newline="") as f:
        keys = sorted({k for r in rows for k in r})
        w = csv.DictWriter(f, fieldnames=keys)
        w.writeheader()
        w.writerows(rows)
    report = dict(environment=environment(a.threads), tools={t.id: version_of(t) for t in selected},
                  corpora={n: str(pth) for n, pth in corpora.items()}, summary=summary)
    (out / f"{stem}.json").write_text(json.dumps(report, indent=2))
    lines = ["| Corpus | Tool | Size (MiB) | Ratio | Create (s) | Extract (s) | Create mem (MiB) | Round trip |",
             "| --- | --- | ---: | ---: | ---: | ---: | ---: | --- |"]
    for s in summary:
        lines.append(f"| {s['corpus']} | {s['label']} | {s['archive_bytes'] / 2**20:.3f} | {s['ratio']:.4f} |"
                     f" {s['create_s']:.2f} | {s['extract_s']:.2f} | {s['create_peak_mib']:.0f} |"
                     f" {'ok' if s['roundtrip_ok'] else 'FAIL'} |")
    (out / f"{stem}.md").write_text("\n".join(lines) + "\n")
    print("\n".join(lines))
    print(f"\nwrote {out / stem}.{{csv,json,md}}")


if __name__ == "__main__":
    main()
