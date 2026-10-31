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
