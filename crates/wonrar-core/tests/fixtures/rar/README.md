# RAR compatibility fixtures

These small upstream archives are test data, never executables. Passwords are
public fixture values, not credentials.

- `version.rar`, `solid.rar`, `crypted.rar`, `comment-hpw-password.rar`,
  `unicode-entry.rar`, and `archive.part1.rar` originate from
  https://github.com/ttys3/unrar.rs/tree/0971923085c66fa663b1081dd31c5e1150be12cb/data .
  Upstream MIT/Apache-2.0 license texts are preserved under `licenses/unrar/WRAPPER-*`.
- `test_read_format_rar5_*.rar` originate from corresponding `.rar.uu` files in
  https://github.com/libarchive/libarchive/tree/3a359a69787ab52a2a76320e615797108123a889/libarchive/test .
  Only uuencoding was removed. RAR5 test attribution is preserved in
  `licenses/unrar/LIBARCHIVE-FIXTURES.txt`.

`crypted.rar` uses password `unrar`; `comment-hpw-password.rar` uses `password`.
`version.rar` contains `VERSION` with bytes `unrar-0.4.0`.
The multipart fixture demonstrates explicit rejection; the symlink fixture
exercises blocking and incomplete integrity coverage. No files are executed.

Tests also derive corrupt and unsafe-path archives in temporary directories.

## SHA-256

| File | SHA-256 |
| --- | --- |
| `archive.part1.rar` | `b64e082db122fac3616b8309afaaff78f939424767fbf0f097fa178ac9365548` |
| `comment-hpw-password.rar` | `dd47390a7fc73391183b38a4c140b595fdd52ab22afbdfa4e41db5023fc55f1b` |
| `crypted.rar` | `3215288892bfae2e641786952ab2547ea72cdd58c9c544604cd63e242fa07318` |
| `solid.rar` | `23da48de916e4feb7a0f0fbd6ba520dae3492d04e98d061225b0e923668e48f4` |
| `test_read_format_rar5_compressed.rar` | `68f9d7e2662cd5bc5607ba9c6bf1edbf121cf3b08011a7d8643091791750a7c8` |
| `test_read_format_rar5_multiple_files_solid.rar` | `0aa2c652c06304dedd93019663c1d52d5e85055d5fdb8045bfcc4345b90b4a0f` |
| `test_read_format_rar5_symlink.rar` | `a7336968564786ea5ff106fff2f6ef8dfd8ad5ecca86fbd1db3d63ba42bcd29a` |
| `unicode-entry.rar` | `404c872a59c60f5040586065a4f0b30478139a405284154479cc9b57f7bf2815` |
| `version.rar` | `2c595ee9aaea2c837bd0e11a659b9b62b46251f18ab7b6675748da0fadd34460` |
