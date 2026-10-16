//! Same-executable scan worker: private pipes, bounded messages and kill/reap supervision.
//! This is process supervision, not an OS filesystem or network permission sandbox.
use crate::ScanReport;
use serde::{Deserialize, Serialize};
use std::{
    io::{BufRead, BufReader, Read, Write},
    path::Path,
    process::{Child, Command, Stdio},
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread,
    time::{Duration, Instant},
};
use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, System};
use wonrar_core::Progress;

const SWITCH: &str = "--wonrar-scan-worker";
const MAX_REQUEST: u64 = 16 * 1024;
const MAX_FRAME: usize = 2 * 1024 * 1024;
const MAX_OUTPUT: usize = 16 * 1024 * 1024;
const MAX_RSS: u64 = 768 * 1024 * 1024;
const MAX_WALL: Duration = Duration::from_secs(120);

#[derive(Serialize, Deserialize)]
struct Request {
    path: String,
    password: Option<String>,
    #[serde(default)]
    entry_ids: Option<Vec<String>>,
    #[serde(default)]
    expected_identity: Option<String>,
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "type", content = "payload", rename_all = "snake_case")]
enum Frame {
    Progress(Progress),
    Report(Box<ScanReport>),
    Error(String),
}

fn send(frame: &Frame) -> Result<(), String> {
    let bytes = serde_json::to_vec(frame).map_err(|e| e.to_string())?;
    if bytes.len() > MAX_FRAME {
        return Err("Scan report exceeded the worker message limit.".into());
    }
    let mut out = std::io::stdout().lock();
    out.write_all(&bytes)
        .and_then(|_| out.write_all(b"\n"))
        .and_then(|_| out.flush())
        .map_err(|e| e.to_string())
}

/// Must be called at executable entry before initializing the desktop runtime or CLI parser.
pub fn maybe_run() -> Option<i32> {
    if std::env::args_os().nth(1).as_deref() != Some(std::ffi::OsStr::new(SWITCH)) {
        return None;
    }
    #[cfg(unix)]
    {
        // These limits apply only in the dedicated child, never in the desktop parent.
        let cpu = libc::rlimit {
            rlim_cur: 120,
            rlim_max: 121,
        };
        let core = libc::rlimit {
            rlim_cur: 0,
            rlim_max: 0,
        };
        // SAFETY: valid pointers to initialized rlimit structures; libc retains no references.
        unsafe {
            libc::setrlimit(libc::RLIMIT_CPU, &cpu);
            libc::setrlimit(libc::RLIMIT_CORE, &core);
        }
    }
    let result = (|| {
        let mut bytes = Vec::new();
        std::io::stdin()
            .take(MAX_REQUEST + 1)
            .read_to_end(&mut bytes)
            .map_err(|e| e.to_string())?;
        if bytes.len() as u64 > MAX_REQUEST {
            return Err("Scan worker request is too large.".to_owned());
        }
        let request: Request = serde_json::from_slice(&bytes)
            .map_err(|_| "Invalid scan worker request.".to_owned())?;
        let started = Instant::now();
        let last = std::sync::Mutex::new(
            started
                .checked_sub(Duration::from_millis(100))
                .unwrap_or(started),
        );
        let cancelled = AtomicBool::new(false);
        let report = crate::scan_scoped(
            Path::new(&request.path),
            request.password.as_deref(),
            request.entry_ids.as_deref(),
            request.expected_identity.as_deref(),
            &cancelled,
            &|progress| {
                if let Ok(mut last) = last.lock() {
                    if last.elapsed() >= Duration::from_millis(100) {
                        *last = Instant::now();
                        let _ = send(&Frame::Progress(progress));
                    }
                }
            },
            crate::ScanLimits::default(),
        )?;
        send(&Frame::Report(Box::new(report)))
    })();
    Some(match result {
        Ok(()) => 0,
        Err(error) => {
            let _ = send(&Frame::Error(error));
            1
        }
    })
}

struct ChildGuard(Child);
impl Drop for ChildGuard {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn read_frames(reader: impl Read, tx: mpsc::SyncSender<Result<Frame, String>>) {
    let mut reader = BufReader::new(reader);
    let mut total = 0usize;
    loop {
        let mut bytes = Vec::new();
        let read = (&mut reader)
            .take(MAX_FRAME as u64 + 2)
            .read_until(b'\n', &mut bytes);
        match read {
            Ok(0) => return,
            Ok(n) if n <= MAX_FRAME + 1 && bytes.last() == Some(&b'\n') => {
                total += n;
                if total > MAX_OUTPUT {
                    let _ = tx.send(Err("Scan worker output limit exceeded.".into()));
                    return;
                }
                let frame = serde_json::from_slice(&bytes)
                    .map_err(|_| "Invalid scan worker response.".into());
                let failed = frame.is_err();
                if tx.send(frame).is_err() || failed {
                    return;
                }
            }
            _ => {
                let _ = tx.send(Err(
                    "Scan worker sent an oversized or incomplete message.".into()
                ));
                return;
            }
        }
    }
}

/// Executes a built-in scan in a separate copy of the current application.
pub fn scan(
    path: &Path,
    password: Option<&str>,
    cancel: &AtomicBool,
    progress: &dyn Fn(Progress),
) -> Result<ScanReport, String> {
    scan_reviewed(path, password, None, None, cancel, progress)
}
pub fn scan_selected(
    path: &Path,
    password: Option<&str>,
    entry_ids: &[String],
    expected_identity: &str,
    cancel: &AtomicBool,
    progress: &dyn Fn(Progress),
) -> Result<ScanReport, String> {
    scan_reviewed(
        path,
        password,
        Some(entry_ids),
        Some(expected_identity),
        cancel,
        progress,
    )
}
pub fn scan_reviewed(
    path: &Path,
    password: Option<&str>,
    entry_ids: Option<&[String]>,
    expected_identity: Option<&str>,
    cancel: &AtomicBool,
    progress: &dyn Fn(Progress),
) -> Result<ScanReport, String> {
    if cancel.load(Ordering::Relaxed) {
        return Err("Scan cancelled before starting.".into());
    }
    let request = serde_json::to_vec(&Request {
        path: path
            .to_str()
            .ok_or("Scan path must be valid Unicode.")?
            .into(),
        password: password.map(str::to_owned),
        entry_ids: entry_ids.map(<[String]>::to_vec),
        expected_identity: expected_identity.map(str::to_owned),
    })
    .map_err(|e| e.to_string())?;
    if request.len() as u64 > MAX_REQUEST {
        return Err(
            "Scan request exceeds the worker request limit (path, password and selected IDs)."
                .into(),
        );
    }
    let staging = tempfile::Builder::new()
        .prefix("wonrar-scan-work-")
        .tempdir()
        .map_err(|e| e.to_string())?;
    let mut command = Command::new(std::env::current_exe().map_err(|e| e.to_string())?);
    command
        .arg(SWITCH)
        // Parent-owned staging also cleans up nested decoded files after forced termination.
        .env("TMPDIR", staging.path())
        .env("TMP", staging.path())
        .env("TEMP", staging.path())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    let mut child = ChildGuard(
        command
            .spawn()
            .map_err(|e| format!("Could not start built-in scan worker: {e}"))?,
    );
    let mut stdin = child
        .0
        .stdin
        .take()
        .ok_or("Scan worker input is unavailable.")?;
    let stdout = child
        .0
        .stdout
        .take()
        .ok_or("Scan worker output is unavailable.")?;
    // Password travels only through this private stdin pipe, never process arguments.
    let writer = thread::spawn(move || stdin.write_all(&request));
    let (tx, rx) = mpsc::sync_channel(8);
    let reader = thread::spawn(move || read_frames(stdout, tx));
    let started = Instant::now();
    let mut last_memory = started
        .checked_sub(Duration::from_millis(500))
        .unwrap_or(started);
    let pid = Pid::from_u32(child.0.id());
    let mut system = System::new();
    let mut memory_observed = false;
    let mut report = None;
    let mut exited = None;
    let mut disconnected = false;
    let outcome = loop {
        if cancel.load(Ordering::Relaxed) {
            break Err("Scan cancelled; the worker was stopped.".into());
        }
        if started.elapsed() > MAX_WALL {
            break Err(
                "Inspection incomplete: scan worker exceeded the 120-second timeout.".into(),
            );
        }
        if last_memory.elapsed() >= Duration::from_millis(250) {
            last_memory = Instant::now();
            system.refresh_processes_specifics(
                ProcessesToUpdate::Some(&[pid]),
                true,
                ProcessRefreshKind::nothing().with_memory(),
            );
            if let Some(process) = system.process(pid) {
                memory_observed |= process.memory() > 0;
                if process.memory() > MAX_RSS {
                    break Err("Inspection incomplete: scan worker exceeded the 768 MiB sampled memory threshold.".into());
                }
            }
        }
        match rx.recv_timeout(Duration::from_millis(25)) {
            Ok(Ok(Frame::Progress(p))) => progress(p),
            Ok(Ok(Frame::Report(value))) => {
                if report.is_some() {
                    break Err("Scan worker returned duplicate reports.".into());
                }
                report = Some(*value);
            }
            Ok(Ok(Frame::Error(error))) | Ok(Err(error)) => break Err(error),
            Err(mpsc::RecvTimeoutError::Disconnected) => disconnected = true,
            Err(mpsc::RecvTimeoutError::Timeout) => {}
        }
        if exited.is_none() {
            match child.0.try_wait() {
                Ok(Some(status)) => exited = Some(status),
                Ok(None) => {}
                Err(error) => break Err(error.to_string()),
            }
        }
        if let Some(status) = exited {
            if disconnected {
                if !status.success() {
                    break Err("Built-in scan worker stopped unexpectedly; no complete report is available.".into());
                }
                break report.ok_or_else(|| "Built-in scan worker returned no report.".to_owned());
            }
        }
        if disconnected && exited.is_none() {
            thread::sleep(Duration::from_millis(10));
        }
    };
    drop(rx); // Unblock a producer if cancellation/error occurred with a full message queue.
    drop(child); // kill/reap on every error and cancellation path, then let pipes close.
    let _ = writer.join();
    let _ = reader.join();
    let mut report = outcome?;
    report.warnings.push("Scan ran in a supervised worker (120-second wall timeout; sampled 768 MiB resident-memory threshold). Sampling can overshoot and is not a hard memory reservation or filesystem/network permission sandbox.".into());
    if !memory_observed {
        report.warnings.push("Worker resident memory could not be observed during this scan; only time and data limits were supervised.".into());
    }
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn worker_protocol_rejects_unbounded_or_invalid_frames() {
        let (tx, rx) = mpsc::sync_channel(8);
        read_frames(std::io::Cursor::new(vec![b'x'; MAX_FRAME + 2]), tx);
        assert!(rx.recv().unwrap().is_err());
        let (tx, rx) = mpsc::sync_channel(8);
        read_frames(std::io::Cursor::new(b"not-json\n"), tx);
        assert!(rx.recv().unwrap().is_err());
    }
}
