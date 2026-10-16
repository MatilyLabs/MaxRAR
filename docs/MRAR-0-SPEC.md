# MRAR/0 experimental wire format

Legacy format reference: the current writer emits experimental
[MRAR/1](MRAR-1-SPEC.md); the current reader still supports /0. The initial writer
policy and measurements below describe the original /0 implementation.

Implementation dated October 15, 2026. This format is experimental; keep
original files. It is not a stable interchange or backup format. Its container
code is original Rust; compression uses Zstandard, not a new compression algorithm.

## Try it

In MaxRAR choose **Add → All formats → MRAR (Experimental)**. The CLI uses the
same implementation:

```sh
maxrar-cli create --output experiment.mrar ./source-folder --json
maxrar-cli test experiment.mrar --json
maxrar-cli extract experiment.mrar --destination ./unpacked --json
```

Use new output paths: existing archives are not overwritten. Create a disposable archive from your own test files with the current writer.

## Layout

All integers in the fixed header are unsigned little endian.

| Byte offset | Size | Value |
| --- | ---: | --- |
| 0 | 8 | `4d 52 41 52 0d 0a 1a 0a` (`MRAR\r\n\x1a\n`) |
| 8 | 2 | Version `0` |
| 10 | 2 | Flags `0` |
| 12 | 4 | UTF-8 JSON index length, at most 8 MiB |
| 16 | index length | Index described below |
| following index | variable | Consecutive independently encoded blocks |
| last 32 bytes | 32 | Raw SHA-256 digest of every preceding byte |

Unknown versions/flags are errors. The footer is an integrity check, not sender
authentication. Detection uses magic bytes, including for extensionless files.

## Required JSON fields

Example for an empty file:

```json
{"version":0,"entries":[{"path":"notes.txt","kind":"file","size":0,"sha256":"e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855","blocks":[]}]}
```

All fields are required. Unknown fields, duplicate fields and incorrect types
are errors. The index version must match the header. Entry order defines entry
IDs (`"0"`, `"1"`, etc.). JSON whitespace/key order do not affect decoding.

| Entry field | Meaning |
| --- | --- |
| `path` | Portable UTF-8 relative path using `/`; no link targets |
| `kind` | `file` or `directory` |
| `size` | Decoded file bytes; zero for directories |
| `sha256` | Lowercase 64-character SHA-256 of file bytes; empty string for directories |
| `blocks` | Ordered array of block objects; empty for directories/empty files |

| Block field | Meaning |
| --- | --- |
| `codec` | `store` or `zstd` |
| `offset` | Unsigned byte offset relative to the start of the data section |
| `encoded_size` | Payload length in bytes, positive and no greater than decoded size |
| `decoded_size` | Positive, at most 1,048,576 bytes |
| `sha256` | Lowercase 64-character SHA-256 of decoded block bytes |

Block offsets must be contiguous, starting at zero, in entry/block order. No
gaps, overlaps, unused data, concatenated archives or trailing data are allowed.
The sum of each file's decoded block sizes must equal its declared size. Stored
blocks require equal encoded/decoded lengths. A Zstandard block contains one
complete frame, with no additional frames or trailing bytes. The decoder caps
the Zstandard window at 1 MiB and bounds output before accepting it, then verifies
block/file hashes.

## Writer and reader policies

The initial writer uses independent 1 MiB blocks, Zstandard level 3, and chooses
stored bytes when the compressed candidate is not smaller. Container metadata
still adds overhead. There is no cross-file history, solid compression or dedup.

Default limits are 100,000 entries, 2 GiB per decoded file, 8 GiB total decoded
bytes and an 8 MiB index. Checked arithmetic and block bounds precede decoding.
Paths are capped at 65,535 UTF-8 bytes and 256 components, and follow the shared
extraction policy; normalized case/Unicode collisions
are rejected. Directories have no blocks. Symlinks, devices and sparse entries
are unsupported. New archives use private staging, validation and no-clobber
publication. Extraction uses the shared reviewed plans and scoped writer.

There is no encryption/password, permission/timestamp restoration, append/edit,
comment, split volume, recovery record, dictionary or executable wrapper support.
Archive identity and footer hashing read the whole archive; selected extraction
avoids decompressing unrelated files but does not eliminate this read cost.

## Independent verification

`verification/mrar_interop.py` independently implements the container in Python
and uses a separately installed libzstd for QA. It checks Rust output and creates
archives tested/extracted by the Rust decoder. That QA prerequisite is not a
product dependency; MaxRAR bundles its existing codec dependency.

This specification describes MRAR/0 only. Parser fuzzing, broader hostile
corpora, compatibility testing and format stabilization remain release work.
See the [current format](MRAR-1-SPEC.md).

## Large-file validation

The local large dummy-file benchmark tested a 1 GiB text file, 256 MiB random data, 512 MiB mixed data and 64 MiB across 4096 files. All MRAR integrity/extraction/hash checks passed; damaged and truncated copies were rejected. This is synthetic single-run validation, not a stable-format release or a WinRAR comparison.
