//! Asks the registered MaxRAR menu what Explorer would show for each path given.
//! Usage: cargo run -p maxrar-shell --example probe -- <path>...
#[cfg(windows)]
fn main() -> windows::core::Result<()> {
    use windows::core::{GUID, HSTRING};
    use windows::Win32::System::Com::*;
    use windows::Win32::UI::Shell::*;
    unsafe {
        CoInitializeEx(None, COINIT_APARTMENTTHREADED).ok()?;
        let clsid = GUID::from_u128(0x94ae9828_d27b_4b7d_9a32_c2e17040f695);
        let root: IExplorerCommand = CoCreateInstance(&clsid, None, CLSCTX_LOCAL_SERVER)?;
        for arg in std::env::args().skip(1) {
            let paths: Vec<&str> = arg.split('|').collect();
            let items: Vec<IShellItem> = paths
                .iter()
                .map(|p| SHCreateItemFromParsingName(&HSTRING::from(*p), None))
                .collect::<windows::core::Result<_>>()?;
            let pidls: Vec<_> = items
                .iter()
                .map(|i| SHGetIDListFromObject(i))
                .collect::<windows::core::Result<_>>()?;
            let ptrs: Vec<_> = pidls.iter().map(|p| *p as *const _).collect();
            let array = SHCreateShellItemArrayFromIDLists(&ptrs)?;
            let state = root.GetState(&array, false)?;
            println!(
                "{arg}\n  root: {} state={state}",
                root.GetTitle(&array)?.to_string()?
            );
            let subs = root.EnumSubCommands()?;
            loop {
                let mut cmd = [None];
                let mut n = 0;
                if subs.Next(&mut cmd, Some(&mut n)).is_err() || n == 0 {
                    break;
                }
                let cmd = cmd[0].take().unwrap();
                let s = cmd.GetState(&array, false)?;
                println!(
                    "  - {} {}",
                    cmd.GetTitle(&array)?.to_string()?,
                    if s == ECS_HIDDEN.0 as u32 {
                        "(hidden)"
                    } else {
                        ""
                    }
                );
            }
        }
    }
    Ok(())
}
#[cfg(not(windows))]
fn main() {}
