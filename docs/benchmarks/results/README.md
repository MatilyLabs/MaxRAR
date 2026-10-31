# Benchmark results

Raw output of `scripts/benchmark/mrar_bench.py`. Each run has a per-trial CSV, a
JSON with environment, tool versions and medians, and a Markdown table.

## October 31, 2026 · Windows 11, Intel Core i7-14700F (28 threads), 32 GB

Corpora from `scripts/benchmark/make_corpora.py` (scale 1.0, hashes in
`2026-10-31-corpora-manifest.json`) plus this repository's `node_modules`
(1,550 files, 75 MB). 1 warm-up + 3 measured trials, medians shown. Warm cache.

- `*-baseline.*`: all tools, MRAR built from `main` (single-threaded writer).
  **The node_modules rows say FAIL because of a harness bug, not an archiver
  bug**: the source tree sat under a path longer than 260 characters and the
  harness skipped 182 source files when hashing. Every tool stored all files;
  the harness now uses extended-length paths. Sizes and times in those rows are valid.
- `*-mrar-baseline.*` and `*-mrar-phase1.*`: MRAR only, before and after the
  Phase 1 parallel writer and parallel verify, run back to back.

### Phase 1 result

Archives are byte-identical before and after (same SHA-256 on text, small-files
and mixed), so MRAR/1 compatibility is unchanged.

| Corpus | Create before | Create after | Speed-up | Create memory before → after |
| --- | ---: | ---: | ---: | ---: |
| text (256 MiB) | 1.57 s | 0.46 s | 3.4× | 15 → 33 MiB |
| mixed (256 MiB) | 1.48 s | 0.70 s | 2.1× | 15 → 32 MiB |
| random (128 MiB) | 0.71 s | 0.46 s | 1.5× | 12 → 29 MiB |
| small-files (64 MiB) | 0.47 s | 0.37 s | 1.3× | 43 → 79 MiB |
| node_modules (71 MiB) | 0.64 s | 0.29 s | 2.2× | 21 → 38 MiB |

`maxrar-cli test` on the text archive: 0.61 s → 0.17 s (3.6×).

Extraction is unchanged. Small-file extraction (6.4 s for 4,096 files vs 1.7 s
for 7-Zip) is dominated by the per-file `sync_all` in `write_planned`
(`crates/wonrar-core/src/lib.rs`), shared by every format.

### Where MRAR stands (ratio)

| Corpus | MRAR/1 | tar+zstd -3 | tar+zstd -19 | 7z -mx9 | tar+xz -9 |
| --- | ---: | ---: | ---: | ---: | ---: |
| text | 55.4 MiB | 56.4 | 38.1 | 35.9 | 35.8 |
| small-files | 0.94 MiB | 0.70 | 0.44 | 0.46 | 0.46 |
| mixed | 155.7 MiB | 156.0 | 147.7 | 146.4 | 146.8 |
| node_modules | 21.4 MiB | 20.4 | 15.7 | 13.8 | 14.0 |

MRAR/1 now writes text at 0.46 s (tar+zstd -3 with 28 threads: 0.26 s; 7-Zip -mx9: 138 s), but its ratio is zstd-3
class. The ratio gap (up to 35 % on text) is the work of the next phases.

## MRAR/2 (solid units, codec chosen per unit)

`*-mrar-v2.*` (all corpora) and `*-mrar-v2-random.*` (random corpus re-run on
its own: the first run followed the mixed corpus and its times were inflated by
disk activity; sizes were identical). Other tools from `*-baseline.*`, same
machine, same session. Size in MiB; create and extract in seconds (median of 3).

| Corpus | MRAR/2 | 7z -mx9 | tar+xz -9 | tar+zstd -19 | 7z -mx5 |
| --- | --- | --- | --- | --- | --- |
| text (256 MiB) | **28.84** · 3.6 · 1.7 | 35.94 · 138 · 1.5 | 35.82 · 133 · 1.2 | 38.05 · 32 · 0.3 | 39.06 · 48 · 0.8 |
| small-files (64 MiB) | **0.421** · 2.3 · 6.2 | 0.457 · 5.4 · 1.7 | 0.457 · 5.1 · 1.2 | 0.442 · 3.3 · 1.0 | 0.461 · 4.3 · 1.7 |
| mixed (256 MiB) | 146.56 · 15.7 · 0.6 | **146.43** · 72 · 1.0 | 146.80 · 120 · 0.8 | 147.74 · 18 · 0.3 | 147.83 · 23 · 0.6 |
| random (128 MiB) | **128.000** · 0.4 · 0.9 | 128.008 · 11 · 0.2 | 128.007 · 65 · 0.3 | 128.003 · 8 · 0.2 | 128.008 · 9 · 0.2 |
| node_modules (71 MiB) | 14.67 · 5.6 · 3.2 | **13.83** · 10.3 · 1.5 | 13.98 · 19 · 0.6 | 15.69 · 8.4 · 0.5 | 14.05 · 9.0 · 1.5 |

What this shows, on this machine and these corpora:

- **Text: 20 % smaller than 7-Zip -mx9 and xz -9, and about 37× faster to create.** PPMd is chosen for every unit.
- **Small related files: smallest of all** (5 % below zstd -19, 8 % below 7-Zip), mostly from the compact index (MRAR/1's index alone was 19 % of its archive).
- **Mixed text/random: within 0.1 % of 7-Zip -mx9, 4.6× faster to create, faster to extract.**
- **node_modules: 6 % larger than 7-Zip -mx9.** 7-Zip's solid LZMA with a 64 MiB dictionary and its encoder still win on many similar source files.
- **Extraction is slower than 7-Zip on many small files** (per-file `sync_all`, shared by every format) and on node_modules.
- Peak create memory reaches about 2.5 GiB on mixed data with 12 threads (7-Zip -mx9: 2.6 GiB). Thread count is capped by available memory; archive bytes do not depend on it.

Single machine, warm cache, synthetic corpora plus one real tree. Not yet run:
Silesia, enwik, Linux kernel source, a second machine, cold cache.
