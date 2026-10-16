use clap::{Parser, Subcommand, ValueEnum};
use serde_json::{json, Value};
use std::{
    fs::File,
    io::Read,
    path::PathBuf,
    process::ExitCode,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};
use wonrar_core::{ConflictPolicy, ExtractionPlan, Limits};

/// MaxRAR archive automation. ZIP, RAR extraction, 7z, TAR, compression streams and experimental MRAR; no archive contents are executed.
#[derive(Parser)]
#[command(name = "maxrar-cli", version, about)]
struct Args {
    /// Emit compact machine-readable JSON. Standard output always contains JSON.
    #[arg(long, global = true)]
    json: bool,
    /// Emit line-delimited progress JSON to stderr.
    #[arg(long, global = true)]
    progress: bool,
    /// Read an archive password from stdin, never from process arguments.
    #[arg(long, global = true)]
    password_stdin: bool,
    /// Require this inspected archive SHA-256 for plan/extract/delete/comment/scan operations.
    #[arg(long, global = true)]
    expect_identity: Option<String>,
    #[command(subcommand)]
    command: Command,
}
#[derive(Clone, Copy, ValueEnum)]
enum Policy {
    Skip,
    KeepBoth,
    Replace,
}
impl From<Policy> for ConflictPolicy {
    fn from(value: Policy) -> Self {
        match value {
            Policy::Skip => Self::Skip,
            Policy::KeepBoth => Self::KeepBoth,
            Policy::Replace => Self::Replace,
        }
    }
}
#[derive(Subcommand)]
enum Command {
    /// Inspect inventory, capabilities, identity and evidence-based findings.
    Inspect { archive: PathBuf },
    /// Retrieve bounded cited evidence; optionally ask an installed local Ollama model.
    Ask {
        archive: PathBuf,
        question: String,
        #[arg(long)]
        model: Option<String>,
    },
    /// Run built-in offline structural and bundled static-rule inspection.
    Scan {
        archive: PathBuf,
        /// Scan only these file entry IDs; requires --expect-identity from inspect/list.
        #[arg(long = "entry")]
        entries: Vec<String>,
    },
    /// List entries, with stable IDs tied to the archive identity.
    List { archive: PathBuf },
    /// Decode supported payloads and verify available checksums; this is not a malware scan.
    Test { archive: PathBuf },
    /// Preview bounded UTF-8 text without writing or executing archive content.
    Preview {
        archive: PathBuf,
        #[arg(long)]
        entry: String,
        #[arg(long, default_value_t = 262144)]
        max_bytes: u64,
    },
    /// Review output paths, blocked entries and destination conflicts.
    Plan {
        archive: PathBuf,
        #[arg(short, long)]
        destination: PathBuf,
        #[arg(long)]
        entry: Vec<String>,
        #[arg(long, value_enum, default_value = "skip")]
        policy: Policy,
    },
    /// Plan and extract selected entries (default collision policy: skip).
    Extract {
        archive: PathBuf,
        #[arg(short, long)]
        destination: PathBuf,
        #[arg(long)]
        entry: Vec<String>,
        #[arg(long, value_enum, default_value = "skip")]
        policy: Policy,
    },
    /// Execute an exported plan after revalidating archive identity and conflicts.
    ExtractPlan { plan: PathBuf },
    /// Create ZIP, 7z, TAR/compression streams or experimental .mrar. Existing outputs are preserved.
    Create {
        #[arg(short, long)]
        output: PathBuf,
        #[arg(required = true)]
        inputs: Vec<PathBuf>,
    },
    /// Delete selected entry IDs from a ZIP using a staged rewrite.
    Delete {
        archive: PathBuf,
        #[arg(long, required = true)]
        entry: Vec<String>,
        #[arg(long, required = true)]
        yes: bool,
    },
    /// Replace a ZIP archive comment from a UTF-8 text file.
    Comment {
        archive: PathBuf,
        #[arg(long)]
        text_file: PathBuf,
    },
}

fn perform(args: &Args, cancel: &AtomicBool, password: Option<&str>) -> Result<Value, String> {
    if args.expect_identity.is_some()
        && !matches!(
            args.command,
            Command::Plan { .. }
                | Command::Extract { .. }
                | Command::ExtractPlan { .. }
                | Command::Delete { .. }
                | Command::Comment { .. }
                | Command::Scan { .. }
        )
    {
        return Err(
            "--expect-identity applies to plan, extract, extract-plan, delete, comment or scan."
                .into(),
        );
    }
    let check_identity = |identity: &str| -> Result<(), String> {
        if args
            .expect_identity
            .as_deref()
            .is_some_and(|expected| expected != identity)
        {
            Err("Archive changed since inspection; refresh entry selection before modifying or extracting.".into())
        } else {
            Ok(())
        }
    };
    let progress = |p: wonrar_core::Progress| {
        if args.progress {
            if let Ok(line) = serde_json::to_string(&json!({"type":"progress", "data":p})) {
                eprintln!("{line}");
            }
        }
    };
    fn value<T: serde::Serialize>(r: wonrar_core::Result<T>) -> Result<Value, String> {
        serde_json::to_value(r.map_err(|e| e.to_string())?).map_err(|e| e.to_string())
    }
    match &args.command {
        Command::Inspect { archive } => {
            value(wonrar_core::inspect_with_password(archive, password))
        }
        Command::List { archive } => {
            let info =
                wonrar_core::inspect_with_password(archive, password).map_err(|e| e.to_string())?;
            Ok(json!({"archive_identity":info.identity,"entries":info.entries}))
        }
        Command::Ask {
            archive,
            question,
            model,
        } => serde_json::to_value(wonrar_inspect::ask(archive, question, model.as_deref())?)
            .map_err(|e| e.to_string()),
        Command::Scan { archive, entries } => {
            serde_json::to_value(wonrar_scan::worker::scan_reviewed(
                archive,
                password,
                if entries.is_empty() {
                    None
                } else {
                    Some(entries.as_slice())
                },
                args.expect_identity.as_deref(),
                cancel,
                &progress,
            )?)
            .map_err(|e| e.to_string())
        }
        Command::Test { archive } => value(wonrar_core::test_with_password(
            archive, cancel, &progress, password,
        )),
        Command::Preview {
            archive,
            entry,
            max_bytes,
        } => value(wonrar_core::preview_with_password(
            archive, entry, *max_bytes, password,
        )),
        Command::Plan {
            archive,
            destination,
            entry,
            policy,
        } => {
            let plan = wonrar_core::plan_with_password(
                archive,
                destination,
                entry,
                (*policy).into(),
                Limits::default(),
                password,
            )
            .map_err(|e| e.to_string())?;
            check_identity(&plan.archive_identity)?;
            serde_json::to_value(plan).map_err(|e| e.to_string())
        }
        Command::Extract {
            archive,
            destination,
            entry,
            policy,
        } => {
            let plan = wonrar_core::plan_with_password(
                archive,
                destination,
                entry,
                (*policy).into(),
                Limits::default(),
                password,
            )
            .map_err(|e| e.to_string())?;
            check_identity(&plan.archive_identity)?;
            value(wonrar_core::extract_with_password(
                &plan, cancel, &progress, password,
            ))
        }
        Command::ExtractPlan { plan } => {
            let reviewed: ExtractionPlan =
                serde_json::from_reader(File::open(plan).map_err(|e| e.to_string())?)
                    .map_err(|e| e.to_string())?;
            check_identity(&reviewed.archive_identity)?;
            value(wonrar_core::extract_with_password(
                &reviewed, cancel, &progress, password,
            ))
        }
        Command::Create { output, inputs } => value(wonrar_core::create_with_password(
            output, inputs, cancel, &progress, password,
        )),
        Command::Delete {
            archive,
            entry,
            yes,
        } => {
            if !yes {
                return Err("Deleting entries requires --yes".into());
            }
            if let Some(expected) = args.expect_identity.as_deref() {
                value(wonrar_core::delete_entries_checked(
                    archive, entry, expected, cancel,
                ))
            } else {
                value(wonrar_core::delete_entries(archive, entry, cancel))
            }
        }
        Command::Comment { archive, text_file } => {
            let file = File::open(text_file).map_err(|e| e.to_string())?;
            if file.metadata().map_err(|e| e.to_string())?.len() > 65535 {
                return Err("ZIP comments are limited to 65535 bytes".into());
            }
            let mut text = String::new();
            file.take(65536)
                .read_to_string(&mut text)
                .map_err(|e| e.to_string())?;
            if let Some(expected) = args.expect_identity.as_deref() {
                value(wonrar_core::set_comment_checked(
                    archive, &text, expected, cancel,
                ))
            } else {
                value(wonrar_core::set_comment(archive, &text, cancel))
            }
        }
    }
}

fn main() -> ExitCode {
    if let Some(code) = wonrar_core::rar_worker::maybe_run() {
        std::process::exit(code);
    }
    if let Some(code) = wonrar_scan::worker::maybe_run() {
        std::process::exit(code);
    }
    let args = Args::parse();
    let cancel = Arc::new(AtomicBool::new(false));
    let signal = cancel.clone();
    if let Err(error) = ctrlc::set_handler(move || signal.store(true, Ordering::Relaxed)) {
        eprintln!("Could not install cancellation handler: {error}");
        return ExitCode::FAILURE;
    }
    let password = if args.password_stdin {
        let mut bytes = Vec::new();
        if let Err(error) = std::io::stdin().take(4097).read_to_end(&mut bytes) {
            eprintln!("Could not read password: {error}");
            return ExitCode::FAILURE;
        }
        if bytes.len() > 4096 {
            eprintln!("Password input exceeds 4096 bytes");
            return ExitCode::FAILURE;
        }
        match String::from_utf8(bytes) {
            Ok(s) => Some(s.trim_end_matches(['\r', '\n']).to_owned()),
            Err(_) => {
                eprintln!("Password must be UTF-8");
                return ExitCode::FAILURE;
            }
        }
    } else {
        None
    };
    let result = perform(&args, &cancel, password.as_deref());
    let (output, code) = match result {
        Ok(value) => (value, ExitCode::SUCCESS),
        Err(error) => (
            json!({"error":error,"cancelled":cancel.load(Ordering::Relaxed)}),
            if cancel.load(Ordering::Relaxed) {
                ExitCode::from(130)
            } else {
                ExitCode::FAILURE
            },
        ),
    };
    let output = if args.json {
        serde_json::to_string(&output)
    } else {
        serde_json::to_string_pretty(&output)
    };
    match output {
        Ok(text) => {
            println!("{text}");
            code
        }
        Err(error) => {
            eprintln!("JSON encoding failed: {error}");
            ExitCode::FAILURE
        }
    }
}
