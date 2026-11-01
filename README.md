<p align="center">
  <img src="docs/assets/maxrar-hero.gif" alt="MaxRAR archive manager — animated Browse, Inspect and Extract workflow with the MaxRAR logo" width="1200" />
</p>

<p align="center">
  <a href="https://github.com/MatilyLabs/MaxRAR/releases/latest"><img src="https://img.shields.io/github/v/release/MatilyLabs/MaxRAR?style=flat-square&logo=github&label=version&color=3569ad" alt="Latest MaxRAR release version" /></a>
  <a href="https://github.com/MatilyLabs/MaxRAR/releases"><img src="https://img.shields.io/github/downloads/MatilyLabs/MaxRAR/total?style=flat-square&logo=github&label=downloads&color=2ea043" alt="Total MaxRAR downloads" /></a>
  <a href="https://github.com/MatilyLabs/MaxRAR/stargazers"><img src="https://img.shields.io/github/stars/MatilyLabs/MaxRAR?style=flat-square&logo=github&label=stars&color=c9a227" alt="GitHub stars" /></a>
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-Apache--2.0-blue?style=flat-square" alt="Apache License 2.0" /></a>
  <img src="https://img.shields.io/badge/built%20with-Rust-dea584?style=flat-square&logo=rust&logoColor=white" alt="Built with Rust" />
  <img src="https://img.shields.io/badge/UI-Svelte-ff3e00?style=flat-square&logo=svelte&logoColor=white" alt="Svelte interface" />
  <a href="https://github.com/MatilyLabs/MaxRAR/issues"><img src="https://img.shields.io/github/issues/MatilyLabs/MaxRAR?style=flat-square&label=issues" alt="Open issues" /></a>
</p>

# MaxRAR — Archive Manager for Windows, macOS & Linux

**Know what you unpack.**

MaxRAR is a desktop archive manager by [Matily](https://www.matily.org), built with Rust and Tauri. Create ZIP and 7z archives, extract RAR4/RAR5 files, preview supported text before extraction, test archive integrity, and inspect archive contents locally.

[Download](#download) · [Screenshots](#screenshots) · [Supported formats](#supported-formats) · [MRAR](#experimental-mrar) · [User manual](#user-manual) · [CLI](#cli) · [FAQ](#frequently-asked-questions)


## Download

**MaxRAR v0.1.2 is available for testing.** Choose the download for your computer.

<p align="center">
  <a href="https://github.com/MatilyLabs/MaxRAR/releases/download/v0.1.2/MaxRAR_0.1.2_x64-setup.exe"><img src="docs/assets/buttons/get-windows.svg" alt="Download MaxRAR for Windows x64 (EXE installer)" width="200" height="53" /></a>
  <a href="https://github.com/MatilyLabs/MaxRAR/releases/download/v0.1.2/MaxRAR_0.1.2_macOS_apple-silicon.dmg"><img src="docs/assets/buttons/get-mac-apple-silicon.svg" alt="Download MaxRAR for Mac with Apple Silicon (DMG)" width="200" height="53" /></a>
  <a href="https://github.com/MatilyLabs/MaxRAR/releases/download/v0.1.2/MaxRAR_0.1.2_macOS_intel.dmg"><img src="docs/assets/buttons/get-mac-intel.svg" alt="Download MaxRAR for Mac with Intel (DMG)" width="200" height="53" /></a>
  <a href="https://github.com/MatilyLabs/MaxRAR/releases/tag/v0.1.2"><img src="docs/assets/buttons/get-linux.svg" alt="Download MaxRAR for Linux x64 (DEB, RPM or AppImage)" width="200" height="53" /></a>
</p>

<p align="center"><sub>v0.1.2 · Not sure which Mac? Apple menu → About This Mac · <a href="#first-launch">First launch</a></sub></p>

[Release notes](https://github.com/MatilyLabs/MaxRAR/releases/tag/v0.1.2) · [All downloads](https://github.com/MatilyLabs/MaxRAR/releases) · [SHA-256 checksums](https://github.com/MatilyLabs/MaxRAR/releases/download/v0.1.2/SHA256SUMS.txt)

| Your computer | Package | Download |
| --- | --- | --- |
| **Windows** — x64 | EXE installer | [Download for Windows](https://github.com/MatilyLabs/MaxRAR/releases/download/v0.1.2/MaxRAR_0.1.2_x64-setup.exe) |
| **Mac with Apple Silicon** (M1, M2, M3, M4…) | DMG | [Download for Apple Silicon](https://github.com/MatilyLabs/MaxRAR/releases/download/v0.1.2/MaxRAR_0.1.2_macOS_apple-silicon.dmg) |
| **Mac with Intel** | DMG | [Download for Intel Mac](https://github.com/MatilyLabs/MaxRAR/releases/download/v0.1.2/MaxRAR_0.1.2_macOS_intel.dmg) |
| **Linux** — x64 | DEB / RPM / AppImage | [DEB](https://github.com/MatilyLabs/MaxRAR/releases/download/v0.1.2/MaxRAR_0.1.2_amd64.deb) · [RPM](https://github.com/MatilyLabs/MaxRAR/releases/download/v0.1.2/MaxRAR-0.1.2-1.x86_64.rpm) · [AppImage](https://github.com/MatilyLabs/MaxRAR/releases/download/v0.1.2/MaxRAR_0.1.2_amd64.AppImage) |

Not sure which Mac you have? Open **Apple menu → About This Mac**. If it lists a chip such as “Apple M2”, choose Apple Silicon. If it lists an Intel processor, choose Intel.

### First launch

From v0.1.2, the Windows installer and app are signed by the publisher (Ripon Chandra Malo, through Azure Artifact Signing), and the Mac apps are signed with an Apple Developer ID and notarized by Apple. You can compare a download with the [checksums file](https://github.com/MatilyLabs/MaxRAR/releases/download/v0.1.2/SHA256SUMS.txt) (Windows: `certutil -hashfile <file> SHA256`; Mac/Linux: `shasum -a 256 <file>`).

**Windows**

1. Run `MaxRAR_0.1.2_x64-setup.exe`.
2. Windows SmartScreen may still show **“Windows protected your PC”** while the new signature builds reputation. Check that the publisher is Ripon Chandra Malo, choose **More info**, then **Run anyway**.
3. Follow the installer, then open **MaxRAR** from the Start menu.
4. On Windows 11, right-click files or folders and choose **MaxRAR**. On Windows 10 the commands are in the classic right-click menu, and **Send to → Compress with MaxRAR** works for several items.

**macOS**

1. Open the DMG for your Mac and drag **MaxRAR** into **Applications**.
2. Open MaxRAR from Applications. macOS asks once whether to open an app downloaded from the internet; click **Open**.
3. To add Finder compress/extract actions, choose **Help → Desktop integration → Enable desktop actions**.

**Linux**

Install the DEB or RPM with your package manager so system libraries are resolved. For the AppImage, make it executable (`chmod +x MaxRAR_0.1.2_amd64.AppImage`) and run it. Only the AppImage supports in-app updates; DEB/RPM installs link you to the latest packages.

### Using MaxRAR for the first time

- **Open an archive:** double-click it, choose **Open** in MaxRAR, or drag it onto the window. Browse folders, search filenames and preview supported text before extracting.
- **Extract:** choose **Extract To**, review the destination, pick Skip, Keep both or Replace for existing files, then **Extract**.
- **Create an archive:** select files or folders and choose a format (ZIP or 7z, with optional password). RAR can be opened and extracted, not created.
- **Check an archive:** **Test** verifies integrity; **Scan** is a limited local check, not antivirus.
- **Updates:** v0.1.0 had no working update check, so install v0.1.1 manually once. After that, use **About MaxRAR** to check for signed updates (the project’s release downloads must be publicly reachable).

See the [user manual](docs/USER-MANUAL.md) and [desktop integration guide](docs/DESKTOP-INTEGRATION.md) for more.

MaxRAR’s original code is licensed under [Apache License 2.0](LICENSE). License texts and notices are in [NOTICE](NOTICE) and [THIRD-PARTY-NOTICES.md](THIRD-PARTY-NOTICES.md).

## Why MaxRAR?

- **See what is inside.** Browse archive folders, search filenames, preview supported text, and see a compact breakdown of file types.
- **Open RAR archives.** Browse and extract supported single-volume RAR4/RAR5 archives without installing another application.
- **Create ZIP and 7z archives.** Package files and folders with optional password protection. TAR and single-file compression formats are also supported.
- **Extract with a clear destination.** Review where files will go and choose Skip, Keep both or Replace when files already exist.
- **Check before extracting.** Test archive readability and available checksums, or scan an archive or selected files with bundled local static checks.
- **Work from your desktop or terminal.** macOS Finder and Windows Explorer integration open compression and extraction review windows. A JSON CLI uses the same Rust core.

Archive content is not uploaded by the built-in scanner. Test checks integrity; Scan checks limited static indicators. Neither guarantees that content is safe.

## Screenshots

The following captures show the interface with sample data, including development preview screens. They illustrate the workflows; they are not a record of cross-platform release validation. The hero above is an illustrative animation, not a timed operation.

### Understand what is inside

File-type counts, a short description inferred from filenames and folders, and likely starting files help you explore an archive before extraction.

<img src="docs/assets/archive-inspector.png" alt="MaxRAR archive browser showing a sample website project, file-type counts and a likely starting file" width="777" />

### Choose where files go

Review the destination before extracting. Additional conflict and file details remain available when needed.

<img src="docs/assets/extract-files.png" alt="MaxRAR extraction dialog showing an archive, destination folder, ready status and Extract button" width="560" />

### Review local inspection results

Scan the archive or a selection. Results distinguish warning indicators and inspection coverage; a clean result is not a safety guarantee.

<img src="docs/assets/local-scan.png" alt="MaxRAR single-file scan result with a visual status scale, files checked and expandable scan details" width="600" />

## Supported formats

| Format | Browse / extract / test | Create |
| --- | --- | --- |
| ZIP | Yes, for enabled codecs | Deflate; optional AES-256 |
| 7z | Yes, for enabled codecs | LZMA2; optional AES-256 |
| TAR, TAR.GZ, TAR.BZ2, TAR.XZ, TAR.ZST | Yes, for supported variants | Yes |
| GZ, BZ2, XZ, ZST | Single decoded file | Single input file |
| MRAR (`.mrar`) | Experimental /0, /1 and /2 | Experimental /2 |
| ZIP-based JAR, OpenDocument, WHL, NPZ and modern ZIP-based checkpoints | Generic container operations | Generic ZIP creation |
| RAR4 / RAR5 (single volume) | Yes, including supported solid and encrypted archives | Not supported |

**Need another format?** [Request format support](https://github.com/MatilyLabs/MaxRAR/issues/new?title=Format%20support%20request) and tell us the extension and whether you need extraction, creation, or both.

ZIP supports staged deletion and comment edits. Other formats currently have more limited editing. Generic package/model inspection does not validate application semantics or deserialize models. Password protection can leave filenames visible. See the [core format contract](crates/wonrar-core/README.md) for exact behavior and limits.

**Test checks integrity; Scan checks limited static indicators. Neither is a guarantee that content is safe.** Scan is not comprehensive antivirus protection. Archive repair, split volumes and metadata-preserving backup support are not implemented.

## Experimental MRAR

**MaxRAR’s own archive format, with shared compression and built-in integrity checks.**

MRAR/2 compresses files together in 16 MiB blocks. For each block it tests
small samples and picks the method that fits the data: PPMd for text, LZMA2
for binaries and similar files, or no compression for data that won’t shrink.
Every block carries a SHA-256 check. MaxRAR can create, browse, preview, test
and extract `.mrar` archives, and still reads MRAR/0 and MRAR/1 files.

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="docs/assets/benchmarks/headline-dark.svg">
  <img alt="MRAR/2 at a glance: 20% smaller than 7-Zip -mx9 on 256 MiB of text and 38 times faster to create; 5% smaller than the best other tool on 4,096 small files; 6% larger than 7-Zip -mx9 on a real node_modules folder." src="docs/assets/benchmarks/headline-light.svg" width="880">
</picture>

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="docs/assets/benchmarks/size-dark.svg">
  <img alt="Archive size by corpus. Text: MRAR/2 28.84 MiB, xz -9 35.82, 7-Zip -mx9 35.94. Small files: MRAR/2 0.421 MiB, zstd -19 0.442, 7-Zip -mx9 0.457. Mixed: 7-Zip -mx9 146.4 MiB, MRAR/2 146.6. node_modules: 7-Zip -mx9 13.83 MiB, MRAR/2 14.67." src="docs/assets/benchmarks/size-light.svg" width="880">
</picture>

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="docs/assets/benchmarks/tradeoff-dark.svg">
  <img alt="Archive size against create time. On text, MRAR/2 is both the smallest and among the fastest, at 3.6 seconds against 138 seconds for 7-Zip -mx9. On node_modules, 7-Zip -mx9 and xz -9 are smaller but slower; MRAR/2 is on the speed-size frontier." src="docs/assets/benchmarks/tradeoff-light.svg" width="880">
</picture>

**Where it wins:** text (20% smaller than 7-Zip -mx9 and xz -9) and many small
related files (smallest of all tools tested). Mixed text and random data is a
tie with 7-Zip -mx9 at a fifth of the create time.

**Where it doesn’t yet:** a real `node_modules` folder is 6% larger than 7-Zip
-mx9. Extracting thousands of small files is slower than 7-Zip, because MaxRAR
flushes every file to disk. Creating archives can use up to about 2.5 GiB of
memory on a large machine; fewer threads are used when memory is short.

Measured on one Windows 11 PC (Intel Core i7-14700F, 32 GB), warm cache,
median of 3 trials, with every archive extracted and checked byte for byte.
Most inputs are generated test data; standard corpora such as Silesia and a
second machine are still to come. Results vary with files and settings.
[Raw results and method](docs/benchmarks/results/README.md) ·
regenerate the charts with `python scripts/benchmark/make_charts.py`.

**MRAR is experimental.** Keep your originals; use established formats when you
need stable exchange or backups.

[MRAR/2 specification](docs/MRAR-2-SPEC.md) · [MRAR/1 specification](docs/MRAR-1-SPEC.md) · [Legacy MRAR/0 specification](docs/MRAR-0-SPEC.md)

## Run locally

Requirements: Node.js 22+, Rust 1.95+, and the [Tauri platform prerequisites](https://v2.tauri.app/start/prerequisites/). Native builds require the appropriate platform toolchain and WebView libraries.

```sh
npm ci
npm run tauri -- dev
```

For the frontend alone, use `npm run dev`. Archive operations require the native desktop runtime.

### Check and build

```sh
npm run check
npm test
npm run build
cargo fmt --all -- --check
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
npm run tauri -- build
```

GitHub Actions builds and checks Windows x64, macOS Intel, macOS Apple Silicon and Linux x64 packages. Releases are published separately from successful build artifacts. CI success does not establish installed behavior on every platform; signing, notarization and broader compatibility testing remain ongoing work.

## CLI

The executable is `maxrar-cli`. Internal Cargo package names retain the `wonrar-*` prefix.

```sh
cargo build -p wonrar-cli --release --locked
maxrar-cli inspect ./bundle.zip --json
maxrar-cli test ./bundle.zip --json
maxrar-cli scan ./bundle.zip --json --progress
maxrar-cli create --output ./documents.zip ./documents --json
maxrar-cli plan ./bundle.zip --destination ./unpacked --policy skip --json > plan.json
maxrar-cli extract-plan plan.json --json --progress
```

Use the built executable from `target/release/` unless it is on your PATH. Review plans before executing them. Password input is available through `--password-stdin`; do not put passwords in command arguments. For automation, bind entry IDs to the reviewed archive using `--expect-identity`. Run `maxrar-cli --help` for all commands.

## Frequently asked questions

### Can MaxRAR replace WinRAR or 7-Zip?

MaxRAR is an alternative archive-manager workflow for supported ZIP, RAR extraction, 7z and TAR operations. It does not currently provide all WinRAR or 7-Zip features. Check the supported-format table before choosing it for a particular archive.

### Can MaxRAR open or create RAR files?

**Open and extract: yes. Create: no.**

MaxRAR supports browsing, extraction, integrity testing, text preview and local scanning for supported single-volume RAR4 and RAR5 archives, including solid compression and password-protected contents or headers. No separate WinRAR or UnRAR installation is required.

RAR creation, multipart volumes and self-extracting RAR executables are not supported.

### How do I extract a ZIP or 7z archive?

Open the archive, choose **Extract To**, review the destination and any conflicts, then choose **Extract**. You can also extract selected entries.

### Can I create password-protected archives?

Yes. ZIP and 7z creation support optional AES-256 encryption. Filenames may remain visible depending on the format and encryption options. See the [user manual](docs/USER-MANUAL.md).

### What is the difference between Test and Scan?

**Test** checks readability and available checksums to find integrity problems. **Scan** looks for structural warnings and bundled static rule matches. Scan is limited inspection, not comprehensive antivirus protection.

### Does the scanner upload my files?

No. Built-in scanning runs locally without uploading archive content or requiring a separately installed scanner.

### Is MRAR better than ZIP, 7z or RAR?

MRAR is experimental. Results depend on the input and settings, and current measurements do not establish a universal advantage. Keep originals and use established formats for exchange and backups.

## Repository structure

```text
crates/                 Rust archive core, CLI, inspection and scanner
src/                    Svelte desktop interface
src-tauri/              Native integration, capabilities, icons and packaging
public/                 Interface assets and their provenance
design/app-icon.svg    Canonical application icon source
design/icons/          Upstream icon license notices
docs/                   User manual and technical documentation
verification/           Automated checks and synthetic test fixtures
scripts/                Packaging and desktop integration helpers
licenses/               Preserved third-party license texts
.github/workflows/      Build and update-staging workflows
```

Builds, dependencies, signing material, machine-specific reports, private development notes and unpublished design work are excluded by `.gitignore`. Optional sample archives, promotion templates and benchmark research are also kept local. Automated tests, their interoperability fixtures, lockfiles and license notices are intentionally included.

## User manual

Read the [MaxRAR user manual](docs/USER-MANUAL.md) for browsing, creation, extraction, passwords, scans and updates.

## Documentation

- [Architecture and security boundaries](docs/ARCHITECTURE.md)
- [Desktop right-click integration](docs/DESKTOP-INTEGRATION.md)
- [Signed update configuration](docs/RELEASE-UPDATES.md)
- [Optional promotion feed](docs/PROMOTIONS.md)

## Project and licensing

A product of [Matily](https://www.matily.org). [Report an issue](https://github.com/MatilyLabs/MaxRAR/issues/new/choose) after reviewing any filenames or logs you choose to share.

MaxRAR’s original code, documentation, application artwork and authored baseline scan rules are licensed under [Apache License 2.0](LICENSE). See [NOTICE](NOTICE) for project attribution. Third-party code, icons and fixtures retain their respective licenses; see [third-party acknowledgments](THIRD-PARTY-NOTICES.md) and [license texts](licenses/). The native UnRAR decoder remains under its separate UnRAR license.
