//! Making a change visible without signing out.
//!
//! Order of preference, and the reason this trait exists at all: broadcast if we
//! can, notify the shell if we must, restart Explorer only when the user has
//! said yes. Nothing here ever kills Explorer on its own initiative.

use std::ffi::c_void;
use std::os::windows::process::CommandExt;
use std::process::Command;

use windows::Win32::Foundation::{LPARAM, WPARAM};
use windows::Win32::UI::Shell::{SHChangeNotify, SHCNE_ASSOCCHANGED, SHCNF_IDLIST};
use windows::Win32::UI::WindowsAndMessaging::{
    SendMessageTimeoutW, SystemParametersInfoW, HWND_BROADCAST, SMTO_ABORTIFHUNG, SPIF_SENDCHANGE,
    SPIF_UPDATEINIFILE, SPI_SETCURSORS, SPI_SETDESKWALLPAPER, WM_SETTINGCHANGE,
};

use mino_core::error::{Error, Result};
use mino_core::provider::ShellRefresher;

/// Stops a console window flashing up when we shell out.
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

pub struct WindowsShell;

impl WindowsShell {
    pub fn new() -> Self {
        WindowsShell
    }
}

impl Default for WindowsShell {
    fn default() -> Self {
        Self::new()
    }
}

impl ShellRefresher for WindowsShell {
    /// `WM_SETTINGCHANGE` with an area string such as `ImmersiveColorSet`. Apps
    /// that listen re-read their theme immediately; those that do not will pick
    /// it up next time they start.
    fn broadcast_setting_change(&self, area: &str) -> Result<()> {
        let wide: Vec<u16> = area.encode_utf16().chain(std::iter::once(0)).collect();
        unsafe {
            // The timeout matters: a hung window must not hang the app, hence
            // SMTO_ABORTIFHUNG and a short deadline.
            SendMessageTimeoutW(
                HWND_BROADCAST,
                WM_SETTINGCHANGE,
                WPARAM(0),
                LPARAM(wide.as_ptr() as isize),
                SMTO_ABORTIFHUNG,
                250,
                None,
            );
        }
        Ok(())
    }

    fn notify_assoc_changed(&self) -> Result<()> {
        unsafe {
            SHChangeNotify(SHCNE_ASSOCCHANGED, SHCNF_IDLIST, None, None);
        }
        Ok(())
    }

    fn refresh_cursors(&self) -> Result<()> {
        unsafe {
            SystemParametersInfoW(SPI_SETCURSORS, 0, None, SPIF_SENDCHANGE)
                .map_err(|e| Error::shell(format!("reloading cursors: {e}")))
        }
    }

    /// `SPI_SETDESKWALLPAPER` is what actually repaints the desktop. The flags
    /// matter: `UPDATEINIFILE` makes it stick across a sign-out, `SENDCHANGE`
    /// tells everything else it happened.
    fn apply_wallpaper(&self, path: &str) -> Result<()> {
        // Must outlive the call: Windows reads the string through this pointer.
        let mut wide: Vec<u16> = path.encode_utf16().chain(std::iter::once(0)).collect();
        unsafe {
            SystemParametersInfoW(
                SPI_SETDESKWALLPAPER,
                0,
                Some(wide.as_mut_ptr() as *mut c_void),
                SPIF_UPDATEINIFILE | SPIF_SENDCHANGE,
            )
            .map_err(|e| Error::shell(format!("setting the wallpaper to `{path}`: {e}")))
        }
    }

    /// Only ever reached after the user agrees in the UI.
    ///
    /// Explorer is asked to close and then started again. Windows usually
    /// restarts it by itself, so the second command is a safety net for the
    /// cases where it does not — starting a second Explorer is harmless.
    fn restart_explorer(&self) -> Result<()> {
        Command::new("taskkill")
            .args(["/f", "/im", "explorer.exe"])
            .creation_flags(CREATE_NO_WINDOW)
            .status()
            .map_err(|e| Error::shell(format!("stopping Explorer: {e}")))?;

        std::thread::sleep(std::time::Duration::from_millis(600));

        Command::new("explorer.exe")
            .creation_flags(CREATE_NO_WINDOW)
            .spawn()
            .map_err(|e| Error::shell(format!("starting Explorer: {e}")))?;

        Ok(())
    }
}
