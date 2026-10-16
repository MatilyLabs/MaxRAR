# MaxRAR archive core

Shared synchronous Rust API for desktop worker jobs and the JSON CLI. Callers must
run archive operations off their UI thread. Original source is licensed under [Apache License 2.0](../../LICENSE);
dependency licenses remain applicable. See [NOTICE](../../NOTICE) for attribution.

## Implemented formats and operations

| Operation | ZIP | 7z | TAR and TAR compounds | gzip / bzip2 / XZ / Zstandard | RAR |
|---|---|---|---|---|---|
| Inventory and findings | Yes | Yes | Yes | One decoded entry | RAR4 / RAR5, single volume |
| Bounded text/raw-byte inspection | Yes | Yes | Yes | Yes | Yes |
| Plan and selected extraction | Yes | Yes | Yes | Single entry | Yes |
| Decode/available checksum test | Yes | Yes | Headers + compression checksums | Compression checksums when present | Yes |
| Create from files/folders | Deflate | LZMA2, non-solid | Yes | Exactly one regular source file | Unsupported |
| Password contents | AES-256 ZIP | AES-256 7z | Unsupported | Unsupported | Yes; encrypted headers also supported |
| Delete entries / archive comment | Staged rewrite | Unsupported | Unsupported | Unsupported | Unsupported |

### RAR decoder boundary

RAR4/RAR5 single-volume operations use the bundled `unrar-ng-sys` 0.7.7
native UnRAR decoder, not a user-installed executable. Hosts must call
`wonrar_core::rar_worker::maybe_run()` before initializing their runtime or
parsing arguments (as the MaxRAR desktop and CLI do). Inventory, decoded bytes,
Test and extraction run in a separate copy of that host executable.

The parent creates a private archive snapshot (at most 8 GiB compressed), binds
it to the reviewed SHA-256 for extraction, and supervises a 120-second worker
deadline and sampled 768 MiB resident-memory threshold. Sampling may overshoot;
this is process supervision, not a filesystem/network permission sandbox.
Decoding rejects dictionaries over 256 MiB and enforces actual chunk-level
per-file/total output budgets, including preceding solid entries. RAR_TEST sends
chunks to a bounded prefix buffer or numerically named private staging files;
native destination extraction is never invoked. Completed selected files use
the shared capability-relative writer and conflict policy after checksum success.
Selected extraction decodes through the last selected item, not necessarily
the archive tail. Prefix reads report truncation and incomplete checksum coverage.
Snapshotting and staged decoded files require additional temporary disk space.

Multipart volumes, SFX executables, editing, creation, link extraction, and
metadata restoration are not supported. Test fails as incomplete on special
entries rather than claiming complete integrity coverage. Preserve the native
UnRAR license and wrapper notices under `licenses/unrar/` when distributing.

### Experimental MRAR/0 and MRAR/1

`.mrar` has an original indexed container with create, inventory, bounded preview,
integrity test, reviewed selected extraction and raw-byte inspection. Legacy /0 blocks are
independent Zstandard level 3 or stored bytes, at most 1 MiB decoded, with a 1 MiB
Zstandard window cap. SHA-256 covers decoded blocks, each file and the complete
container. New /1 writers compress the index and group nonempty small files
(up to 256 KiB each) into at most 8 MiB shared blocks, retaining independent
blocks for large files. A reader caches one verified group; selected extraction
can decode the whole group. Grouped per-file compressed sizes are proportional
payload estimates. Inventory checks the index and container footer; full file/block
verification requires Test or fully reading the file. Hashes do not authenticate
the sender or establish malware safety.

The strict JSON index is capped at 8 MiB; default file/total/entry limits apply.
Offsets must cover the data section exactly without overlap/gaps; unknown or
duplicate fields, versions, flags, codecs, unsafe/colliding names, and files used
as ancestor directories are rejected. New archives use private payload and output
staging, full test and no-clobber publication. Extraction reuses the scoped writer
and fresh reviewed plans. Up to two encoded payload copies can coexist during
creation; disk use is additional to the source files.

Both versions are experimental. No encryption, edit/comment, deduplication,
metadata restoration, recovery or split volumes are implemented. Keep originals.
No stable format or performance advantage over RAR is claimed. See the
[grouped /1 specification](../../docs/MRAR-1-SPEC.md),
[legacy /0 specification](../../docs/MRAR-0-SPEC.md) and
[independent container check](../../verification/mrar_interop.py).

Container detection uses bytes and validated headers, independent of suffix.
Supported TAR wrappers are `.tar.gz`/`.tgz`, `.tar.bz2`/`.tbz2`, `.tar.xz`/`.txz`
and `.tar.zst`/`.tzst`. Extensionless gzip-compressed TAR and single-file source
downloads are supported. A single gzip payload is not assumed to contain TAR.
For streams, an output name is derived from the source archive filename; untrusted
gzip FNAME is ignored. Extensionless single files use `<filename>.unpacked`.

ZIP-based JAR, OpenDocument, WHL, NPZ and ZIP-based PyTorch files can be browsed as
ZIP containers. This does not validate their application semantics, load tensors,
unpickle objects, start Java, execute notebooks or run any supplied code. Older
non-ZIP checkpoint variants remain unsupported. Brotli has no identifying magic
and is deliberately deferred instead of claiming reliable suffix-free detection.

ZIP filenames and created 7z filenames remain visible when contents are encrypted.
7z encrypted headers can be opened with `inspect_with_password`; a password is
never placed into an extraction plan or progress event. Passwords are borrowed
strings and caller-owned buffers are not guaranteed to be zeroed by this crate.
ZIP's `deflate` and AES features and 7z's LZMA/LZMA2, PPMd, Deflate, AES features
are enabled. Other codec, split-volume, legacy filename, image-preview, executable
metadata, recovery-record and platform-integration compatibility is not promised.

## Extraction contract

- Inventory IDs are entry indices scoped to the archive SHA-256; TAR IDs exclude
  metadata extension records, and single compression streams use ID `0`.
- Plans include selected IDs, content identity, explicit destinations, individual
  conflicts, policy limits and blocked-entry reasons. Empty selection means all.
- Plans are regenerated at execution and compared; changed archives, modified plan
  entries and changed destination conflicts require a new review.
- Files obstructing destination ancestors are visible conflicts: Skip skips those
  entries, Replace blocks them, and Keep both renames the directory consistently
  for its children. Files colliding with archive-required directories are also
  resolved before writing, independent of inventory order.
- Absolute/traversal paths, backslashes, Windows device/stream names, links,
  special files and non-portable path components are blocked. Inventory collision
  comparisons normalize NFC and lowercase. This is not a complete model of every
  filesystem's Unicode equivalence rules.
- Leading `./` member prefixes are normalized for ordinary POSIX TAR archives.
  Interior `.` and `..` components are rejected; original member names remain
  visible in inventory and extraction plans.
- `cap-std` directory handles scope output operations, including parent creation,
  staging and publication. A temporary regular file is completely decoded before
  publication. Skip/Keep both use an atomic hard-link no-clobber commit; Replace
  uses a scoped rename. Filesystems lacking hard-link support fail explicitly.
- Limits default to 100,000 selected entries, 2 GiB per decoded file and 8 GiB
  per operation. Decoded bytes are counted, not solely declared sizes. A bounded
  read chunk and decoder working memory can exceed the output-byte budget.
- 7z solid processing drains preceding/skipped data through the same decoded-byte
  budget. Small selections in large solid archives can therefore be rejected.
- TAR/compressed-stream inventory is sequential and actually decodes contents.
  Limits include skipped payloads; planning rejects limits insufficient for the
  required traversal. There is an additional bounded allowance for TAR header
  bytes. GNU/PAX extension payloads are capped at 1 MiB each; paths at 65,535
  bytes; trailing TAR zero padding at 1 MiB. GNU/PAX sparse variants and
  concatenated TAR archives are rejected. Two zero end blocks are required.
- XZ dictionaries and Zstandard windows are capped at 128 MiB per decoder.
  These are codec limits, not a process-wide RAM limit. TAR itself has header
  checksums, not per-file payload checksums; the test result states that limit.
- Cancellation checks happen between read chunks. Finished extracted files remain
  on failure/cancel; unfinished staging files are removed. Failed jobs currently
  return an error, without a structured partial receipt. Directories may remain.

## Archive publication

New archives are written to a same-filesystem temporary file, tested, synced and
published without replacing an existing archive. ZIP edits raw-copy retained
payloads into a staged archive, check its structure, then atomically replace the
original. Raw copying is not a payload integrity test. Existing data is preserved
if the operation fails before publication.

ZIP edits use a retained hidden sibling `.<archive>.wonrar.lock` advisory lock to
prevent cooperating WonRaR processes using the same archive path from editing
concurrently. Alternate hard-link/symlink aliases are not a shared lock identity. The empty lock
file is deliberately retained to avoid lock-inode races. Other applications do not
necessarily honor this lock; concurrent external modification is unsupported.
Desktop callers use `delete_entries_checked` and `set_comment_checked`, supplying
the identity from the reviewed inventory. The expected identity is compared inside
the mutation lock before staging, preventing stale positional IDs from deleting
different entries after an external repack. Unchecked compatibility wrappers remain
available for callers intentionally operating on the current archive.

## Raw-byte API for embedded inspection

`inspect_entry_bytes(path, entry_id, max_bytes, password, cancel)` returns
`EntryBytes { bytes, truncated }`, preserving binary bytes without UTF-8 conversion.
The returned allocation is capped at 16 MiB regardless of a larger requested cap.
Cancellation is checked between read chunks. A truncated result does not establish
full-payload CRC/authentication coverage. Sequential/solid formats may decode
preceding data under their separate traversal budget; callers need a worker
deadline in addition to a returned-byte cap. No external service or scanner is
called by this API.

Resolved new dependencies: `tar` 0.4.46 and `flate2` 1.1.10 (MIT OR Apache-2.0),
`bzip2` 0.6.1 (MIT OR Apache-2.0) with `libbz2-rs-sys` 0.2.5 (bzip2-1.0.6),
`lzma-rust2` 0.22.0 (Apache-2.0), `zstd` 0.13.3 (MIT) and native `zstd-sys`
2.1.1+zstd.1.5.7 (BSD-3-Clause). Distribution must retain their notices. This
listing does not replace a full transitive dependency license review.

## Remaining release gates

This is a working development core, not a completed security audit. ZIP/7z decoder
metadata allocation happens inside dependencies before our inventory cap can be
checked. TAR extension parsing is explicitly capped before allocation. Parsing,
fingerprinting and some codec initialization cannot currently be interrupted.
These library functions do not themselves enforce an OS memory/CPU sandbox or
terminate processes. Direct native inventory/profile calls can decode up to the
configured 8 GiB payload traversal budget before returning; that byte bound is
not a wall-time or total-process memory bound. Scanner worker supervision belongs
to the caller and does not automatically protect every direct core call.
Truncated previews do not establish full-payload checksum/authentication coverage.
Extraction permissions/timestamps, origin metadata and xattrs are not preserved.
Paths with non-UTF-8 source filenames are rejected. Filesystem/directory metadata
durability after power failure is not guaranteed by syncing the payload alone.

Synthetic tests cover traversal, unsafe platform names, destination symlinks and
concurrent destination-parent swaps,
client-plan tampering, changed archives, conflicts, implicit directory renaming,
Unicode inventory collisions, cancellation cleanup, no-clobber creation,
transactional edits, and password payload roundtrips for ZIP and 7z. Stream tests
cover all implemented codecs, extensionless single/TAR gzip, binary inspection,
corrupt checksums, truncated TAR, skipped data budgets, metadata/padding bombs,
ZIP container aliases, and Python-standard-library-generated GNU/PAX fixtures.
Broad independent tool interoperability, hostile corpora and installed Windows/macOS/Linux
validation remain necessary before distribution claims.
