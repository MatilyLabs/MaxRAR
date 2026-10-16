# MRAR/1 experimental grouped format

October 15, 2026. This is an experimental revision, not stable interchange or
backup support. Keep originals. New writers emit /1; updated readers accept /0
and /1. Older MaxRAR builds reject /1 as an unsupported version. The existing
[MRAR/0 specification](MRAR-0-SPEC.md) and archived /0 measurements remain valid.

## Header and index

The 16-byte little-endian header keeps the `MRAR\r\n\x1a\n` magic, followed by
u16 version **1**, u16 flags **1**, and u32 encoded-index length. Only the
version/flags pairs `(0,0)` and `(1,1)` are accepted. Unknown values fail.

The encoded index is a u32 decoded JSON byte length followed by exactly one
standard Zstandard frame. Both encoded section length and decoded JSON length
are positive and capped at 8 MiB; the encoded length includes the four-byte
decoded length prefix. The decoder caps the index window at 8 MiB, reads at most
declared decoded length plus one byte, and rejects wrong lengths, skippable
frames, additional frames and trailing bytes. Index decompression is bounded but
occurs before footer verification; it is not authenticated input.

The decoded index keeps /0 entry/block fields and changes `version` to 1. It
adds an optional `groups` array (absent means empty) and an optional file `group`
object (`null`/absent means independent blocks). Unknown/duplicate fields fail.
Directory and empty-file rules are unchanged; neither may reference a group.
All existing path, file, total and entry-count limits still apply.

```json
{
  "version": 1,
  "entries": [
    {"path":"a.txt","kind":"file","size":3,"sha256":"<SHA-256 of abc>","blocks":[],"group":{"id":0,"offset":0}},
    {"path":"b.txt","kind":"file","size":3,"sha256":"<SHA-256 of def>","blocks":[],"group":{"id":0,"offset":3}}
  ],
  "groups": [
    {"codec":"store","offset":0,"encoded_size":6,"decoded_size":6,"sha256":"<SHA-256 of abcdef>"}
  ]
}
```

Hash placeholders above explain the layout; this is not a runnable fixture.

## Groups and independent blocks

A group is a block object with the same fields as /0 blocks. Its decoded size is
at most **8 MiB** and its Zstandard decoder window is capped at 8 MiB. Codec is
`store` or `zstd`; encoded length must be positive and no greater than decoded
length; store requires equality. Each group has a decoded SHA-256. There may be
no more groups than the default entry-count limit.

A grouped file has empty `blocks`, positive size at most **256 KiB**, and a
`group` containing the zero-based group `id` and decoded-byte `offset`. Checked
offset-plus-size must stay inside the group. Group slices must cover their
decoded group exactly, without overlaps, gaps or unused groups. Group IDs in
entry order are nondecreasing, avoiding repeated full-group decoding during a
normal sequential test. No recursive/reference chains or dedup references exist.

Files without `group` use /0's independently decoded blocks (up to 1 MiB, with
1 MiB Zstandard windows). Combined encoded group/block ranges must cover the
data section exactly, without gaps or overlaps. /1 permits their ranges to be
interleaved in the file; /0 retains its original entry/block order requirement.

The final 32 bytes remain SHA-256 over all preceding bytes, including the fixed
header and encoded index. It detects accidental changes, not sender identity or
malware. Inventory verifies the index and footer. Test verifies all decoded
groups, blocks and files. Extraction verifies selected file hashes before
publishing each file through the existing scoped staging writer. Previewing a
prefix does not verify the unread suffix's file hash.

## Writer and reader policy

The writer traverses sources in deterministic input/walk order, collecting
nonempty files up to 256 KiB into consecutive groups of at most 8 MiB. It flushes
a pending group before an independent file. Files are not reordered by type.
Empty files/directories retain explicit index records. File sizes are checked
again while reading. Link/path rules, private staging, cancellation checks,
full self-test, and no-clobber publication remain in force.

The writer retains **Zstandard level 3**, with stored fallback if compression
would expand a block/group. It reuses encoder contexts. Payload experiments did
not justify a higher default level's extra time. The JSON index is compressed
at level 3; this reduces metadata overhead without inventing a lossy index.

The reader caches **one decoded group** per open archive. A selected file can
require decoding/hashing up to 8 MiB of other group content. Independent large
files do not require decoding other files. The cache bound is not a process-wide
memory guarantee; index, codec context, encoded input and output buffers also
consume memory. Inventory's per-file compressed size for a group member is a
proportional payload estimate, excluding index overhead; it is not separately
extractable compressed data and estimates may round to zero.

The scanner uses an identity-bound `EntryInspectionSession` for each container,
reusing that verified group across successive files instead of reopening and
decoding it for every match operation. Nested containers have their own session;
existing scan byte/count/depth/time limits and end-of-scan identity checks remain.

No encryption, edit/comment, deduplication, metadata restoration, recovery or
split volumes have been added. These are original container changes using the
existing bundled Zstandard dependency, with no new user installation required.
