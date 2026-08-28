//! Registry access through the Win32 API.
//!
//! FIRST-BUILD CHECK: every `unsafe` call below is written against the exact
//! signatures of `windows = "0.58"` (pinned in the workspace manifest). If the
//! crate is bumped, argument shapes such as `uloptions: u32` vs `Option<u32>`
//! are the things that move. This file is deliberately the only place in the
//! project where that matters.

use windows::core::{HSTRING, PCWSTR};
use windows::Win32::Foundation::{ERROR_FILE_NOT_FOUND, ERROR_SUCCESS, WIN32_ERROR};
use windows::Win32::System::Registry::{
    RegCloseKey, RegCreateKeyExW, RegDeleteTreeW, RegDeleteValueW, RegOpenKeyExW, RegQueryValueExW,
    RegSetValueExW, HKEY, HKEY_CLASSES_ROOT, HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_READ,
    KEY_WRITE, REG_BINARY, REG_DWORD, REG_EXPAND_SZ, REG_OPTION_NON_VOLATILE, REG_SAM_FLAGS,
    REG_SZ, REG_VALUE_TYPE,
};

use mino_core::error::{Error, Result};
use mino_core::provider::{Hive, RegLoc, RegValue, RegistryProvider};

/// An owned key handle that closes itself.
struct Key(HKEY);

impl Drop for Key {
    fn drop(&mut self) {
        unsafe {
            let _ = RegCloseKey(self.0);
        }
    }
}

fn root(hive: Hive) -> HKEY {
    match hive {
        Hive::CurrentUser => HKEY_CURRENT_USER,
        Hive::LocalMachine => HKEY_LOCAL_MACHINE,
        Hive::ClassesRoot => HKEY_CLASSES_ROOT,
    }
}

fn fail(what: &str, status: WIN32_ERROR) -> Error {
    let os = std::io::Error::from_raw_os_error(status.0 as i32);
    Error::registry(format!("{what}: {os}"))
}

/// `Ok(None)` means the key is simply not there, which is a normal state and
/// not an error — an unset value is how Windows spells "default".
fn open(hive: Hive, path: &str, access: REG_SAM_FLAGS) -> Result<Option<Key>> {
    let mut handle = HKEY::default();
    let status = unsafe { RegOpenKeyExW(root(hive), &HSTRING::from(path), 0, access, &mut handle) };
    match status {
        s if s == ERROR_SUCCESS => Ok(Some(Key(handle))),
        s if s == ERROR_FILE_NOT_FOUND => Ok(None),
        s => Err(fail(&format!("opening {}\\{path}", hive.short()), s)),
    }
}

fn create(hive: Hive, path: &str) -> Result<Key> {
    let mut handle = HKEY::default();
    let status = unsafe {
        RegCreateKeyExW(
            root(hive),
            &HSTRING::from(path),
            0,
            PCWSTR::null(),
            REG_OPTION_NON_VOLATILE,
            KEY_READ | KEY_WRITE,
            None,
            &mut handle,
            None,
        )
    };
    if status == ERROR_SUCCESS {
        Ok(Key(handle))
    } else {
        Err(fail(&format!("creating {}\\{path}", hive.short()), status))
    }
}

pub struct WindowsRegistry;

impl WindowsRegistry {
    pub fn new() -> Self {
        WindowsRegistry
    }
}

impl Default for WindowsRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl RegistryProvider for WindowsRegistry {
    fn read(&self, loc: &RegLoc) -> Result<Option<RegValue>> {
        let Some(key) = open(loc.hive, &loc.path, KEY_READ)? else {
            return Ok(None);
        };
        let name = HSTRING::from(loc.name.as_str());

        // First call: how big is it, and what type?
        let mut kind = REG_VALUE_TYPE::default();
        let mut size: u32 = 0;
        let status =
            unsafe { RegQueryValueExW(key.0, &name, None, Some(&mut kind), None, Some(&mut size)) };
        match status {
            s if s == ERROR_SUCCESS => {}
            s if s == ERROR_FILE_NOT_FOUND => return Ok(None),
            s => return Err(fail(&format!("reading {loc}"), s)),
        }

        let mut buffer = vec![0u8; size as usize];
        let status = unsafe {
            RegQueryValueExW(
                key.0,
                &name,
                None,
                Some(&mut kind),
                Some(buffer.as_mut_ptr()),
                Some(&mut size),
            )
        };
        if status != ERROR_SUCCESS {
            return Err(fail(&format!("reading {loc}"), status));
        }
        buffer.truncate(size as usize);

        // Compared with `==` rather than matched: these are constants of a
        // newtype, and const patterns on foreign types are a trap.
        let value = if kind == REG_DWORD {
            if buffer.len() < 4 {
                return Err(Error::registry(format!(
                    "{loc}: REG_DWORD shorter than 4 bytes"
                )));
            }
            RegValue::Dword(u32::from_le_bytes([
                buffer[0], buffer[1], buffer[2], buffer[3],
            ]))
        } else if kind == REG_SZ {
            RegValue::Sz(utf16_to_string(&buffer))
        } else if kind == REG_EXPAND_SZ {
            RegValue::ExpandSz(utf16_to_string(&buffer))
        } else if kind == REG_BINARY {
            RegValue::Binary(buffer)
        } else {
            return Err(Error::registry(format!(
                "{loc}: registry type {} is not one this app writes",
                kind.0
            )));
        };
        Ok(Some(value))
    }

    fn write(&self, loc: &RegLoc, value: &RegValue) -> Result<()> {
        // Creating is idempotent and gives us the key chain for free.
        let key = create(loc.hive, &loc.path)?;
        let name = HSTRING::from(loc.name.as_str());

        let (kind, bytes) = match value {
            RegValue::Dword(d) => (REG_DWORD, d.to_le_bytes().to_vec()),
            RegValue::Sz(s) => (REG_SZ, string_to_utf16_bytes(s)),
            RegValue::ExpandSz(s) => (REG_EXPAND_SZ, string_to_utf16_bytes(s)),
            RegValue::Binary(b) => (REG_BINARY, b.clone()),
        };

        // The slice carries its own length here — no separate cbData argument.
        let status = unsafe { RegSetValueExW(key.0, &name, 0, kind, Some(&bytes)) };
        if status == ERROR_SUCCESS {
            Ok(())
        } else {
            Err(fail(&format!("writing {loc}"), status))
        }
    }

    fn delete_value(&self, loc: &RegLoc) -> Result<()> {
        let Some(key) = open(loc.hive, &loc.path, KEY_WRITE)? else {
            return Ok(()); // nothing to delete
        };
        let status = unsafe { RegDeleteValueW(key.0, &HSTRING::from(loc.name.as_str())) };
        match status {
            s if s == ERROR_SUCCESS || s == ERROR_FILE_NOT_FOUND => Ok(()),
            s => Err(fail(&format!("deleting {loc}"), s)),
        }
    }

    fn key_exists(&self, hive: Hive, path: &str) -> Result<bool> {
        Ok(open(hive, path, KEY_READ)?.is_some())
    }

    fn create_key(&self, hive: Hive, path: &str) -> Result<()> {
        create(hive, path).map(|_| ())
    }

    fn delete_key(&self, hive: Hive, path: &str) -> Result<()> {
        let status = unsafe { RegDeleteTreeW(root(hive), &HSTRING::from(path)) };
        match status {
            s if s == ERROR_SUCCESS || s == ERROR_FILE_NOT_FOUND => {
                // RegDeleteTree empties the key; remove the key itself too.
                let status = unsafe {
                    windows::Win32::System::Registry::RegDeleteKeyW(
                        root(hive),
                        &HSTRING::from(path),
                    )
                };
                match status {
                    s if s == ERROR_SUCCESS || s == ERROR_FILE_NOT_FOUND => Ok(()),
                    s => Err(fail(&format!("removing {}\\{path}", hive.short()), s)),
                }
            }
            s => Err(fail(&format!("clearing {}\\{path}", hive.short()), s)),
        }
    }
}

fn utf16_to_string(bytes: &[u8]) -> String {
    let units: Vec<u16> = bytes
        .chunks_exact(2)
        .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
        .collect();
    let end = units.iter().position(|&u| u == 0).unwrap_or(units.len());
    String::from_utf16_lossy(&units[..end])
}

fn string_to_utf16_bytes(text: &str) -> Vec<u8> {
    text.encode_utf16()
        .chain(std::iter::once(0))
        .flat_map(u16::to_le_bytes)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn utf16_round_trip() {
        let bytes = string_to_utf16_bytes("Segoe UI");
        assert_eq!(utf16_to_string(&bytes), "Segoe UI");
    }

    #[test]
    fn utf16_stops_at_the_terminator() {
        let mut bytes = string_to_utf16_bytes("ok");
        bytes.extend_from_slice(&[0x41, 0x00]); // junk after the NUL
        assert_eq!(utf16_to_string(&bytes), "ok");
    }
}
