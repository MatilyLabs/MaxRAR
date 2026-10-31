# MRAR/2 research and proof plan

October 31, 2026 · Status: plan, nothing implemented · Owner: MaxRAR core

**Goal.** Make MRAR compress harder *and* faster, then show with a reproducible,
public benchmark that MRAR sits on the best ratio/speed trade-off curve
against 7-Zip, RAR, zstd, xz and others.

**What we will claim, and how.** No archiver is the best on every input. The
claim we aim to earn is narrower and defensible:

> On corpora X, Y, Z, on machines A and B with N threads, MRAR presets are on the
> Pareto frontier for archive size vs. create time and vs. extract time, against
> these exact tool versions and settings. Raw data and scripts are published.

Where MRAR loses, the report says so.

## 1. Where MRAR/1 stands

From `docs/benchmarks/MRAR-COMPARISON.md` (1 GiB synthetic text, one Intel Mac, one trial):

| Format | Size | Create |
| --- | ---: | ---: |
| MRAR/1 (zstd L3, 1 thread) | 27.6 MiB | 22.1 s |
| 7z (LZMA2, non-solid) | 26.3 MiB | 302.2 s |
| TAR.ZST L3 | 30.4 MiB | 3.8 s |

Public reference points on Silesia (211 MB), from the research:

| Tool | Size | Time | Source |
| --- | ---: | ---: | --- |
| zstd -19 -T16 | 52.8 MB | 11.1 s | Kanzi README (Ryzen 9950X) |
| xz -9 | 48.8 MB | 43.6 s | Kanzi README |
| 7-Zip -mx=9 | 48.8 MB | n/a | Mahoney's Silesia table |
| bsc -T16 | 47.9 MB | 1.2 s | Kanzi README |
| Kanzi -l 7 | 47.3 MB | 1.2 s | Kanzi README |
| zpaq -m7 | 39.0 MB | very slow | Mahoney's Silesia table |

These are vendor or third-party numbers on other hardware. They show the bar,
not a result. **Kanzi and bsc are the real competition for "fast and heavy",
not 7-Zip.**

## 2. Diagnosis (verified in `crates/wonrar-core/src/mrar.rs`)

Speed:

- **The writer does the work several times over.** After compressing, it copies
  the temp data into the stage file, reads the whole stage again to hash it,
  then runs a full self-test that decodes everything (`create`, around
  lines 945–966). The saved data shows `test_seconds` = 11.1 of the 22.1 s
  create time.
- **One thread.** Blocks and groups are independent already but are compressed
  one after another.
- **SHA-256 runs three times** (block, file, whole container) and again on every
  open.
- **Incompressible data still goes through full LZ matching**; store fallback is
  decided only after compression.

Ratio:

- **1 MiB independent blocks with a 1 MiB window** for large files. Every block
  starts cold, so repetition across blocks is lost.
- **Files between 256 KiB and 1 MiB** get their own cold frame.
- **No reordering by type, no dedup, no dictionaries.**
- **The JSON index stores three 64-character hex hashes per file.** On the
  4,096-small-file test that is most of the 0.195 MiB archive (TAR.ZST: 0.075 MiB).
  It also caps archives near 30–40k files.

## 3. What the research says about ML and "innovative" approaches

- **Neural compressors (NNCP, cmix, ts_zip, LLM-based)** reach record ratios but
  run at roughly kB/s to 100 kB/s. A 1 GiB archive would take hours. They also
  need bit-identical floating point on encoder and decoder, which is unsafe for
  an archive format. **Not viable as the compressor.**
- **Where ML does pay off: choosing the method.** A tiny model, trained offline
  and compiled in as constants, looks at cheap features of each chunk (entropy,
  text fraction, how well a quick zstd-1 sample compresses) and picks *store*,
  fast zstd, strong zstd, LZMA2 or PPMd. It is deterministic because it only
  reads input bytes. The simple "sample-compress" rule gets most of the
  store-skip benefit; the learned part matters in max mode, where a wrong codec
  costs 10× time.
- **Meta OpenZL (format-aware compression, 2025)**: strong on structured data,
  but its format and API are still changing. Watch, don't build on it yet.
- **Proven big levers**: parallel chunks, larger windows with chaining, sorting
  files by type before solid grouping, dedup, trained dictionaries for small
  files, BCJ/delta filters for executables and tables, PPMd for text.

## 4. MRAR/2 design

Container: header → data units → dictionary units → compact binary index →
64-byte trailer (index offset, length, hash; archive hash). A *unit* is a group
of small files or one block of a large file. Each unit declares codec, optional
dictionary, optional filters, window/model memory and decoded size, all capped
and checked before the decoder allocates anything.

Write pipeline: scan → sort/cluster by file family → whole-file dedup →
dictionary training → plan units → predictor picks codec per unit → parallel
workers → in-order writer → parallel verify → trailer.

Output is byte-identical for any thread count, because units are independent
and their parameters are fixed before compression.

### Presets and targets

| Preset | Method | Target |
| --- | --- | --- |
| **Fast** | zstd 1–3, 4 MiB blocks, store-skip, family sort, dedup | TAR.ZST size, faster than tar.zst on 8 cores, still random access |
| **Balanced** (default) | zstd 9–12 with block chaining, trained dictionaries, clustering | Within 0–4 % of 7z size, 30–50× faster than 7z |
| **Max** | predictor picks zstd-19 / LZMA2 / PPMd per unit, BCJ/delta filters | 3–8 % smaller than 7z/RAR on text, equal on binary, 5–10× faster |

These targets are estimates to be confirmed or cut by Phase 0 measurements.

## 5. Phases

Each phase ships on its own and must **move the frontier on at least two
corpora without making any corpus worse by more than 1 %**, or it is dropped.

| Phase | Work | Proof experiment | Effort |
| --- | --- | --- | --- |
| **0. Harness** | Benchmark driver: real corpora, competitors, 5 trials, peak memory, random-access latency, raw CSV | Reproduces the MRAR/1 numbers within noise | M |
| **1. Pipeline** | Parallel units (rayon), trailer index, binary index, BLAKE3 unit hashes (SHA-256 per file kept), single-pass writer, parallel verify | Same archive bytes with 1/4/16 threads; 1 GiB create ≤ 5 s, extract ≤ 3 s, unchanged size | M |
| **2. Free ratio** | Whole-file dedup, family sort, sample-based store-skip | Dedup hit rate on real trees; random 256 MiB create time | S |
| **3. Windows** | 4 MiB blocks, chain to previous block, LDM in balanced | Ratio vs single-file seek time on logs and a VM slice | M |
| **4. Dictionaries** | Trained zstd dictionaries per file family, 1–2 MiB groups | 4,096-file test beats TAR.ZST; < 2 MiB decoded per single-file read | M |
| **5. Max mode** | LZMA2 + PPMd per unit, learned codec picker, BCJ/delta filters | Picker regret < 1 % vs brute-force best; fuzz every new codec/filter | M–L |
| **6. Only if data demands** | MinHash clustering, FastCDC chunk dedup, table transforms | Must close a measured gap > 5 % | L |

Phases 1 and 2 need no new compression science and should roughly quadruple
speed on their own. They come first.

## 6. Benchmark suite ("the proof")

**Corpora** (pin and publish SHA-256 of each):

1. Silesia (`silesia.tar`), the standard, comparable with lzbench and Kanzi
2. enwik8 (text reference)
3. Linux kernel source tree, uncompressed (source code)
4. A pinned `node_modules` tree (many small files)
5. A redistributable office documents / PDF / image set
6. Logs and CSV/JSON datasets
7. A VM or disk image slice
8. Already-compressed media (JPEG, MP4, ZIP)
9. The existing synthetic MRAR/1 set, for continuity

**Competitors** (current versions, `--version` recorded):

- `7z a -t7z -mx=5` and `-mx=9 -ms=on -mmt=N`
- `rar a -m5 -ma5 -s -mt<N>` (licence permitting)
- `zstd -3 -T0`, `zstd -19 -T0`, `zstd --ultra -22 --long=27 -T0`
- `xz -9 -T0`, `brotli -q 11`
- `kanzi -l 6/7`, `bsc`, `zpaq -m5` as reference points

Several levels per tool, so each tool's curve is real rather than one point.

**Metrics per run**: archive size, create wall time including I/O, extract wall
time, peak memory for both, single-file extraction latency, SHA-256 round-trip
check. Thread scaling at 1, 4 and all cores. Cold and warm cache reported separately.

**Trials**: 1 discarded warm-up plus 5 measured; report median and range.
At least two machines (Windows x64 desktop and the Intel Mac; add Apple Silicon
if available). Fixed power plan; note turbo/governor.

**Publishing**: Pareto charts per corpus, full tables, raw per-trial CSV,
scripts and corpus hashes in `docs/benchmarks/`. GitHub Actions is used only
for correctness and relative regression checks, never for published numbers
(runner hardware varies).

**Mistakes to avoid**: competitors at defaults only, 1 thread vs many, old
versions, in-memory codec timing vs whole-tool timing, synthetic-only data,
single trials, leaving out memory and decompression.

## 7. Risks

- **Format churn.** MRAR/2 is a new version; readers must keep /0 and /1.
- **Security.** Every new codec, filter and dictionary is untrusted input:
  caps before allocation, fuzzing in CI.
- **Random access vs ratio.** Chaining and bigger groups slow single-file
  extraction; the benchmark measures it so the trade-off is visible.
- **Licences.** Only permissive dependencies (rayon, blake3, postcard, zstd,
  lzma-rust2, ppmd-rust: verify each licence before adding).
- **Overclaiming.** Publish losses as well as wins.

## Sources

Research done October 31, 2026. Numbers marked as third-party were not reproduced by us.

- lzbench: https://github.com/inikep/lzbench
- Kanzi: https://github.com/flanglet/kanzi-cpp
- Silesia: https://mattmahoney.net/dc/silesia.html
- Large Text Compression Benchmark: https://mattmahoney.net/dc/text.html
- NNCP: https://bellard.org/nncp/
- OpenZL paper: https://arxiv.org/abs/2510.03203
- zstd manual: https://github.com/facebook/zstd/blob/dev/programs/zstd.1.md
- Btrfs compression heuristics: https://btrfs.readthedocs.io/en/latest/Compression.html
- GitHub-hosted runner hardware: https://docs.github.com/en/actions/using-github-hosted-runners/using-github-hosted-runners/about-github-hosted-runners
