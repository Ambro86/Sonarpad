//! Startup must be linked with crt-static so this check can run without VC++.
use std::ffi::c_void;

pub const DOWNLOAD_URL: &str = "https://aka.ms/vs/18/release/14.50.35719/VC_redist.x64.exe";
const LIBRARIES: &[&str] = &["vcruntime140.dll", "vcruntime140_1.dll", "msvcp140.dll"];

#[link(name = "kernel32")]
unsafe extern "system" {
    fn LoadLibraryExW(name: *const u16, file: *mut c_void, flags: u32) -> *mut c_void;
    fn FreeLibrary(module: *mut c_void) -> i32;
    fn SetThreadErrorMode(mode: u32, old_mode: *mut u32) -> i32;
}

#[link(name = "user32")]
unsafe extern "system" {
    fn MessageBoxW(window: *mut c_void, text: *const u16, title: *const u16, flags: u32) -> i32;
}

#[link(name = "shell32")]
unsafe extern "system" {
    fn ShellExecuteW(
        window: *mut c_void,
        operation: *const u16,
        file: *const u16,
        parameters: *const u16,
        directory: *const u16,
        show: i32,
    ) -> isize;
}

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(Some(0)).collect()
}

/// Standard Windows Yes/No dialog, usable with screen readers and keyboard.
/// Only opens the official URL after Yes; never installs or elevates silently.
pub fn offer_download(title: &str, message: &str, open_error: &str) {
    let title = wide(title);
    let message = wide(message);
    unsafe {
        if MessageBoxW(
            std::ptr::null_mut(),
            message.as_ptr(),
            title.as_ptr(),
            0x04 | 0x30,
        ) == 6
        {
            let operation = wide("open");
            let url = wide(DOWNLOAD_URL);
            if ShellExecuteW(
                std::ptr::null_mut(),
                operation.as_ptr(),
                url.as_ptr(),
                std::ptr::null(),
                std::ptr::null(),
                1,
            ) <= 32
            {
                let error = wide(&format!("{open_error}\n\n{DOWNLOAD_URL}"));
                MessageBoxW(std::ptr::null_mut(), error.as_ptr(), title.as_ptr(), 0x10);
            }
        }
    }
}

fn missing_with(mut available: impl FnMut(&str) -> bool) -> Vec<&'static str> {
    LIBRARIES
        .iter()
        .copied()
        .filter(|name| !available(name))
        .collect()
}

/// Check real loadability, including architecture and dependent libraries,
/// rather than trusting a registry entry left behind by a broken installation.
pub fn missing_libraries() -> Vec<&'static str> {
    missing_with(|name| {
        let name: Vec<u16> = name.encode_utf16().chain(Some(0)).collect();
        // Limit the search to the executable directory and Windows System32.
        // Suppress the loader's own inaccessible/error dialogs on this thread.
        unsafe {
            let mut old_mode = 0;
            let changed = SetThreadErrorMode(0x0001 | 0x8000, &mut old_mode) != 0;
            let module = LoadLibraryExW(name.as_ptr(), std::ptr::null_mut(), 0x0200 | 0x0800);
            if !module.is_null() {
                FreeLibrary(module);
            }
            if changed {
                SetThreadErrorMode(old_mode, std::ptr::null_mut());
            }
            !module.is_null()
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn installed_runtime_does_not_prompt() {
        assert!(missing_with(|_| true).is_empty());
    }

    #[test]
    fn absent_runtime_reports_all_required_libraries() {
        assert_eq!(missing_with(|_| false), LIBRARIES);
    }

    #[test]
    fn partial_runtime_is_not_mistaken_for_a_complete_installation() {
        assert_eq!(
            missing_with(|name| name != "vcruntime140_1.dll"),
            ["vcruntime140_1.dll"]
        );
    }
}
