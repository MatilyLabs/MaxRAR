# MRAR/2 experimental solid format

October 31, 2026. Experimental, not stable interchange or backup support. Keep
originals. New writers emit /2. Readers accept /0, /1 and /2. Older MaxRAR
builds reject /2 as an unsupported version. The [MRAR/1](MRAR-1-SPEC.md) and
[MRAR/0](MRAR-0-SPEC.md) specifications remain valid for existing archives.

## Container

Unchanged from /1: 16-byte little-endian header (`MRAR\r\n\x1a\n` magic, u16
version **2**, u16 flags **2**, u32 encoded-index length), the encoded index
(u32 decoded length + exactly one Zstandard frame, both capped at 8 MiB, window
capped at 8 MiB), the data section, and a 32-byte SHA-256 footer over all
preceding bytes. Only version/flags pairs `(0,0)`, `(1,1)` and `(2,2)` are accepted.

## Solid stream and units

All non-empty files are concatenated in entry order into one *solid stream*.
The stream is cut into **units** of at most 16 MiB (the writer uses exactly
16 MiB except for the last). Units are compressed independently and stored
back to back in the data section, so unit *k* starts where unit *k-1* ends.
A file can span several units; a unit can hold many files.

Each unit has one codec:

| Id | Codec | Parameters | Decoder bound |
| --- | --- | --- | --- |
| 0 | store | none | encoded = decoded |
| 1 | LZMA2 (raw stream, end marker) | `dict_log` 12–24 | dictionary ≤ 16 MiB |
| 2 | PPMd variant H (7-Zip PPMd7), no end marker | `order` 2–16, `mem_log` 11–26 | model ≤ 64 MiB |

Encoded size is positive and never larger than the decoded size. The decoder
reads at most decoded size + 1 bytes and rejects any other length; LZMA2 units
must consume their encoded bytes exactly. Every unit carries the SHA-256 of its
decoded bytes, which is verified before any of its bytes are returned.

## Binary index

Numbers are unsigned LEB128, at most 10 bytes, minimal encoding only.

```text
unit_count
  per unit: codec u8 [, dict_log u8 | , order u8, mem_log u8]
            encoded_size, decoded_size, sha256 [32 bytes]
entry_count
  per entry: kind u8 (0 file, 1 directory)
             shared_prefix_len, suffix_len, suffix bytes (UTF-8)
             file only: size
```

A path is the previous entry's first `shared_prefix_len` bytes followed by the
suffix. A file's position in the stream is the sum of the sizes of the files
before it, so no offsets are stored. The index must end exactly after the last
entry.

Validation (all before any payload is returned): unit sizes and codec
parameters within the bounds above; unit encoded sizes cover the data section
exactly; file sizes cover the decoded units exactly; total size, entry count
and per-file size within MaxRAR's default limits; paths canonical, portable,
at most 65,535 bytes and 256 components, with no duplicates (including
case-folded or Unicode-normalized duplicates).

## Integrity

There are no per-file hashes in /2. Every byte of every file lies in exactly
one unit, and every unit's SHA-256 is checked on decode, so a file read from a
verified unit is verified. Test decodes and checks every unit. The footer
checks the whole container for accidental change. None of these hashes
authenticate the sender or detect malware.

## Writer policy

The writer walks sources in the same deterministic order as /1. Codec choice
depends only on a unit's bytes, so archives are byte-identical for any thread
count:

1. 64 probes of 4 KiB at golden-ratio positions are compressed with zstd level 1.
2. If every probe is incompressible, a zstd level-1 pass with a 16 MiB window
   over the whole unit decides between store and LZMA2 (it catches duplicate files).
3. If some probes are incompressible, LZMA2 (PPMd is slow on random data).
4. Otherwise eight 64 KiB slices are compressed with PPMd (order 6, 64 MiB)
   and LZMA2 (preset 6). PPMd wins if at least 10 % smaller; LZMA2 wins if not
   larger; otherwise both compress the whole unit and the smaller is kept.
5. Store is used whenever a codec would not shrink the unit.

The index is compressed with zstd level 19.

Work runs on up to 12 threads, also limited to a quarter of available memory
at about 240 MiB per thread. Readers decode units in parallel for Test and
Extract with the same limit.

## Reader policy

Reading one file decodes every unit it overlaps, up to 16 MiB each. Previews
cache one decoded unit. Extraction writes the selected files in stream order and
decodes each needed unit once, ahead of the writer on worker threads.

No encryption, editing, comments, deduplication, metadata restoration,
recovery or split volumes. The LZMA2 and PPMd implementations are the
existing `lzma-rust2` (Apache-2.0) and `ppmd-rust` (CC0-1.0 OR MIT-0) crates,
already used for 7z support.
