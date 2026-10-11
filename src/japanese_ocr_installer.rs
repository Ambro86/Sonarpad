//! Explicit, accessible installation of the optional Windows Japanese OCR language.
//! Runs only after the Tools-menu action AND the user's confirmation.
//! Uses the Windows Add-WindowsCapability cmdlet, as the original successful installer did.
//! The elevated process writes progress events to a uniquely named temp file,
//! which the normal Sonarpad UI reads without freezing its message loop.

use crate::app_windows::podcast_save_window;
use crate::settings::Language;
use base64::Engine;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{LazyLock, Mutex};
use std::time::{Duration, Instant};
use windows::Win32::Foundation::{CloseHandle, HANDLE, HWND, LPARAM, WAIT_OBJECT_0, WPARAM};
use windows::Win32::System::Threading::WaitForSingleObject;
use windows::Win32::UI::Shell::{SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW, ShellExecuteExW};
use windows::Win32::UI::WindowsAndMessaging::SW_HIDE;
use windows::core::{PCWSTR, w};

struct InstallSession {
    dialog: isize,
    cancel_path: PathBuf,
    cancel_requested: bool,
    last_percent: Option<u32>,
}

static SESSION: LazyLock<Mutex<Option<InstallSession>>> = LazyLock::new(|| Mutex::new(None));

pub(crate) enum InstallEvent {
    Percent(u32),
    Cancelled,
    Finished {
        success: bool,
        reboot_required: bool,
        error: Option<String>,
    },
}

fn post_event(hwnd_value: isize, event: InstallEvent) {
    let payload = Box::into_raw(Box::new(event));
    let sent = unsafe {
        windows::Win32::UI::WindowsAndMessaging::PostMessageW(
            HWND(hwnd_value),
            crate::WM_JAPANESE_OCR_INSTALL_UPDATE,
            WPARAM(0),
            LPARAM(payload as isize),
        )
    };
    if sent.is_err() {
        unsafe {
            let _unused = Box::from_raw(payload);
        }
    }
}

// No PDF content or arbitrary user input is injected in this command. The paths
// are random private status/cancellation markers in the user temporary directory.
// An elevated job keeps Add-WindowsCapability identical to the previously working
// PowerShell installation while allowing the parent to signal cancellation.
fn script_for_installation(status_path: &Path, cancel_path: &Path) -> String {
    const SCRIPT: &str = r#"
$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'
$statusPath = '__STATUS_PATH__'
$cancelPath = '__CANCEL_PATH__'
$capability = 'Language.OCR~~~ja-JP~0.0.1.0'
$utf8 = New-Object System.Text.UTF8Encoding($false)
function Emit([string]$line) {
    [System.IO.File]::AppendAllText($statusPath, ($line + "`n"), $utf8)
}
$job = $null
try {
    Emit 'START'
    $present = Get-WindowsCapability -Online -Name $capability -ErrorAction Stop
    if ($present.State -eq 'Installed') { Emit 'DONE:OK'; return }
    if ($present.State -eq 'InstallPending') { Emit 'DONE:RESTART'; return }
    if ([System.IO.File]::Exists($cancelPath)) { Emit 'CANCELLED'; return }
    $job = Start-Job -ScriptBlock {
        param([string]$name)
        $ErrorActionPreference = 'Stop'
        # Keep the cmdlet's progress stream enabled inside the background job.
        # The outer elevated PowerShell window stays hidden and has no UI.
        $ProgressPreference = 'Continue'
        Add-WindowsCapability -Online -Name $name -ErrorAction Stop
    } -ArgumentList $capability -ErrorAction Stop
    # The Windows PowerShell job stores ProgressRecord items separately from
    # normal output. Reading only new records preserves cancellation and
    # reports percentages from the actual Windows servicing operation.
    $script:seenProgress = 0
    $script:lastProgress = -1
    function Relay-JobProgress {
        if ($null -eq $job -or $job.ChildJobs.Count -eq 0) { return }
        try {
            $stream = $job.ChildJobs[0].Progress
            if ($null -eq $stream) { return }
            while ($script:seenProgress -lt $stream.Count) {
                $record = $stream[$script:seenProgress]
                $script:seenProgress++
                if ($null -eq $record) { continue }
                $percentage = [int]$record.PercentComplete
                # -1 means indeterminate. Hold 100 until the completed
                # capability has actually been verified by Windows.
                if ($percentage -ge 0 -and $percentage -lt 100 -and
                    $percentage -gt $script:lastProgress) {
                    Emit ('P:' + $percentage.ToString([System.Globalization.CultureInfo]::InvariantCulture))
                    $script:lastProgress = $percentage
                }
            }
        } catch {
            # Progress reporting must never prevent installation or cancellation.
        }
    }
    while ($job.State -eq 'Running' -or $job.State -eq 'NotStarted') {
        Relay-JobProgress
        if ([System.IO.File]::Exists($cancelPath)) {
            Stop-Job -Job $job -ErrorAction SilentlyContinue
            Emit 'CANCELLED'
            return
        }
        $null = Wait-Job -Job $job -Timeout 1
    }
    if ([System.IO.File]::Exists($cancelPath)) { Emit 'CANCELLED'; return }
    Relay-JobProgress
    $result = @(Receive-Job -Job $job -ErrorAction Stop)
    $installed = Get-WindowsCapability -Online -Name $capability -ErrorAction Stop
    if ($installed.State -ne 'Installed' -and $installed.State -ne 'InstallPending') {
        throw ('Windows reported OCR capability state: ' + $installed.State)
    }
    $restart = ($installed.State -eq 'InstallPending')
    foreach ($item in $result) {
        if ($null -ne $item -and $null -ne $item.PSObject.Properties['RestartNeeded']) {
            if ($item.RestartNeeded -eq $true) { $restart = $true }
        }
    }
    # Send 100% only after the capability state has been verified.
    Emit 'P:100'
    if ($restart) { Emit 'DONE:RESTART' } else { Emit 'DONE:OK' }
} catch {
    $reason = $_.Exception.Message.Replace("`n", ' ').Replace("`r", ' ')
    Emit ('FAILED:' + $reason)
} finally {
    if ($null -ne $job) {
        if ($job.State -eq 'Running' -or $job.State -eq 'NotStarted') {
            Stop-Job -Job $job -ErrorAction SilentlyContinue
        }
        Remove-Job -Job $job -Force -ErrorAction SilentlyContinue
    }
}
"#;
    SCRIPT
        .replace(
            "__STATUS_PATH__",
            &status_path.to_string_lossy().replace('\'', "''"),
        )
        .replace(
            "__CANCEL_PATH__",
            &cancel_path.to_string_lossy().replace('\'', "''"),
        )
}

fn encoded_script(script: &str) -> String {
    let utf16: Vec<u8> = script.encode_utf16().flat_map(u16::to_le_bytes).collect();
    base64::engine::general_purpose::STANDARD.encode(utf16)
}

fn read_new_events(text: &str, already_seen: &mut usize) -> Vec<InstallEvent> {
    // Parse only complete lines. This works across partial writes by the elevated process.
    let mut results = Vec::new();
    let bytes = text.as_bytes();
    if *already_seen > bytes.len() {
        *already_seen = 0;
    }
    let tail = &text[*already_seen..];
    let mut complete = 0;
    for row in tail.split_inclusive('\n') {
        if !row.ends_with('\n') {
            break;
        }
        complete += row.len();
        let row = row.trim_matches(|c: char| c == '\r' || c == '\n' || c == '\u{feff}');
        if let Some(raw) = row.strip_prefix("P:") {
            if let Ok(pct) = raw.parse::<u32>()
                && pct <= 100
            {
                results.push(InstallEvent::Percent(pct));
            }
        } else if row == "CANCELLED" {
            results.push(InstallEvent::Cancelled);
        } else if let Some(raw) = row.strip_prefix("DONE:") {
            results.push(InstallEvent::Finished {
                success: true,
                reboot_required: raw == "RESTART" || raw == "3010",
                error: None,
            });
        } else if let Some(raw) = row.strip_prefix("FAILED:") {
            results.push(InstallEvent::Finished {
                success: false,
                reboot_required: false,
                error: Some(raw.to_string()),
            });
        }
    }
    *already_seen += complete;
    results
}

pub(crate) fn start(hwnd: HWND, language: Language) {
    // A second click cannot start a competing system capability installation.
    {
        let Ok(slot) = SESSION.lock() else {
            return;
        };
        if slot.is_some() {
            return;
        }
    }
    let filename = format!(
        "sonarpad-ja-ocr-{}-{}.status",
        std::process::id(),
        uuid::Uuid::new_v4()
    );
    let status_path = std::env::temp_dir().join(filename);
    let cancel_path = status_path.with_extension("cancel");
    if let Err(err) = fs::write(&status_path, "") {
        crate::log_debug(&format!(
            "Japanese OCR: cannot create private progress file: {err}"
        ));
        crate::show_error(
            hwnd,
            language,
            &crate::i18n::tr(language, "app.pdf_japanese_ocr_install_failed"),
        );
        return;
    }

    let script = script_for_installation(&status_path, &cancel_path);
    let params = format!(
        "-NoLogo -NoProfile -WindowStyle Hidden -EncodedCommand {}",
        encoded_script(&script)
    );
    let params_w = crate::accessibility::to_wide(&params);
    let mut exec = SHELLEXECUTEINFOW {
        cbSize: std::mem::size_of::<SHELLEXECUTEINFOW>() as u32,
        fMask: SEE_MASK_NOCLOSEPROCESS,
        hwnd,
        lpVerb: w!("runas"),
        lpFile: w!("powershell.exe"),
        lpParameters: PCWSTR(params_w.as_ptr()),
        nShow: SW_HIDE.0,
        ..Default::default()
    };
    // First request elevation. If UAC is declined, don't show a waiting dialog.
    if let Err(err) = unsafe { ShellExecuteExW(&mut exec) } {
        crate::log_debug(&format!(
            "Japanese OCR: user declined elevation or launch failed: {err}"
        ));
        let _remove_result = fs::remove_file(&status_path);
        crate::show_error(
            hwnd,
            language,
            &crate::i18n::tr(language, "app.pdf_japanese_ocr_install_failed"),
        );
        return;
    }
    if exec.hProcess.0 == 0 {
        crate::log_debug("Japanese OCR: elevated installer launched without a process handle");
        let _remove_result = fs::remove_file(&status_path);
        crate::show_error(
            hwnd,
            language,
            &crate::i18n::tr(language, "app.pdf_japanese_ocr_install_failed"),
        );
        return;
    }
    let labels = podcast_save_window::SaveDialogLabels {
        title: crate::i18n::tr(language, "app.pdf_japanese_ocr_install_title"),
        in_progress: crate::i18n::tr(language, "app.pdf_japanese_ocr_install_progress"),
        cancel: crate::i18n::tr(language, "podcast.save.cancel"),
        cancel_confirm: crate::i18n::tr(language, "app.pdf_japanese_ocr_install_cancel_confirm"),
    };
    // Same uncluttered progress window as MP3 re-export: static status label,
    // native progress bar and Cancel. No keyboard-focusable read-only Edit.
    let dialog = podcast_save_window::open_with_labels(hwnd, language, labels, true);
    if dialog.0 == 0 {
        let _cancel_result = fs::write(&cancel_path, "cancel");
        crate::log_debug(
            "Japanese OCR: accessible progress window creation failed; cancellation requested",
        );
        crate::show_error(
            hwnd,
            language,
            &crate::i18n::tr(language, "app.pdf_japanese_ocr_install_error"),
        );
    } else {
        podcast_save_window::show_indeterminate_progress(dialog);
        podcast_save_window::enable_accessible_progress_context(dialog);
        podcast_save_window::restrict_tab_to_cancel(dialog);
    }
    {
        let Ok(mut slot) = SESSION.lock() else {
            return;
        };
        *slot = Some(InstallSession {
            dialog: dialog.0,
            cancel_path: cancel_path.clone(),
            cancel_requested: false,
            last_percent: None,
        });
    }
    if dialog.0 != 0 {
        // Focus and modal tracking may re-enter the UI; release SESSION first.
        crate::bring_modal_window_to_foreground(dialog);
    }
    crate::log_debug(
        "Japanese OCR: started elevated PowerShell Add-WindowsCapability job with modal cancelable progress",
    );

    let hwnd_value = hwnd.0;
    let process_value = exec.hProcess.0;
    std::thread::spawn(move || {
        let handle = HANDLE(process_value);
        let mut position = 0;
        let mut ended = false;
        let mut saw_finish = false;
        let started = Instant::now();
        while !saw_finish && started.elapsed() < Duration::from_secs(7200) {
            std::thread::sleep(Duration::from_millis(300));
            if let Ok(text) = fs::read_to_string(&status_path) {
                for event in read_new_events(&text, &mut position) {
                    if matches!(
                        event,
                        InstallEvent::Finished { .. } | InstallEvent::Cancelled
                    ) {
                        saw_finish = true;
                    }
                    post_event(hwnd_value, event);
                }
            }
            if saw_finish {
                break;
            }
            if unsafe { WaitForSingleObject(handle, 0) } == WAIT_OBJECT_0 {
                if ended {
                    break;
                }
                ended = true; // grace period for the final file write
            }
        }
        if !saw_finish {
            post_event(
                hwnd_value,
                InstallEvent::Finished {
                    success: false,
                    reboot_required: false,
                    error: Some(
                        "The Windows component installer stopped without a result, or timed out."
                            .to_string(),
                    ),
                },
            );
        }
        unsafe {
            let _close_result = CloseHandle(handle);
        }
        let _remove_result = fs::remove_file(&status_path);
        let _remove_cancel_result = fs::remove_file(&cancel_path);
    });
}

/// Called only for this installer's own modal progress dialog.
/// The elevated PowerShell job observes the cancellation marker and stops itself.
pub(crate) fn cancel(hwnd: HWND, dialog: HWND, language: Language) -> bool {
    // The session lock is released at the end of this block, before any UI call.
    let cancel_write_failed = {
        let Ok(mut slot) = SESSION.lock() else {
            return false;
        };
        let Some(session) = slot.as_mut() else {
            return false;
        };
        if session.dialog == 0 || session.dialog != dialog.0 {
            return false;
        }
        if session.cancel_requested {
            return true;
        }
        session.cancel_requested = true;
        match fs::write(&session.cancel_path, "cancel") {
            Ok(()) => false,
            Err(err) => {
                crate::log_debug(&format!(
                    "Japanese OCR: could not request cancellation: {err}"
                ));
                session.cancel_requested = false;
                true
            }
        }
    };
    if cancel_write_failed {
        crate::show_error(
            hwnd,
            language,
            &crate::i18n::tr(language, "app.pdf_japanese_ocr_install_error"),
        );
        return true;
    }
    podcast_save_window::set_status_text(
        dialog,
        &crate::i18n::tr(language, "app.pdf_japanese_ocr_install_canceling"),
    );
    crate::log_debug("Japanese OCR: cancellation requested; waiting for Windows servicing to stop");
    true
}

pub(crate) fn active_dialog() -> HWND {
    SESSION
        .lock()
        .ok()
        .and_then(|slot| slot.as_ref().map(|session| HWND(session.dialog)))
        .unwrap_or(HWND(0))
}

pub(crate) fn handle_update(hwnd: HWND, lparam: LPARAM) {
    if lparam.0 == 0 {
        return;
    }
    let event = unsafe { *Box::from_raw(lparam.0 as *mut InstallEvent) };
    let language = crate::with_state(hwnd, |state| state.settings.language).unwrap_or_default();
    // Progress updates are handled while holding the session lock; terminal events
    // clear the session and release the lock at the end of this block before any UI call.
    let dialog = {
        let Ok(mut slot) = SESSION.lock() else {
            return;
        };
        let Some(session) = slot.as_mut() else {
            return;
        };
        let dialog = HWND(session.dialog);
        if let InstallEvent::Percent(pct) = event {
            if session.cancel_requested || session.last_percent.is_some_and(|last| pct <= last) {
                return;
            }
            session.last_percent = Some(pct);
            if dialog.0 != 0 {
                crate::send_message_w_safe(
                    dialog,
                    podcast_save_window::WM_PODCAST_SAVE_PROGRESS,
                    WPARAM(pct as usize),
                    LPARAM(0),
                );
                // Speak at most every 10 percent to avoid interrupting NVDA constantly.
                if pct == 100 || pct % 10 == 0 {
                    let status = crate::i18n::tr(language, "app.pdf_japanese_ocr_install_progress");
                    crate::screen_reader_speak(&format!("{status} {pct}%"));
                }
            }
            return;
        }
        *slot = None;
        dialog
    };
    match event {
        InstallEvent::Percent(_) => {}
        InstallEvent::Cancelled => {
            if dialog.0 != 0 {
                crate::send_message_w_safe(
                    dialog,
                    podcast_save_window::WM_PODCAST_SAVE_DONE,
                    WPARAM(0),
                    LPARAM(0),
                );
            }
            crate::show_info(
                hwnd,
                language,
                &crate::i18n::tr(language, "app.pdf_japanese_ocr_install_cancelled"),
            );
        }
        InstallEvent::Finished {
            success,
            reboot_required,
            error,
        } => {
            if dialog.0 != 0 {
                crate::send_message_w_safe(
                    dialog,
                    podcast_save_window::WM_PODCAST_SAVE_DONE,
                    WPARAM(0),
                    LPARAM(0),
                );
            }
            if success {
                let key = if reboot_required {
                    "app.pdf_japanese_ocr_install_restart"
                } else {
                    "app.pdf_japanese_ocr_install_success"
                };
                crate::show_info(hwnd, language, &crate::i18n::tr(language, key));
            } else {
                let message = crate::i18n::tr(language, "app.pdf_japanese_ocr_install_error");
                crate::log_debug(&format!(
                    "Japanese OCR: Windows installation failed: {}",
                    error.unwrap_or_else(|| "unknown error".to_string())
                ));
                crate::show_error(hwnd, language, &message);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{InstallEvent, encoded_script, read_new_events, script_for_installation};
    use std::path::Path;

    #[test]
    fn parser_reads_only_completed_lines() {
        let mut cursor = 0;
        let first = read_new_events("START\nP:10\nP:2", &mut cursor);
        assert_eq!(first.len(), 1);
        assert!(matches!(first[0], InstallEvent::Percent(10)));
        let second = read_new_events("START\nP:10\nP:20\nDONE:RESTART\n", &mut cursor);
        assert!(matches!(second[0], InstallEvent::Percent(20)));
        assert!(matches!(
            second[1],
            InstallEvent::Finished {
                success: true,
                reboot_required: true,
                ..
            }
        ));
        let third = read_new_events("START\nP:10\nP:20\nDONE:RESTART\nFAILED:87\n", &mut cursor);
        assert!(matches!(
            third[0],
            InstallEvent::Finished { success: false, .. }
        ));
        let mut cursor = 0;
        let cancelled = read_new_events("START\nCANCELLED\n", &mut cursor);
        assert!(matches!(cancelled[0], InstallEvent::Cancelled));
    }
    #[test]
    fn script_uses_only_official_fixed_capability() {
        let script = script_for_installation(
            Path::new(r"C:\Temp\some'path.status"),
            Path::new(r"C:\Temp\cancel'signal.cancel"),
        );
        assert!(script.contains("Language.OCR~~~ja-JP~0.0.1.0"));
        assert!(script.contains("some''path.status"));
        assert!(script.contains("cancel''signal.cancel"));
        assert!(script.contains("Add-WindowsCapability -Online -Name"));
        assert!(script.contains("Stop-Job -Job $job"));
        assert!(script.contains("$job.ChildJobs[0].Progress"));
        assert!(script.contains("$record.PercentComplete"));
        assert!(script.contains("$ProgressPreference = 'Continue'"));
        assert!(script.contains("Relay-JobProgress"));
        assert!(script.contains("Emit 'P:100'"));
        assert!(!script.contains("dism.exe"));
        assert!(!encoded_script(&script).is_empty());
    }
}
