# Third-party acknowledgments

MaxRAR is an original application. The project has not yet applied its proposed Apache-2.0 license; no top-level application license is implied by these dependency notices. Dependencies and assets remain under their own licenses.

## Microsoft Fluent System Icons

Toolbar and interface icon assets are derived from Microsoft Fluent System Icons, revision `a347af95478f989038cccbf085978865264c048a`, under the MIT license. Upstream: https://github.com/microsoft/fluentui-system-icons . Preserve `design/icons/fluent/LICENSE` and `design/icons/fluent/NOTICE` with redistributed assets. The production asset manifest records individual paths and modifications.

## Rust and desktop dependencies

Cargo.lock records exact selected versions; `cargo metadata --locked --format-version 1` records their declared licenses and upstream URLs. Major components include Tauri and its official dialog/opener plugins (MIT OR Apache-2.0), Serde/serde_json (MIT OR Apache-2.0), clap (MIT OR Apache-2.0), uuid (MIT OR Apache-2.0), ZIP (MIT), sevenz-rust2 (Apache-2.0), fs2 (MIT OR Apache-2.0), unicode-normalization (MIT OR Apache-2.0), tempfile (MIT OR Apache-2.0), ctrlc (MIT OR Apache-2.0). Check the complete resolved dependency metadata before redistribution; this list is not a substitute for their license texts.

Frontend dependency versions and notices are recorded by package-lock.json and their installed package license files. The generated concept image is a reference, not a source of production toolbar assets. MaxRAR's application icon is original geometric artwork.

Native platform WebView libraries and system codecs retain their platform licenses. No competitor logos are included. Bundled archive engines retain their own licenses as described below.

## Embedded scanning

YARA-X 1.21.0 is used as a local Rust pattern-matching dependency under BSD-3-Clause. Its exact upstream license and source revision are preserved in `licenses/yara-x/`. YARA-X is developed by VirusTotal; the VirusTotal service/API is not integrated. Its Wasmtime/Cranelift runtime and other transitive dependencies retain their own licenses; resolved metadata is in Cargo.lock.

The five baseline rules were authored for MaxRAR, not copied from a third-party rule collection. Their provenance and scope are recorded in `crates/wonrar-scan/rules/NOTICE.md`. Original application/rule licensing remains pending.

New archive dependencies include tar (MIT OR Apache-2.0), flate2 (MIT OR Apache-2.0), bzip2 (MIT OR Apache-2.0), libbz2-rs-sys (bzip2-1.0.6), lzma-rust2 (Apache-2.0), zstd (MIT), and zstd-safe/native zstd-sys (BSD-3-Clause). sysinfo is MIT; libc is MIT OR Apache-2.0. Wasmtime/Cranelift components declare Apache-2.0 WITH LLVM-exception. This inventory is not a substitute for complete transitive license collection and redistribution review before publication.

## Desktop updates

Tauri plugin updater 2.14.0 (Apache-2.0 OR MIT) implements the update client, with minisign-verify 0.3.0 (MIT) for package signatures. Upstream sources: https://github.com/tauri-apps/plugins-workspace and https://github.com/jedisct1/rust-minisign-verify . Their license texts are preserved under licenses/updater/ and licenses/minisign-verify/. Exact versions and transitive dependency licenses are recorded by Cargo.lock and Cargo metadata. No application license is adopted by adding these notices.

Inspector promotions use reqwest 0.13.5 (MIT OR Apache-2.0), rustls 0.23.46 (Apache-2.0 OR ISC OR MIT), and minisign-verify 0.3.0 (MIT). License files are preserved under `licenses/reqwest/`, `licenses/rustls/` and `licenses/minisign-verify/`. The signed feed carries plain text and approved links; no remote advertising SDK is included.

## RAR decoding

`unrar-ng-sys` 0.7.7 bundles the native UnRAR decoder by Alexander Roshal. The Rust
bindings are MIT OR Apache-2.0; the native source has the separate UnRAR license,
which permits handling and redistributing RAR decoding but restricts recreating
RAR compression. The complete native license and wrapper license texts are
preserved in `licenses/unrar/` and included in desktop bundles. No RAR writer is
implemented. Upstream: https://github.com/ttys3/unrar.rs .

Benign compatibility fixtures originate from that project's test data and from
libarchive's RAR5 test corpus. Provenance, revisions and checksums are recorded
in `crates/wonrar-core/tests/fixtures/rar/README.md`. Libarchive fixture attribution
is preserved in `licenses/unrar/LIBARCHIVE-FIXTURES.txt`.
