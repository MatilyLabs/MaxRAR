//! Built-in RAR decoder process. Call `maybe_run` before initializing a host runtime.
//! Decoding uses RAR_TEST and a chunk callback; native destination extraction is disabled.
use crate::*;
use std::{
    ffi::{CString, OsStr},
    io::{BufRead, BufReader},
    process::{Child, Command, Stdio},
    sync::mpsc,
    thread,
    time::{Duration, Instant},
};
use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, System};
use unrar_ng_sys as native;

const SWITCH: &str = "--maxrar-rar-worker";
const MAX_FRAME: u64 = 32 * 1024 * 1024;
const MAX_REQUEST: u64 = 2 * 1024 * 1024;
const WALL: Duration = Duration::from_secs(120);

#[derive(Serialize, Deserialize)]
pub(crate) enum Operation {
    List,
    Decode { ids: Vec<usize> },
    Prefix { id: usize, limit: u64 },
    Test,
}
#[derive(Serialize, Deserialize)]
pub(crate) struct Request {
    pub parent_pid: u32,
    pub path: PathBuf,
    pub directory: PathBuf,
    pub password: Option<String>,
    pub operation: Operation,
    pub limits: Limits,
}
#[derive(Serialize, Deserialize)]
pub(crate) struct Item {
    pub entry: Entry,
    pub special: bool,
}
#[derive(Serialize, Deserialize)]
#[serde(tag = "type", content = "value")]
pub(crate) enum Frame {
    Inventory(Vec<Item>),
    Progress {
        id: usize,
        bytes: u64,
    },
    Ready {
        id: usize,
        bytes: u64,
        truncated: bool,
    },
    Done,
    Error(String),
}

fn send(value: &Frame) -> Result<()> {
    let bytes = serde_json::to_vec(value).map_err(|_| invalid("RAR worker encoding error"))?;
    if bytes.len() as u64 > MAX_FRAME {
        return Err(invalid("RAR worker inventory exceeds message limit"));
    }
    let mut out = std::io::stdout().lock();
    out.write_all(&bytes)?;
    out.write_all(b"\n")?;
    out.flush()?;
    Ok(())
}

struct CallbackState {
    secret: Option<CString>,
    output: Option<File>,
    total: u64,
    entry_bytes: u64,
    max_file: u64,
    max_total: u64,
    prefix: Option<u64>,
    truncated: bool,
    error: Option<String>,
    id: usize,
    last_progress: Instant,
}

extern "C" fn callback(
    message: native::UINT,
    data: native::LPARAM,
    p1: native::LPARAM,
    p2: native::LPARAM,
) -> i32 {
    // The state is boxed for a stable address, used synchronously only by UnRAR,
    // and outlives the handle. Never unwind across the native callback boundary.
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        if data == 0 {
            return -1;
        }
        // SAFETY: caller installed a live, exclusively borrowed CallbackState pointer.
        let state = unsafe { &mut *(data as *mut CallbackState) };
        match message {
            native::UCM_NEEDPASSWORD => {
                let Some(secret) = &state.secret else {
                    return -1;
                };
                let bytes = secret.as_bytes_with_nul();
                if p1 == 0 || p2 <= 0 || bytes.len() > p2 as usize {
                    return -1;
                }
                // SAFETY: UnRAR supplies a writable p2-byte password buffer.
                unsafe {
                    std::ptr::copy_nonoverlapping(bytes.as_ptr(), p1 as *mut u8, bytes.len())
                };
                1
            }
            native::UCM_NEEDPASSWORDW => {
                let Some(secret) = &state.secret else {
                    return -1;
                };
                let Ok(secret) = secret.to_str() else {
                    return -1;
                };
                #[cfg(windows)]
                let wide: Vec<native::WCHAR> = secret.encode_utf16().chain(Some(0)).collect();
                #[cfg(not(windows))]
                let wide: Vec<native::WCHAR> = secret
                    .chars()
                    .map(|c| c as native::WCHAR)
                    .chain(Some(0))
                    .collect();
                if p1 == 0 || p2 <= 0 || wide.len() > p2 as usize {
                    return -1;
                }
                // SAFETY: UnRAR supplies a writable buffer of p2 native wide characters.
                unsafe {
                    std::ptr::copy_nonoverlapping(
                        wide.as_ptr(),
                        p1 as *mut native::WCHAR,
                        wide.len(),
                    )
                };
                1
            }
            native::UCM_CHANGEVOLUME | native::UCM_CHANGEVOLUMEW => {
                state.error = Some(
                    "Multipart RAR archives are not supported yet. Use a single-volume archive."
                        .into(),
                );
                -1
            }
            native::UCM_LARGEDICT => {
                state.error = Some("RAR dictionary exceeds the supported memory limit".into());
                -1
            }
            native::UCM_PROCESSDATA => {
                if p2 == 0 {
                    return 1;
                }
                if p1 == 0 || p2 < 0 || p2 as u64 > 32 * 1024 * 1024 {
                    state.error = Some("RAR decoder returned an invalid chunk".into());
                    return -1;
                }
                let n = p2 as u64;
                if n > state.max_file.saturating_sub(state.entry_bytes)
                    || n > state.max_total.saturating_sub(state.total)
                {
                    state.error = Some("RAR decoded byte limit exceeded".into());
                    return -1;
                }
                // SAFETY: UnRAR owns this read-only n-byte chunk for this callback.
                let bytes = unsafe { std::slice::from_raw_parts(p1 as *const u8, n as usize) };
                let take = state
                    .prefix
                    .map_or(n, |limit| limit.saturating_sub(state.entry_bytes).min(n));
                if let Some(out) = &mut state.output {
                    if out.write_all(&bytes[..take as usize]).is_err() {
                        state.error = Some("Could not stage decoded RAR content".into());
                        return -1;
                    }
                }
                state.entry_bytes += n;
                state.total += n;
                if state.last_progress.elapsed() >= Duration::from_millis(100) {
                    let _ = send(&Frame::Progress {
                        id: state.id,
                        bytes: state.total,
                    });
                    state.last_progress = Instant::now();
                }
                if state.prefix.is_some_and(|limit| state.entry_bytes > limit) {
                    state.truncated = true;
                    return -1;
                }
                1
            }
            _ => -1, // No interactive prompts or unknown native requests.
        }
    }));
    result.unwrap_or(-1)
}

struct Handle(*const native::Handle);
impl Drop for Handle {
    fn drop(&mut self) {
        // SAFETY: this non-null native handle is owned here and closed exactly once.
        unsafe { native::RARCloseArchive(self.0) };
    }
}

fn native_error(code: i32) -> Error {
    invalid(match code {
        native::ERAR_MISSING_PASSWORD => "RAR password required".into(),
        native::ERAR_BAD_PASSWORD => "Incorrect RAR password".into(),
        native::ERAR_BAD_DATA => "RAR checksum failed: damaged data".into(),
        native::ERAR_NO_MEMORY | native::ERAR_LARGE_DICT => {
            "RAR decoder memory limit exceeded".into()
        }
        _ => format!(
            "RAR decoding failed (code {code}); the archive may be incomplete or unsupported"
        ),
    })
}

fn wide_name(wide: &[native::WCHAR]) -> Result<String> {
    let end = wide
        .iter()
        .position(|&c| c == 0)
        .ok_or_else(|| invalid("RAR filename too long"))?;
    #[cfg(windows)]
    {
        String::from_utf16(&wide[..end]).map_err(|_| invalid("Invalid RAR Unicode filename"))
    }
    #[cfg(not(windows))]
    {
        wide[..end]
            .iter()
            .map(|&c| {
                char::from_u32(c as u32).ok_or_else(|| invalid("Invalid RAR Unicode filename"))
            })
            .collect()
    }
}

fn decode(request: Request) -> Result<()> {
    let mut state = Box::new(CallbackState {
        secret: request
            .password
            .as_deref()
            .map(CString::new)
            .transpose()
            .map_err(|_| invalid("RAR password contains a NUL character"))?,
        output: None,
        total: 0,
        entry_bytes: 0,
        max_file: request.limits.max_file_bytes,
        max_total: request.limits.max_total_bytes,
        prefix: None,
        truncated: false,
        error: None,
        id: 0,
        last_progress: Instant::now(),
    });
    let path = request
        .path
        .to_str()
        .ok_or_else(|| invalid("RAR staging path is not Unicode"))?;
    #[cfg(windows)]
    let wide: Vec<u16> = path.encode_utf16().chain(Some(0)).collect();
    #[cfg(not(windows))]
    let wide: Vec<native::WCHAR> = path
        .chars()
        .map(|c| c as native::WCHAR)
        .chain(Some(0))
        .collect();
    // SAFETY: zero is a valid initial value for all scalar/pointer fields in this C ABI struct.
    let mut open: native::OpenArchiveDataEx = unsafe { std::mem::zeroed() };
    open.archive_name_w = wide.as_ptr();
    open.open_mode = if matches!(request.operation, Operation::List) {
        native::RAR_OM_LIST
    } else {
        native::RAR_OM_EXTRACT
    };
    open.callback = Some(callback);
    open.user_data = (&mut *state as *mut CallbackState) as native::LPARAM;
    // SAFETY: packed ABI data, filename and callback state remain valid throughout decoding.
    let raw = unsafe { native::RAROpenArchiveEx(std::ptr::addr_of_mut!(open)) };
    let handle = if raw.is_null() {
        None
    } else {
        Some(Handle(raw))
    };
    if open.open_result != 0 {
        return Err(native_error(open.open_result as i32));
    }
    let handle = handle.ok_or_else(|| invalid("Could not open RAR archive"))?;
    if open.flags & native::ROADF_VOLUME != 0 {
        return Err(invalid(
            "Multipart RAR archives are not supported yet. Use a single-volume archive.",
        ));
    }
    let mut items = Vec::new();
    let selected: HashSet<usize> = match &request.operation {
        Operation::Decode { ids } => ids.iter().copied().collect(),
        _ => HashSet::new(),
    };
    let mut ready = HashSet::new();
    for id in 0..=request.limits.max_entries {
        // SAFETY: header and backing Unicode buffer are initialized, writable and remain live during the call.
        let mut header: native::HeaderDataEx = unsafe { std::mem::zeroed() };
        let mut name_buffer = vec![0 as native::WCHAR; 32768];
        header.file_name_ex = name_buffer.as_mut_ptr();
        header.file_name_ex_size = name_buffer.len() as u32;
        let code = unsafe { native::RARReadHeaderEx(handle.0, std::ptr::addr_of_mut!(header)) };
        if code == native::ERAR_END_ARCHIVE {
            if matches!(request.operation, Operation::Prefix { .. }) {
                return Err(invalid("RAR entry ID not found"));
            }
            if !selected.is_subset(&ready) {
                return Err(invalid("RAR selected entry ID not found"));
            }
            if matches!(request.operation, Operation::List) {
                send(&Frame::Inventory(items))?;
            }
            return send(&Frame::Done);
        }
        if code != 0 {
            return Err(native_error(code));
        }
        if id == request.limits.max_entries {
            return Err(invalid("RAR inventory entry limit exceeded"));
        }
        if header.flags & (native::RHDF_SPLITBEFORE | native::RHDF_SPLITAFTER) != 0 {
            return Err(invalid("Multipart RAR archives are not supported yet"));
        }
        let name = wide_name(&name_buffer)?.replace('\\', "/");
        let is_dir = header.flags & native::RHDF_DIRECTORY != 0;
        let unix_type = header.file_attr & 0o170000;
        let special = header.redir_type != 0
            || (header.host_os == 3
                && unix_type != 0
                && unix_type != 0o100000
                && unix_type != 0o040000)
            || (header.host_os == 2 && header.file_attr & 0x400 != 0);
        let size = (header.unp_size_high as u64) << 32 | header.unp_size as u64;
        if matches!(request.operation, Operation::List) {
            items.push(Item {
                entry: Entry {
                    id: id.to_string(),
                    name,
                    size,
                    compressed_size: (header.pack_size_high as u64) << 32 | header.pack_size as u64,
                    is_dir,
                    is_encrypted: header.flags & native::RHDF_ENCRYPTED != 0,
                    modified: None,
                },
                special,
            });
            // SAFETY: live handle; RAR_SKIP does not write filesystem destinations.
            let code = unsafe {
                native::RARProcessFileW(
                    handle.0,
                    native::RAR_SKIP,
                    std::ptr::null(),
                    std::ptr::null(),
                )
            };
            if code != 0 {
                return Err(native_error(code));
            }
            continue;
        }
        let wanted = selected.contains(&id)
            || matches!(request.operation, Operation::Prefix { id: target, .. } if id == target);
        if is_dir && matches!(request.operation, Operation::Prefix { .. }) && wanted {
            return Err(invalid("Directories do not contain entry bytes"));
        }
        if special && wanted {
            return Err(invalid(
                "RAR links and special entries cannot be extracted or previewed",
            ));
        }
        if special && matches!(request.operation, Operation::Test) {
            return Err(invalid(
                "RAR test incomplete: links and special entries cannot be verified",
            ));
        }
        if header.dict_size > 256 * 1024 {
            return Err(invalid("RAR dictionary exceeds 256 MiB limit"));
        }
        if size > state.max_file {
            return Err(invalid("RAR declared file size exceeds decoded byte limit"));
        }
        state.id = id;
        state.entry_bytes = 0;
        state.truncated = false;
        state.prefix = match request.operation {
            Operation::Prefix { id: target, limit } if id == target => Some(limit),
            _ => None,
        };
        state.output = if wanted && !is_dir {
            Some(
                fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(request.directory.join(id.to_string()))?,
            )
        } else {
            None
        };
        send(&Frame::Progress {
            id,
            bytes: state.total,
        })?;
        // RAR_TEST streams decoded data through callback; it never extracts native paths.
        // Links are skipped, rather than asking UnRAR to resolve their targets.
        let action = if special {
            native::RAR_SKIP
        } else {
            native::RAR_TEST
        };
        let code = unsafe {
            native::RARProcessFileW(handle.0, action, std::ptr::null(), std::ptr::null())
        };
        if let Some(error) = state.error.take() {
            return Err(invalid(error));
        }
        state.output = None;
        if !state.truncated && code != 0 {
            return Err(native_error(code));
        }
        if !special && !state.truncated && state.entry_bytes != size {
            return Err(invalid("RAR decoded size differs from header"));
        }
        if wanted || matches!(request.operation, Operation::Test) {
            send(&Frame::Ready {
                id,
                bytes: state
                    .prefix
                    .unwrap_or(state.entry_bytes)
                    .min(state.entry_bytes),
                truncated: state.truncated,
            })?;
            ready.insert(id);
        }
        if state.prefix.is_some() || (!selected.is_empty() && selected.is_subset(&ready)) {
            return send(&Frame::Done);
        }
    }
    Err(invalid("RAR entry limit exceeded"))
}

/// Run before CLI parsing or desktop initialization. Passwords arrive on stdin only.
pub fn maybe_run() -> Option<i32> {
    if std::env::args_os().nth(1).as_deref() != Some(OsStr::new(SWITCH)) {
        return None;
    }
    #[cfg(unix)]
    unsafe {
        // SAFETY: valid initialized rlimit pointers; applies only to this dedicated worker.
        let cpu = libc::rlimit {
            rlim_cur: 120,
            rlim_max: 121,
        };
        let core = libc::rlimit {
            rlim_cur: 0,
            rlim_max: 0,
        };
        libc::setrlimit(libc::RLIMIT_CPU, &cpu);
        libc::setrlimit(libc::RLIMIT_CORE, &core);
    }
    let result = (|| {
        let mut bytes = Vec::new();
        std::io::stdin()
            .take(MAX_REQUEST + 1)
            .read_to_end(&mut bytes)?;
        if bytes.len() as u64 > MAX_REQUEST {
            return Err(invalid("RAR worker request too large"));
        }
        let request: Request =
            serde_json::from_slice(&bytes).map_err(|_| invalid("Invalid RAR worker request"))?;
        let owner = request.parent_pid;
        // Nested decoder workers also stop if their scanner/desktop parent is killed.
        thread::spawn(move || {
            let pid = Pid::from_u32(owner);
            let mut system = System::new();
            loop {
                system.refresh_processes_specifics(
                    ProcessesToUpdate::Some(&[pid]),
                    true,
                    ProcessRefreshKind::nothing(),
                );
                if system.process(pid).is_none() {
                    std::process::exit(1);
                }
                thread::sleep(Duration::from_millis(250));
            }
        });
        decode(request)
    })();
    Some(match result {
        Ok(()) => 0,
        Err(e) => {
            let _ = send(&Frame::Error(e.to_string()));
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

pub(crate) fn run(
    request: Request,
    cancel: &AtomicBool,
    mut frame: impl FnMut(Frame) -> Result<()>,
) -> Result<()> {
    check(cancel)?;
    let bytes =
        serde_json::to_vec(&request).map_err(|_| invalid("RAR worker request encoding failed"))?;
    if bytes.len() as u64 > MAX_REQUEST {
        return Err(invalid("RAR worker request too large"));
    }
    let mut command = Command::new(std::env::current_exe()?);
    command
        .arg(SWITCH)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    let mut child = ChildGuard(command.spawn()?);
    let mut input = child
        .0
        .stdin
        .take()
        .ok_or_else(|| invalid("RAR worker input unavailable"))?;
    let stdout = child
        .0
        .stdout
        .take()
        .ok_or_else(|| invalid("RAR worker output unavailable"))?;
    let writer = thread::spawn(move || input.write_all(&bytes));
    let (tx, rx) = mpsc::sync_channel(4);
    let reader = thread::spawn(move || {
        let mut reader = BufReader::new(stdout);
        loop {
            let mut bytes = Vec::new();
            match (&mut reader)
                .take(MAX_FRAME + 2)
                .read_until(b'\n', &mut bytes)
            {
                Ok(0) => break,
                Ok(n) if n as u64 <= MAX_FRAME + 1 && bytes.last() == Some(&b'\n') => {
                    let decoded = serde_json::from_slice(&bytes)
                        .map_err(|_| "Invalid RAR worker response".to_owned());
                    if tx.send(decoded).is_err() {
                        break;
                    }
                }
                _ => {
                    let _ = tx.send(Err("Oversized or incomplete RAR worker response".into()));
                    break;
                }
            }
        }
    });
    let started = Instant::now();
    let mut last_memory = started;
    let mut system = System::new();
    let pid = Pid::from_u32(child.0.id());
    let outcome = (|| {
        let mut done = false;
        loop {
            check(cancel)?;
            if started.elapsed() > WALL {
                return Err(invalid("RAR worker exceeded 120-second timeout"));
            }
            if last_memory.elapsed() >= Duration::from_millis(100) {
                last_memory = Instant::now();
                system.refresh_processes_specifics(
                    ProcessesToUpdate::Some(&[pid]),
                    true,
                    ProcessRefreshKind::nothing().with_memory(),
                );
                if system
                    .process(pid)
                    .is_some_and(|p| p.memory() > 768 * 1024 * 1024)
                {
                    return Err(invalid(
                        "RAR worker exceeded sampled 768 MiB memory threshold",
                    ));
                }
            }
            match rx.recv_timeout(Duration::from_millis(25)) {
                Ok(Ok(Frame::Error(e))) => return Err(invalid(e)),
                Ok(Ok(Frame::Done)) => {
                    if done {
                        return Err(invalid("Duplicate RAR completion"));
                    }
                    done = true;
                }
                Ok(Ok(value)) if !done => frame(value)?,
                Ok(Ok(_)) => return Err(invalid("Unexpected response after RAR completion")),
                Ok(Err(e)) => return Err(invalid(e)),
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    if let Some(status) = child.0.try_wait()? {
                        return if done && status.success() {
                            Ok(())
                        } else {
                            Err(invalid("RAR decoder stopped without a complete result"))
                        };
                    }
                    thread::sleep(Duration::from_millis(10));
                }
            }
        }
    })();
    drop(rx);
    drop(child); // Kill/reap before joining producers, including cancellation/error paths.
    let _ = writer.join();
    let _ = reader.join();
    outcome
}
