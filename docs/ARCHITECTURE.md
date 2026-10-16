# Architecture and integration

## Packages

- `crates/wonrar-core`: shared ZIP/7z inventory, path policy, preview, planning, extraction, creation and staged editing.
- `crates/wonrar-cli`: structured automation over the same operations; JSON stdout and optional progress stderr.
- `crates/wonrar-inspect`: bounded evidence retrieval, static paper-source profiling and optional local model explanations.
- `crates/wonrar-scan`: bundled YARA-X rules, structural/content inspection, snapshots and supervised same-executable worker.
- `src-tauri`: native desktop IPC and dialogs. CPU/file operations run on blocking workers; long jobs share a single-worker semaphore (mutations are serialized to avoid concurrent archive rewrites) and per-job cooperative cancellation tokens.
- `src`: desktop web interface. No remote content is rendered as executable archive HTML.

## IPC contract

Tauri arguments are camelCase at the top level. Nested core structures use snake_case. All errors reject the invoke promise with a human-readable string. Job operations resolve only after actual completion and return their actual core result.

| Command | Arguments | Result |
| --- | --- | --- |
| `inspect_archive` | `{path,password?}` | `ArchiveInfo` |
| `preview_entry` | `{path, entryId, maxBytes?, password?}` | `Preview` |
| `plan_extraction` | `{request: {path,archive_identity,destination,entry_ids,policy,limits?,password?}}` | `ExtractionPlan` |
| `extract_archive` | `{request: ExtractionPlan, jobId, password?}` | `Receipt` |
| `create_archive` | `{request: {output,inputs,password?}, jobId}` | `ArchiveInfo` |
| `test_archive` | `{request: {path,password?}, jobId}` | `Receipt` |
| `delete_entries` | `{request: {path,archive_identity,entry_ids}, jobId}` | `ArchiveInfo` |
| `update_comment` | `{request: {path,archive_identity,comment}, jobId}` | `ArchiveInfo` |
| `scanner_status` | `{}` | Built-in engine/rule versions |
| `scan_archive` | `{path,password?,jobId}` | Built-in `ScanReport` |
| `research_profile` | `{path,password?}` | Static source/title/bibliography/figure hints, identity and coverage |
| `ask_archive` | `{path,question,model?}` | Bounded cited evidence / optional local model answer |
| `initial_archive_path` | `{}` | Launch archive path or null |
| `cancel_job` | `{jobId}` | `boolean` (whether active token found) |

Passwords are session-only invoke inputs, never part of plans or progress events. ZIP/7z creation uses AES-256 when a password is supplied; names are not encrypted.

The caller supplies a unique job ID before starting a job, enabling cancellation while the promise remains pending. `wonrar://job` events carry `{id,kind,status,progress,message,receipt?}`. Status is `queued`, `running`, `completed`, `failed`, or `cancelled`; progress is a 0–1 fraction or null. Progress reports work performed, not a time estimate. Completion has the serialized core result in `receipt`. Errors/cancellation currently report text, not a complete partial receipt. Do not erase completed job records immediately from the interface.

The `ArchiveInfo.identity` value is a SHA-256 of the archive content. A reviewed plan includes that identity. Extraction revalidates identity, output names, selected entries, resource limits and destination conflicts. The shared core remains authoritative; changing frontend JSON cannot bypass its policy.

Desktop planning and mutations bind entry IDs to the displayed archive identity. Delete/comment validate that identity inside the mutation lock before staging changes, then verify the actual source handle before and after copying. A stale inventory is rejected without replacing archive bytes. CLI callers can supply `--expect-identity` for planning, extraction and mutations to bind an operation to an earlier inventory.

Native dialogs are exposed through `@tauri-apps/plugin-dialog`. Reveal actions use `@tauri-apps/plugin-opener` with reveal-only permission. Archive content cannot invoke shell commands. There is no general filesystem or shell plugin permission. The desktop CSP allows only application assets and Tauri IPC; it does not grant webview model/network access.

## Boundaries and remaining release work

The core is currently in process on blocking workers. Decoded-byte limits apply while copying streams; decoder dictionary/header allocations are not bounded by those byte limits. The current sevenz-rust2 reader uses a platform-maximum dictionary memory allowance internally. Dedicated decoder processes and enforceable memory/time limits remain a security release gate. Cooperative cancellation checks bounded copy chunks; a native process supervisor for hostile decoder isolation is future work. Archive metadata decoding, hashing and blocking filesystem calls are not guaranteed instantly interruptible. The queue and progress do not imply decoding can resume.

Inventory findings are deterministic signals, including unsafe names and scripts. They are not malware diagnoses. Built-in Scan uses YARA-X and five authored baseline rules with independent findings and coverage. No external scanner, VirusTotal service/API, hash lookup or archive upload is included. The native executable/CLI recognizes a private `--wonrar-scan-worker` entry mode before UI/CLI initialization. The parent sends path/password over private stdin and consumes bounded progress/report JSON frames. Cancellation, 120-second wall timeout or sampled768MiB RSS excess kills/reaps the worker; parent-owned private temporary storage removes decoded staging after forced termination. Unix workers also set CPU limits and disable core dumps. This process supervision is not an OS permission sandbox or a hard memory reservation; the monitor can overshoot. RAR operations additionally use a dedicated same-executable decoder worker via `--maxrar-rar-worker`. It receives only a private archive snapshot and numerical staging destinations, uses RAR_TEST callbacks rather than native destination extraction, and enforces decoded byte/dictionary limits. The parent supervises wall time and sampled memory; a parent-liveness monitor stops orphaned nested RAR workers. Other archive operations use in-process blocking workers. Reports bind results to an archive snapshot/content identity and engine/rule versions; skipped/truncated entries and opaque nested containers remain visible. No complete-antivirus coverage is claimed. Model output must keep citations, evidence gaps and model provenance visible and cannot initiate archive writes.

The current metadata policy preserves content and reviewed names, not modification times, executable/permission bits, ownership, ACLs or xattrs. ZIP creation uses default timestamps; extracted Unix files remain mode `0600`. Before claiming metadata-preserving archiving or backup compatibility, add deliberate metadata restoration and per-platform tests.

Before publishing: run native installed-build workflows on all supported operating systems, validate compatibility corpus and hostile inputs, review transitive licenses, add signing/notarization, and verify assistive technology, high DPI, IME and filesystem behavior per platform.

## Simplified extraction and paper profile

The frontend suggests a sibling enclosing folder, defaults to Skip and automatically computes a versioned preview of the plan. The final Extract action obtains a fresh authoritative plan. A changed archive identity, blocked paths or changed conflict decisions requires review. Async plan/profile responses are guarded against stale archive/input revisions. The backend independently validates the final plan before writing.

The research profile inspects up to16 TeX sources at16KiB each, identifies literal documentclass/title hints without macro expansion, and lists at most64 bibliography/figure candidates each. Extensionless single compressed TeX can be identified from its bounded text. File/line references remain archive-identity scoped. No TeX compilation, model deserialization, code execution, notebook execution or package installation is performed.
