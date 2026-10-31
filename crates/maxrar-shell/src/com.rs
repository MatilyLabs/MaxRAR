//! COM objects Explorer loads: a "MaxRAR" root command with one sub-command per action.

use crate::{visible_actions, Action};
use std::ffi::{c_void, OsString};
use std::os::windows::ffi::OsStringExt;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use windows::core::{implement, Interface, Ref, Result, BOOL, GUID, HRESULT, PWSTR};
use windows::Win32::Foundation::{
    CLASS_E_CLASSNOTAVAILABLE, CLASS_E_NOAGGREGATION, E_NOTIMPL, E_POINTER, HMODULE, MAX_PATH,
    S_FALSE, S_OK,
};
use windows::Win32::System::Com::{CoTaskMemFree, IBindCtx, IClassFactory, IClassFactory_Impl};
use windows::Win32::System::LibraryLoader::{
    GetModuleFileNameW, GetModuleHandleExW, GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS,
    GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT,
};
use windows::Win32::UI::Shell::{
    IEnumExplorerCommand, IEnumExplorerCommand_Impl, IExplorerCommand, IExplorerCommand_Impl,
    IShellItemArray, SHStrDupW, ECF_DEFAULT, ECF_HASSUBCOMMANDS, ECS_ENABLED, ECS_HIDDEN,
    SIGDN_FILESYSPATH,
};

pub const CLSID: GUID = GUID::from_u128(0x94ae9828_d27b_4b7d_9a32_c2e17040f695);

static OBJECTS: AtomicUsize = AtomicUsize::new(0);

struct Live;
impl Live {
    fn new() -> Self {
        OBJECTS.fetch_add(1, Ordering::SeqCst);
        Live
    }
}
impl Drop for Live {
    fn drop(&mut self) {
        OBJECTS.fetch_sub(1, Ordering::SeqCst);
    }
}

fn wide(text: &str) -> Result<PWSTR> {
    let mut v: Vec<u16> = text.encode_utf16().collect();
    v.push(0);
    unsafe { SHStrDupW(windows::core::PCWSTR(v.as_ptr())) }
}

/// Folder containing this DLL, which the installer places next to `maxrar.exe`.
fn module_dir() -> Option<PathBuf> {
    let mut module = HMODULE::default();
    unsafe {
        GetModuleHandleExW(
            GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS | GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT,
            windows::core::PCWSTR(module_dir as *const () as *const u16),
            &mut module,
        )
        .ok()?;
    }
    let mut buf = vec![0u16; MAX_PATH as usize];
    loop {
        let n = unsafe { GetModuleFileNameW(Some(module), &mut buf) } as usize;
        if n == 0 {
            return None;
        }
        if n < buf.len() {
            let path = PathBuf::from(OsString::from_wide(&buf[..n]));
            return path.parent().map(PathBuf::from);
        }
        if buf.len() >= 32 * 1024 {
            return None;
        }
        buf.resize(buf.len() * 2, 0);
    }
}

fn app_exe() -> Option<PathBuf> {
    let exe = module_dir()?.join("maxrar.exe");
    exe.is_file().then_some(exe)
}

fn selection(items: Option<&IShellItemArray>) -> Vec<PathBuf> {
    let Some(items) = items else {
        return Vec::new();
    };
    let count = unsafe { items.GetCount() }.unwrap_or(0) as usize;
    if count == 0 || count > crate::MAX_SOURCES {
        return Vec::new();
    }
    let mut paths = Vec::with_capacity(count);
    for i in 0..count {
        let Ok(item) = (unsafe { items.GetItemAt(i as u32) }) else {
            return Vec::new();
        };
        // Virtual items (no file-system path) make the whole selection unsupported.
        let Ok(name) = (unsafe { item.GetDisplayName(SIGDN_FILESYSPATH) }) else {
            return Vec::new();
        };
        let path = unsafe { PathBuf::from(OsString::from_wide(name.as_wide())) };
        unsafe { CoTaskMemFree(Some(name.0 as *const c_void)) };
        paths.push(path);
    }
    paths
}

fn icon() -> Result<PWSTR> {
    match app_exe() {
        Some(exe) => wide(&format!("{},0", exe.display())),
        None => Err(E_NOTIMPL.into()),
    }
}

#[implement(IExplorerCommand)]
struct RootCommand {
    _live: Live,
}

impl IExplorerCommand_Impl for RootCommand_Impl {
    fn GetTitle(&self, _items: Ref<IShellItemArray>) -> Result<PWSTR> {
        wide("MaxRAR")
    }
    fn GetIcon(&self, _items: Ref<IShellItemArray>) -> Result<PWSTR> {
        icon()
    }
    fn GetToolTip(&self, _items: Ref<IShellItemArray>) -> Result<PWSTR> {
        Err(E_NOTIMPL.into())
    }
    fn GetCanonicalName(&self) -> Result<GUID> {
        Ok(CLSID)
    }
    fn GetState(&self, items: Ref<IShellItemArray>, _slow: BOOL) -> Result<u32> {
        let visible =
            app_exe().is_some() && !visible_actions(&selection(items.as_ref())).is_empty();
        Ok(if visible {
            ECS_ENABLED.0 as u32
        } else {
            ECS_HIDDEN.0 as u32
        })
    }
    fn Invoke(&self, _items: Ref<IShellItemArray>, _ctx: Ref<IBindCtx>) -> Result<()> {
        Err(E_NOTIMPL.into())
    }
    fn GetFlags(&self) -> Result<u32> {
        Ok(ECF_HASSUBCOMMANDS.0 as u32)
    }
    fn EnumSubCommands(&self) -> Result<IEnumExplorerCommand> {
        Ok(SubCommands {
            next: std::cell::Cell::new(0),
            _live: Live::new(),
        }
        .into())
    }
}

#[implement(IExplorerCommand)]
struct ActionCommand {
    action: Action,
    _live: Live,
}

impl IExplorerCommand_Impl for ActionCommand_Impl {
    fn GetTitle(&self, _items: Ref<IShellItemArray>) -> Result<PWSTR> {
        wide(self.action.title())
    }
    fn GetIcon(&self, _items: Ref<IShellItemArray>) -> Result<PWSTR> {
        Err(E_NOTIMPL.into())
    }
    fn GetToolTip(&self, _items: Ref<IShellItemArray>) -> Result<PWSTR> {
        Err(E_NOTIMPL.into())
    }
    fn GetCanonicalName(&self) -> Result<GUID> {
        Ok(GUID::from_u128(CLSID.to_u128() + 1 + self.action as u128))
    }
    fn GetState(&self, items: Ref<IShellItemArray>, _slow: BOOL) -> Result<u32> {
        let ok = self.action.applies(&selection(items.as_ref()));
        Ok(if ok {
            ECS_ENABLED.0 as u32
        } else {
            ECS_HIDDEN.0 as u32
        })
    }
    fn Invoke(&self, items: Ref<IShellItemArray>, _ctx: Ref<IBindCtx>) -> Result<()> {
        let paths = selection(items.as_ref());
        if !self.action.applies(&paths) {
            return Ok(());
        }
        let Some(exe) = app_exe() else {
            return Err(E_POINTER.into());
        };
        std::process::Command::new(exe)
            .args(self.action.args(&paths))
            .spawn()
            .map(drop)
            .map_err(|e| {
                windows::core::Error::from_hresult(HRESULT::from_win32(
                    e.raw_os_error().unwrap_or(1) as u32,
                ))
            })
    }
    fn GetFlags(&self) -> Result<u32> {
        Ok(ECF_DEFAULT.0 as u32)
    }
    fn EnumSubCommands(&self) -> Result<IEnumExplorerCommand> {
        Err(E_NOTIMPL.into())
    }
}

#[implement(IEnumExplorerCommand)]
struct SubCommands {
    next: std::cell::Cell<usize>,
    _live: Live,
}

impl IEnumExplorerCommand_Impl for SubCommands_Impl {
    fn Next(&self, celt: u32, out: *mut Option<IExplorerCommand>, fetched: *mut u32) -> HRESULT {
        if out.is_null() {
            return E_POINTER;
        }
        let mut n = 0u32;
        while n < celt && self.next.get() < Action::ALL.len() {
            let action = Action::ALL[self.next.get()];
            self.next.set(self.next.get() + 1);
            let cmd: IExplorerCommand = ActionCommand {
                action,
                _live: Live::new(),
            }
            .into();
            unsafe { out.add(n as usize).write(Some(cmd)) };
            n += 1;
        }
        if !fetched.is_null() {
            unsafe { fetched.write(n) };
        }
        if n == celt {
            S_OK
        } else {
            S_FALSE
        }
    }
    fn Skip(&self, celt: u32) -> Result<()> {
        self.next
            .set((self.next.get() + celt as usize).min(Action::ALL.len()));
        Ok(())
    }
    fn Reset(&self) -> Result<()> {
        self.next.set(0);
        Ok(())
    }
    fn Clone(&self) -> Result<IEnumExplorerCommand> {
        Ok(SubCommands {
            next: std::cell::Cell::new(self.next.get()),
            _live: Live::new(),
        }
        .into())
    }
}

#[implement(IClassFactory)]
struct Factory;

impl IClassFactory_Impl for Factory_Impl {
    fn CreateInstance(
        &self,
        outer: Ref<windows::core::IUnknown>,
        iid: *const GUID,
        object: *mut *mut c_void,
    ) -> Result<()> {
        if object.is_null() || iid.is_null() {
            return Err(E_POINTER.into());
        }
        unsafe { *object = std::ptr::null_mut() };
        if outer.is_some() {
            return Err(CLASS_E_NOAGGREGATION.into());
        }
        let root: IExplorerCommand = RootCommand { _live: Live::new() }.into();
        unsafe { root.query(iid, object).ok() }
    }
    fn LockServer(&self, lock: BOOL) -> Result<()> {
        if lock.as_bool() {
            OBJECTS.fetch_add(1, Ordering::SeqCst);
        } else {
            OBJECTS.fetch_sub(1, Ordering::SeqCst);
        }
        Ok(())
    }
}

/// # Safety
/// Called by COM with valid out pointers.
#[no_mangle]
pub unsafe extern "system" fn DllGetClassObject(
    clsid: *const GUID,
    iid: *const GUID,
    object: *mut *mut c_void,
) -> HRESULT {
    if clsid.is_null() || iid.is_null() || object.is_null() {
        return E_POINTER;
    }
    unsafe { *object = std::ptr::null_mut() };
    if unsafe { *clsid } != CLSID {
        return CLASS_E_CLASSNOTAVAILABLE;
    }
    let factory: IClassFactory = Factory.into();
    unsafe { factory.query(iid, object) }
}

#[no_mangle]
pub extern "system" fn DllCanUnloadNow() -> HRESULT {
    if OBJECTS.load(Ordering::SeqCst) == 0 {
        S_OK
    } else {
        S_FALSE
    }
}
