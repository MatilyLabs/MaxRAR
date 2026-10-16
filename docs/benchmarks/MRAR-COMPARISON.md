# MRAR compression measurements

October 15, 2026 · optimized MaxRAR Rust core · macOS 13.7.8, Intel x86_64 · one trial per case.

This compares MaxRAR’s own archive writers on the same retained synthetic inputs.
MRAR/1 was measured after the ZIP, 7z and TAR.ZST baseline; the timing runs were
not paired. Archive sizes include metadata and checksums. Times include core
inventory, integrity and archive hashing work.

## 1 GiB unique-record text fixture

| Format | Archive size | Create time |
| --- | ---: | ---: |
| **MRAR/1 (Experimental)** | 27.6 MiB | 22.1 s |
| ZIP | 59.4 MiB | 6.0 s |
| 7z | 26.3 MiB | 302.2 s |
| TAR.ZST | 30.4 MiB | 3.8 s |

## Archive sizes across every retained workload

| Synthetic input | MRAR/1 | ZIP | 7z | TAR.ZST |
| --- | ---: | ---: | ---: | ---: |
| Unique-record text (1 GiB) | 27.624 MiB | 59.411 MiB | 26.343 MiB | 30.408 MiB |
| 4,096 related files (64 MiB) | 0.195 MiB | 4.709 MiB | 2.250 MiB | 0.075 MiB |
| Alternating text/random (512 MiB) | 262.883 MiB | 271.621 MiB | 263.777 MiB | 264.149 MiB |
| Seeded pseudorandom (256 MiB) | 256.010 MiB | 256.078 MiB | 256.012 MiB | 256.007 MiB |

All completed cases passed integrity testing, extraction and per-file SHA-256
comparisons in the saved measurement runs. Exact bytes, creation/test/extraction
times and measurement/driver fingerprints are retained in the
[machine-readable results](mrar-comparison-2026-10-15.json).

## Writer settings

- MRAR/1: Zstandard level 3, groups up to 8 MiB for small files, independent
  1 MiB blocks for large files, stored fallback, compressed index, SHA-256 checks.
- ZIP: MaxRAR’s Deflate defaults.
- 7z: MaxRAR’s non-solid LZMA2 defaults.
- TAR.ZST: streaming TAR with Zstandard level 3.

## Inputs and interpretation

Inputs were generated, not user documents: 1 GiB of unique-record text;
4,096 related 16 KiB files; 512 MiB of alternating text and seeded random data;
and 256 MiB of seeded random data. Random generators used seed 20261015.
MiB means 1,048,576 bytes; GiB means 1,073,741,824 bytes.

These are single-trial, warm-cache observations on one machine, without thermal
controls. MRAR/1 and the baseline were recorded in separate runs. Compression
ratio, speed and memory vary by input and settings. The table does not establish
that any format is always smaller or faster. No encryption, scanning or desktop
UI workload was measured. MRAR remains experimental; keep original files.
