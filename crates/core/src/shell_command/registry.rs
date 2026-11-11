//! Windows: the folder of Weaver's copy of the command line is an entry of the user's `Path`, the
//! value `Path` of `HKEY_CURRENT_USER\Environment`.
//!
//! The value keeps its type (`REG_EXPAND_SZ` stays expandable) and every other entry; removing
//! deletes only the entry of the folder. After a change, `WM_SETTINGCHANGE` with `Environment` is
//! broadcast so that the terminals opened afterwards read the new `Path`. The `Path` of the
//! machine is never read or written.

use std::io;
use std::path::Path;
use std::ptr::{null, null_mut};

use windows_sys::Win32::Foundation::ERROR_SUCCESS;
use windows_sys::Win32::System::Registry::{
    RegCloseKey, RegOpenKeyExW, RegQueryValueExW, RegSetValueExW, HKEY, HKEY_CURRENT_USER,
    KEY_QUERY_VALUE, KEY_SET_VALUE, REG_EXPAND_SZ, REG_SZ,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    SendMessageTimeoutW, HWND_BROADCAST, SMTO_ABORTIFHUNG, WM_SETTINGCHANGE,
};

use super::user_path;
use super::{ShellCommandOutcome, ShellCommandResult};

/// `ERROR_FILE_NOT_FOUND`: the key or the value does not exist.
const NOT_FOUND: u32 = 2;
/// `ERROR_MORE_DATA`: the buffer was too small.
const MORE_DATA: u32 = 234;
/// How long a window may take to answer the broadcast, in milliseconds.
const BROADCAST_TIMEOUT_MS: u32 = 5000;

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}

fn error(code: u32, what: &str) -> io::Error {
    io::Error::other(format!(
        "{what}: {}",
        io::Error::from_raw_os_error(code as i32)
    ))
}

/// The value `Path` of the user's environment: its text and its type.
struct UserPath {
    text: String,
    kind: u32,
}

/// `HKEY_CURRENT_USER\Environment`, open for reading and writing a value, closed on drop.
struct Environment(HKEY);

impl Environment {
    fn open() -> io::Result<Self> {
        let mut key: HKEY = null_mut();
        let name = wide("Environment");
        // SAFETY: `name` is a NUL-terminated UTF-16 string that outlives the call, and `key` is
        // a valid place for the handle.
        let code = unsafe {
            RegOpenKeyExW(
                HKEY_CURRENT_USER,
                name.as_ptr(),
                0,
                KEY_QUERY_VALUE | KEY_SET_VALUE,
                &mut key,
            )
        };
        if code == ERROR_SUCCESS {
            Ok(Self(key))
        } else {
            Err(error(code, "cannot open the user's environment"))
        }
    }

    /// The user's `Path`, `None` when the value does not exist.
    fn read(&self) -> io::Result<Option<UserPath>> {
        let name = wide("Path");
        let mut kind = 0u32;
        let mut size = 0u32;
        // SAFETY: the size is asked first, with no data buffer; all pointers are valid.
        let mut code = unsafe {
            RegQueryValueExW(self.0, name.as_ptr(), null(), &mut kind, null_mut(), &mut size)
        };
        if code == NOT_FOUND {
            return Ok(None);
        }
        let mut data: Vec<u16> = Vec::new();
        // The value may grow between the two calls: ask again while the buffer is too small.
        while code == ERROR_SUCCESS || code == MORE_DATA {
            data.resize((size as usize).div_ceil(2) + 1, 0);
            let mut capacity = (data.len() * 2) as u32;
            // SAFETY: `data` holds `capacity` bytes and outlives the call.
            code = unsafe {
                RegQueryValueExW(
                    self.0,
                    name.as_ptr(),
                    null(),
                    &mut kind,
                    data.as_mut_ptr().cast::<u8>(),
                    &mut capacity,
                )
            };
            if code == ERROR_SUCCESS {
                data.truncate((capacity as usize) / 2);
                break;
            }
            if code == MORE_DATA {
                size = capacity;
            }
        }
        if code != ERROR_SUCCESS {
            return Err(error(code, "cannot read the user's Path"));
        }
        if kind != REG_SZ && kind != REG_EXPAND_SZ {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "the user's Path is not a text value",
            ));
        }
        while data.last() == Some(&0) {
            data.pop();
        }
        Ok(Some(UserPath {
            text: String::from_utf16_lossy(&data),
            kind,
        }))
    }

    fn write(&self, text: &str, kind: u32) -> io::Result<()> {
        let name = wide("Path");
        let data = wide(text);
        // SAFETY: `data` is a NUL-terminated UTF-16 string whose byte length is passed, and it
        // outlives the call.
        let code = unsafe {
            RegSetValueExW(
                self.0,
                name.as_ptr(),
                0,
                kind,
                data.as_ptr().cast::<u8>(),
                (data.len() * 2) as u32,
            )
        };
        if code == ERROR_SUCCESS {
            Ok(())
        } else {
            Err(error(code, "cannot write the user's Path"))
        }
    }
}

impl Drop for Environment {
    fn drop(&mut self) {
        // SAFETY: the handle was opened by `open` and is closed once.
        unsafe { RegCloseKey(self.0) };
    }
}

/// Tells the running programs that the environment changed, so that the terminals opened
/// afterwards read it. A program that does not answer in time is not waited for.
fn broadcast_environment_change() {
    let area = wide("Environment");
    let mut result = 0usize;
    // SAFETY: `area` is a NUL-terminated UTF-16 string that outlives the call; the broadcast
    // is synchronous with a time limit for each window.
    unsafe {
        SendMessageTimeoutW(
            HWND_BROADCAST,
            WM_SETTINGCHANGE,
            0,
            area.as_ptr() as isize,
            SMTO_ABORTIFHUNG,
            BROADCAST_TIMEOUT_MS,
            &mut result,
        );
    }
}

/// `%NAME%` as the environment of this process has it, case-insensitively as Windows does.
fn expand(text: &str) -> String {
    user_path::expand_variables(text, &|name| {
        std::env::var_os(name).map(|value| value.to_string_lossy().into_owned())
    })
}

fn folder(copy: &Path) -> String {
    copy.parent().unwrap_or(copy).display().to_string()
}

fn result(installed: bool, place: String, outcome: ShellCommandOutcome) -> ShellCommandResult {
    ShellCommandResult {
        installed,
        place,
        outcome,
        path_warning: false,
    }
}

fn failed(place: String, error: impl std::fmt::Display) -> ShellCommandResult {
    result(
        false,
        place,
        ShellCommandOutcome::Failed {
            reason: error.to_string(),
        },
    )
}

fn holds(path: &Option<UserPath>, dir: &str) -> bool {
    path.as_ref()
        .is_some_and(|path| user_path::contains(&path.text, dir, &expand))
}

/// Reads the user's `Path`; a failure of the system is told as the reason.
fn read_user_path() -> io::Result<(Environment, Option<UserPath>)> {
    let environment = Environment::open()?;
    let path = environment.read()?;
    Ok((environment, path))
}

pub fn status(copy: &Path) -> ShellCommandResult {
    let dir = folder(copy);
    match read_user_path() {
        Ok((_, path)) => result(holds(&path, &dir), dir, ShellCommandOutcome::Done),
        Err(error) => failed(dir, error),
    }
}

pub fn install(copy: &Path) -> ShellCommandResult {
    let dir = folder(copy);
    let (environment, path) = match read_user_path() {
        Ok(read) => read,
        Err(error) => return failed(dir, error),
    };
    if holds(&path, &dir) {
        return result(true, dir, ShellCommandOutcome::Done);
    }
    let (text, kind) = match &path {
        Some(path) => (user_path::with_added(&path.text, &dir), path.kind),
        None => (dir.clone(), REG_EXPAND_SZ),
    };
    if let Err(error) = environment.write(&text, kind) {
        return failed(dir, error);
    }
    broadcast_environment_change();
    result(true, dir, ShellCommandOutcome::Done)
}

pub fn remove(copy: &Path) -> ShellCommandResult {
    let dir = folder(copy);
    let (environment, path) = match read_user_path() {
        Ok(read) => read,
        Err(error) => return failed(dir, error),
    };
    let Some(path) = path.filter(|path| user_path::contains(&path.text, &dir, &expand)) else {
        return result(false, dir, ShellCommandOutcome::Done);
    };
    let text = user_path::without(&path.text, &dir, &expand);
    if let Err(error) = environment.write(&text, path.kind) {
        return result(true, dir, ShellCommandOutcome::Failed { reason: error.to_string() });
    }
    broadcast_environment_change();
    result(false, dir, ShellCommandOutcome::Done)
}
