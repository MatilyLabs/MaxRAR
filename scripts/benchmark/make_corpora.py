"""Generate the deterministic synthetic benchmark corpora.

python scripts/benchmark/make_corpora.py [--out target/bench-corpora] [--scale 1.0]

Scale 1.0 gives: text (256 MiB of unique log/CSV-style records), small-files
(4,096 related 16 KiB files), mixed (256 MiB alternating text and random
1 MiB chunks) and random (128 MiB). Every generator is seeded, so the same
scale always produces the same bytes; a manifest of SHA-256 sums is written.
These are not byte-identical to the October 15 MRAR/1 fixtures.
"""
import argparse
import hashlib
import json
import random
from pathlib import Path

SEED = 20261015
MIB = 1024 * 1024
ACTIONS = ["login", "logout", "upload", "download", "delete", "rename", "share", "sync"]
REGIONS = ["us-east", "us-west", "eu-central", "ap-south", "sa-east"]


def text_chunk(rng: random.Random, size: int, start: int) -> bytes:
    lines, total, n = [], 0, start
    while total < size:
        line = (
            f"2026-10-{1 + n % 28:02d}T{n % 24:02d}:{n % 60:02d}:{(n * 7) % 60:02d}Z,"
            f"id={n},user=u{rng.randrange(50_000):05d},action={rng.choice(ACTIONS)},"
            f"region={rng.choice(REGIONS)},bytes={rng.randrange(1 << 30)},"
            f"latency_ms={rng.random() * 900:.3f},ok={'true' if rng.random() < 0.97 else 'false'}\n"
        )
        lines.append(line)
        total += len(line)
        n += 1
    return "".join(lines).encode()[:size]


def write(path: Path, data: bytes) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(data)


def make(out: Path, scale: float) -> None:
    rng = random.Random(SEED)
    text_size = int(256 * MIB * scale)
    (out / "text").mkdir(parents=True, exist_ok=True)
    with open(out / "text" / "records.csv", "wb") as f:
        done = 0
        while done < text_size:
            chunk = text_chunk(rng, min(8 * MIB, text_size - done), done // 120)
            f.write(chunk)
            done += len(chunk)

    template = text_chunk(random.Random(SEED + 1), 16 * 1024, 0)
    for i in range(int(4096 * scale) or 1):
        body = bytearray(template)
        for _ in range(16):
            pos = rng.randrange(len(body) - 8)
            body[pos : pos + 8] = f"{rng.randrange(10**8):08d}".encode()
        write(out / "small-files" / f"d{i // 256:02d}" / f"record-{i:05d}.csv", bytes(body))

    (out / "mixed").mkdir(parents=True, exist_ok=True)
    with open(out / "mixed" / "mixed.bin", "wb") as f:
        for i in range(int(256 * scale) or 1):
            f.write(text_chunk(rng, MIB, i * 9000) if i % 2 == 0 else rng.randbytes(MIB))

    write(out / "random" / "random.bin", random.Random(SEED + 2).randbytes(int(128 * MIB * scale)))


def manifest(out: Path) -> dict:
    result = {}
    for corpus in sorted(p for p in out.iterdir() if p.is_dir()):
        h, size, files = hashlib.sha256(), 0, 0
        for f in sorted(corpus.rglob("*")):
            if f.is_file():
                h.update(f.relative_to(corpus).as_posix().encode() + b"\0" + f.read_bytes())
                size += f.stat().st_size
                files += 1
        result[corpus.name] = {"bytes": size, "files": files, "sha256": h.hexdigest()}
    return result


def main() -> None:
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    p.add_argument("--out", default="target/bench-corpora")
    p.add_argument("--scale", type=float, default=1.0)
    a = p.parse_args()
    out = Path(a.out)
    out.mkdir(parents=True, exist_ok=True)
    make(out, a.scale)
    m = {"seed": SEED, "scale": a.scale, "corpora": manifest(out)}
    (out / "manifest.json").write_text(json.dumps(m, indent=2))
    print(json.dumps(m, indent=2))


if __name__ == "__main__":
    main()
