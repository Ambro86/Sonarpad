use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use windows::Win32::Foundation::{HINSTANCE, HWND, LPARAM, WPARAM};
use windows::Win32::Graphics::Gdi::HFONT;
use windows::Win32::UI::Controls::{WC_BUTTON, WC_COMBOBOXW, WC_STATIC};
use windows::Win32::UI::WindowsAndMessaging::{
    CB_ADDSTRING, CB_GETCURSEL, CB_SETCURSEL, CBS_DROPDOWNLIST, CreateWindowExW, HMENU,
    SendMessageW, SetTimer, WINDOW_STYLE, WM_SETFONT, WS_CHILD, WS_TABSTOP, WS_VISIBLE,
};
use windows::core::PCWSTR;

use crate::accessibility::to_wide;
use crate::audio_description::{
    AudioDescriptionJob, AudioDescriptionMixPreviewCache, normalize_source_volume,
    render_audio_description_mix_preview,
};
use crate::bass_output::BassOutput;
use crate::settings::Language;

pub(crate) const TIMER_ID: usize = 28601;

#[derive(Default)]
pub(crate) struct MixPreview {
    cancel: Option<Arc<AtomicBool>>,
    cache: Arc<Mutex<Option<AudioDescriptionMixPreviewCache>>>,
    status: Arc<Mutex<Option<String>>>,
}

impl MixPreview {
    pub(crate) fn stop(&mut self) {
        if let Some(cancel) = self.cancel.take() {
            cancel.store(true, Ordering::Relaxed);
        }
        if let Ok(mut status) = self.status.lock() {
            *status = None;
        }
    }

    pub(crate) fn take_status(&self) -> Option<String> {
        self.status.lock().ok()?.take()
    }

    pub(crate) fn schedule(
        &mut self,
        job: AudioDescriptionJob,
        language: Language,
        ducking: Option<(f32, u32)>,
    ) {
        self.stop();
        let cancel = Arc::new(AtomicBool::new(false));
        self.cancel = Some(cancel.clone());
        let cache = self.cache.clone();
        let status = self.status.clone();
        if let Ok(mut status) = status.lock() {
            *status = Some(crate::i18n::tr(
                language,
                "audio_description.mix_preview.preparing",
            ));
        }
        std::thread::spawn(move || {
            // Debounce arrow-key adjustments without blocking the window or screen reader.
            for _step in 0..45 {
                if cancel.load(Ordering::Relaxed) {
                    return;
                }
                std::thread::sleep(Duration::from_millis(20));
            }
            let result = (|| -> Result<(), String> {
                // Includes playback: no two renders can overwrite a playing sample.
                let mut cache = cache.lock().map_err(|error| error.to_string())?;
                if cancel.load(Ordering::Relaxed) {
                    return Ok(());
                }
                let audio = render_audio_description_mix_preview(
                    &job,
                    &crate::i18n::tr(language, "audio_description.mix_preview.phrase"),
                    ducking,
                    &mut cache,
                    cancel.clone(),
                )?;
                if cancel.load(Ordering::Relaxed) {
                    return Ok(());
                }
                // Source gain and TTS gain are already baked into the exported sample.
                let player = BassOutput::start(audio.path(), 0, 1.0, 0.0, 1.0, true)?;
                if cancel.load(Ordering::Relaxed) {
                    player.stop();
                    return Ok(());
                }
                if !player.play() {
                    player.stop();
                    return Err("playback failed".to_string());
                }
                if let Ok(mut status) = status.lock()
                    && !cancel.load(Ordering::Relaxed)
                {
                    *status = Some(crate::i18n::tr(
                        language,
                        "audio_description.mix_preview.playing",
                    ));
                }
                while !cancel.load(Ordering::Relaxed)
                    && !player.is_stopped()
                    && player
                        .position_secs()
                        .is_none_or(|position| position + 0.01 < audio.duration_sec())
                {
                    std::thread::sleep(Duration::from_millis(20));
                }
                player.stop();
                Ok(())
            })();
            if let Ok(mut status) = status.lock()
                && !cancel.load(Ordering::Relaxed)
            {
                *status = Some(match result {
                    Ok(()) => crate::i18n::tr(language, "audio_description.ready"),
                    Err(error) => {
                        crate::log_debug(&format!("Audio description mix preview: {error}"));
                        format!(
                            "{}: {}",
                            crate::i18n::tr(language, "audio_description.mix_preview.error"),
                            error
                        )
                    }
                });
            }
        });
    }
}

impl Drop for MixPreview {
    fn drop(&mut self) {
        self.stop();
    }
}

pub(crate) fn selected_volume(combo: HWND) -> f32 {
    let index = unsafe { SendMessageW(combo, CB_GETCURSEL, WPARAM(0), LPARAM(0)).0 };
    if index < 0 {
        1.0
    } else {
        normalize_source_volume(index as f32 * 0.05)
    }
}

pub(crate) fn set_volume(combo: HWND, volume: f32) {
    unsafe {
        SendMessageW(
            combo,
            CB_SETCURSEL,
            WPARAM((normalize_source_volume(volume) * 20.0).round() as usize),
            LPARAM(0),
        );
    }
}

/// The label and controls stay in place while playback changes, preserving focus.
pub(crate) fn create_controls(
    hwnd: HWND,
    language: Language,
    font: HFONT,
    y: i32,
    ids: (usize, usize),
    volume: f32,
) -> (HWND, HWND, HWND) {
    unsafe {
        let label = CreateWindowExW(
            Default::default(),
            WC_STATIC,
            PCWSTR(
                to_wide(&crate::i18n::tr(
                    language,
                    "audio_description.source_volume",
                ))
                .as_ptr(),
            ),
            WS_CHILD | WS_VISIBLE,
            16,
            y,
            230,
            22,
            hwnd,
            HMENU(0),
            HINSTANCE(0),
            None,
        );
        let combo = CreateWindowExW(
            Default::default(),
            WC_COMBOBOXW,
            PCWSTR::null(),
            WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(CBS_DROPDOWNLIST as u32),
            250,
            y - 3,
            120,
            240,
            hwnd,
            HMENU(ids.0 as isize),
            HINSTANCE(0),
            None,
        );
        for percent in (0..=200).step_by(5) {
            SendMessageW(
                combo,
                CB_ADDSTRING,
                WPARAM(0),
                LPARAM(to_wide(&format!("{percent}%")).as_ptr() as isize),
            );
        }
        set_volume(combo, volume);
        let stop = CreateWindowExW(
            Default::default(),
            WC_BUTTON,
            PCWSTR(
                to_wide(&crate::i18n::tr(
                    language,
                    "audio_description.mix_preview.stop",
                ))
                .as_ptr(),
            ),
            WS_CHILD | WS_VISIBLE | WS_TABSTOP,
            388,
            y - 4,
            230,
            28,
            hwnd,
            HMENU(ids.1 as isize),
            HINSTANCE(0),
            None,
        );
        for control in [label, combo, stop] {
            SendMessageW(control, WM_SETFONT, WPARAM(font.0 as usize), LPARAM(1));
        }
        SetTimer(hwnd, TIMER_ID, 100, None);
        (label, combo, stop)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mix_preview_replacement_and_close_cancel_pending_work() {
        let settings = crate::settings::AppSettings::default();
        let job = crate::audio_description::audio_description_preview_job(
            std::path::PathBuf::from("unused.mp4"),
            Language::Italian,
            &settings,
        );
        let last_cancel;
        {
            let mut preview = MixPreview::default();
            preview.schedule(job.clone(), Language::Italian, None);
            let first_cancel = preview.cancel.as_ref().expect("first request").clone();
            preview.schedule(job, Language::Italian, None);
            assert!(first_cancel.load(Ordering::Relaxed));
            last_cancel = preview.cancel.as_ref().expect("replacement").clone();
            assert!(!last_cancel.load(Ordering::Relaxed));
            assert!(preview.take_status().is_some());
        }
        assert!(last_cancel.load(Ordering::Relaxed));
    }
}
