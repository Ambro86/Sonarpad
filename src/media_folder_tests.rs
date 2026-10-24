use super::*;
use windows::Win32::UI::WindowsAndMessaging::{DestroyWindow, PM_REMOVE, PeekMessageW, WM_QUIT};

#[test]
fn folder_media_order_and_settings() -> Result<(), Box<dyn std::error::Error>> {
    let dir = std::env::temp_dir().join(format!("sonarpad-folder-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir(&dir)?;
    for name in [
        "Track 10.mp3",
        "Track 2.MP4",
        "Track 1.mp3",
        "notes.txt",
        "cover.jpg",
    ] {
        std::fs::write(dir.join(name), [])?;
    }
    std::fs::create_dir(dir.join("folder.mp3"))?;
    let paths = media_files_in_folder(&dir.join("Track 2.MP4"))?;
    assert_eq!(
        paths,
        ["Track 1.mp3", "Track 2.MP4", "Track 10.mp3"].map(|n| dir.join(n))
    );
    let settings = AppSettings::default();
    assert!(!settings.auto_advance_folder_media);
    let mut json = serde_json::to_value(settings)?;
    json.as_object_mut()
        .ok_or("settings must serialize as an object")?
        .remove("auto_advance_folder_media");
    assert!(!serde_json::from_value::<AppSettings>(json.clone())?.auto_advance_folder_media);
    json["auto_advance_folder_media"] = serde_json::json!(true);
    assert!(serde_json::from_value::<AppSettings>(json)?.auto_advance_folder_media);
    std::fs::remove_dir_all(dir)?;
    Ok(())
}

// Run explicitly with APPDATA and SONARPAD_FOLDER_TEST_ROOT pointing to a disposable
// fixture containing media/01.mp3, media/02.mp4, media/03.mp3 and Sonarpad/mpv.
#[test]
#[ignore = "requires real BASS/mpv, media fixtures and isolated APPDATA"]
fn folder_media_real_bass_mpv_navigation() -> Result<(), Box<dyn std::error::Error>> {
    let root = PathBuf::from(std::env::var("SONARPAD_FOLDER_TEST_ROOT")?);
    assert_eq!(PathBuf::from(std::env::var("APPDATA")?), root);
    assert_eq!(settings::settings_dir(), root.join("Sonarpad"));
    let media = root.join("media");
    let audio = media.join("01.mp3");
    let video = media.join("02.mp4");
    let last = media.join("03.mp3");
    assert!(audio.is_file() && video.is_file() && last.is_file());
    installed_mpv_runtime_executable()?;
    embedded_deps::extract_all()?;
    let settings = AppSettings {
        audiobook_playback_volume: 0.0,
        show_video_during_playback: false,
        ..Default::default()
    };
    std::fs::write(
        settings::settings_dir().join("settings.json"),
        serde_json::to_vec(&settings)?,
    )?;
    let hwnd = unsafe {
        LoadLibraryW(w!("Msftedit.dll"))?;
        let class = w!("SonarpadFolderPlaybackTest");
        let instance = HINSTANCE(GetModuleHandleW(None)?.0);
        RegisterClassW(&WNDCLASSW {
            hInstance: instance,
            lpszClassName: class,
            lpfnWndProc: Some(wndproc),
            ..Default::default()
        });
        CreateWindowExW(
            Default::default(),
            class,
            w!("Media folder test"),
            WS_OVERLAPPEDWINDOW,
            0,
            0,
            800,
            600,
            HWND(0),
            HMENU(0),
            instance,
            None,
        )
    };
    assert_ne!(hwnd.0, 0);
    struct Cleanup(HWND);
    impl Drop for Cleanup {
        fn drop(&mut self) {
            audio_player::stop_audiobook_playback(self.0);
            stop_managed_mpv_playback(self.0);
            unsafe {
                crate::log_if_err!(DestroyWindow(self.0));
            }
        }
    }
    let _cleanup = Cleanup(hwnd);
    // Exercise the real menu commands and normal player handoff functions.
    queue_audio_files_and_play(hwnd, vec![audio.clone()]);
    wait_for(|| bass_running(hwnd, &audio), "initial MP3/BASS")?;
    let original_tabs = with_state(hwnd, |s| s.docs.len());
    let original_editor = with_state(hwnd, |s| s.docs[0].hwnd_edit);
    unsafe {
        SendMessageW(hwnd, WM_COMMAND, WPARAM(IDM_PLAYBACK_TRACK_NEXT), LPARAM(0));
    }
    wait_for(|| mpv_running(hwnd, &video), "MP3 to MP4/mpv")?;
    assert_eq!(with_state(hwnd, |s| s.docs.len()), original_tabs);
    // With autoplay off, End must retain the file and allow seeking/restarting.
    handle_player_command(hwnd, PlayerCommand::SeekToEnd);
    wait_for(
        || {
            query_managed_mpv_property(hwnd, "eof-reached")
                .ok()
                .and_then(|v| v.as_bool())
                == Some(true)
        },
        "local video EOF",
    )?;
    let duration = query_managed_mpv_property(hwnd, "duration")?
        .as_f64()
        .ok_or("duration lost at EOF")?;
    assert!(
        query_managed_mpv_property(hwnd, "time-pos")?
            .as_f64()
            .is_some()
    );
    handle_player_command(hwnd, PlayerCommand::Seek(-5));
    wait_for(
        || {
            query_managed_mpv_property(hwnd, "time-pos")
                .ok()
                .and_then(|v| v.as_f64())
                .is_some_and(|p| p > 0.0 && p < duration - 1.0)
        },
        "seek backward from EOF",
    )?;
    handle_player_command(hwnd, PlayerCommand::SeekToEnd);
    wait_for(
        || {
            query_managed_mpv_property(hwnd, "eof-reached")
                .ok()
                .and_then(|v| v.as_bool())
                == Some(true)
        },
        "local video EOF again",
    )?;
    handle_player_command(hwnd, PlayerCommand::TogglePause);
    wait_for(
        || {
            query_managed_mpv_property(hwnd, "time-pos")
                .ok()
                .and_then(|v| v.as_f64())
                .is_some_and(|p| p > 0.05 && p < duration - 1.0)
        },
        "Space restarts video at EOF",
    )?;
    assert_eq!(
        with_state(hwnd, |s| s.active_audiobook.is_none()),
        Some(true)
    );
    unsafe {
        SendMessageW(hwnd, WM_COMMAND, WPARAM(IDM_PLAYBACK_TRACK_PREV), LPARAM(0));
    }
    wait_for(|| bass_running(hwnd, &audio), "MP4 to MP3/BASS")?;
    assert_eq!(with_state(hwnd, |s| s.docs.len()), original_tabs);
    assert!(!is_mpv_playback_active(hwnd));
    assert!(
        !switch_audio_playlist_relative(hwnd, -1),
        "first file must not wrap"
    );

    // Disabled by default: finishing the MP3 must not open the next MP4.
    let duration = audio_player::audiobook_duration_secs(&audio).ok_or("no audio duration")?;
    audio_player::seek_audiobook_to(hwnd, duration.saturating_sub(1))?;
    pump_for(std::time::Duration::from_secs(3));
    assert_eq!(current_folder_media(hwnd).as_ref(), Some(&audio));
    check_bass_end_controls(hwnd, &audio)?;
    audio_player::seek_audiobook_to(hwnd, duration.saturating_sub(1))?;
    pump_for(std::time::Duration::from_secs(3));
    with_state(hwnd, |s| s.settings.auto_advance_folder_media = true);
    sync_local_media_autoplay(hwnd);
    wait_for(|| mpv_running(hwnd, &video), "automatic MP3 to MP4")?;
    let duration = query_managed_mpv_property(hwnd, "duration")?
        .as_f64()
        .ok_or("no duration")?;
    try_send_command_to_managed_mpv(
        hwnd,
        &format!(
            r#"{{"command":["seek",{},"absolute+exact"]}}"#,
            (duration - 0.3).max(0.0)
        ),
    )?;
    wait_for(|| bass_running(hwnd, &last), "automatic MP4 to MP3")?;
    assert_eq!(with_state(hwnd, |s| s.docs.len()), original_tabs);
    assert_eq!(with_state(hwnd, |s| s.docs[0].hwnd_edit), original_editor);
    assert!(
        !switch_audio_playlist_relative(hwnd, 1),
        "last file must not wrap"
    );
    // With autoplay enabled the final file must still support EOF controls.
    let last_duration = audio_player::audiobook_duration_secs(&last).ok_or("last duration")?;
    audio_player::seek_audiobook_to(hwnd, last_duration.saturating_sub(1))?;
    pump_for(std::time::Duration::from_secs(3));
    check_bass_end_controls(hwnd, &last)?;
    unsafe {
        SendMessageW(hwnd, WM_COMMAND, WPARAM(IDM_PLAYBACK_TRACK_PREV), LPARAM(0));
    }
    wait_for(
        || mpv_running(hwnd, &video),
        "previous from final MP3 to MP4",
    )?;
    assert_eq!(with_state(hwnd, |s| s.docs.len()), original_tabs);
    // Pause/resume and both seek directions also work before EOF with autoplay on.
    handle_player_command(hwnd, PlayerCommand::TogglePause);
    assert_eq!(
        query_managed_mpv_property(hwnd, "pause")?.as_bool(),
        Some(true)
    );
    handle_player_command(hwnd, PlayerCommand::Seek(5));
    handle_player_command(hwnd, PlayerCommand::Seek(-2));
    handle_player_command(hwnd, PlayerCommand::TogglePause);
    wait_for(
        || mpv_running(hwnd, &video),
        "video resumes with autoplay enabled",
    )?;
    println!("Verified real BASS <-> mpv menu navigation, autoplay off/on and folder boundaries.");
    Ok(())
}

fn check_bass_end_controls(hwnd: HWND, path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    assert_eq!(audio_player::audiobook_output_stopped(hwnd), Some(true));
    let duration = audio_player::audiobook_duration_secs(path).ok_or("audio duration")?;
    handle_player_command(hwnd, PlayerCommand::Seek(-5));
    wait_for(|| bass_running(hwnd, path), "BASS seek back from EOF")?;
    let position = with_state(hwnd, |s| {
        s.active_audiobook
            .as_ref()
            .map(audio_player::audiobook_position_secs)
    })
    .flatten()
    .ok_or("missing BASS player")?;
    assert!(position < duration as f64 - 1.0);
    handle_player_command(hwnd, PlayerCommand::Seek(2));
    handle_player_command(hwnd, PlayerCommand::TogglePause);
    assert_eq!(
        with_state(hwnd, |s| s.active_audiobook.as_ref().map(|p| p.is_paused)).flatten(),
        Some(true)
    );
    handle_player_command(hwnd, PlayerCommand::TogglePause);
    wait_for(|| bass_running(hwnd, path), "BASS resume")?;
    audio_player::seek_audiobook_to(hwnd, duration.saturating_sub(1))?;
    pump_for(std::time::Duration::from_secs(3));
    handle_player_command(hwnd, PlayerCommand::TogglePause);
    wait_for(|| bass_running(hwnd, path), "Space restarts BASS at EOF")?;
    let position = with_state(hwnd, |s| {
        s.active_audiobook
            .as_ref()
            .map(audio_player::audiobook_position_secs)
    })
    .flatten()
    .ok_or("missing restarted BASS player")?;
    assert!(
        position < 5.0,
        "BASS must restart from the beginning: {position}"
    );
    Ok(())
}

fn bass_running(hwnd: HWND, path: &Path) -> bool {
    with_state(hwnd, |s| {
        s.active_audiobook
            .as_ref()
            .is_some_and(|p| p.path == path && audio_player::audiobook_position_secs(p) > 0.05)
    })
    .unwrap_or(false)
}

fn mpv_running(hwnd: HWND, path: &Path) -> bool {
    current_folder_media(hwnd).as_deref() == Some(path)
        && is_mpv_playback_active(hwnd)
        && query_managed_mpv_property(hwnd, "time-pos")
            .ok()
            .and_then(|v| v.as_f64())
            .is_some_and(|p| p > 0.05)
}

fn pump_for(duration: std::time::Duration) {
    let until = std::time::Instant::now() + duration;
    while std::time::Instant::now() < until {
        unsafe {
            let mut msg = MSG::default();
            while PeekMessageW(&mut msg, HWND(0), 0, 0, PM_REMOVE).as_bool() {
                if msg.message != WM_QUIT {
                    TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                }
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
}

fn wait_for(mut ready: impl FnMut() -> bool, context: &str) -> Result<(), String> {
    let until = std::time::Instant::now() + std::time::Duration::from_secs(20);
    while std::time::Instant::now() < until {
        pump_for(std::time::Duration::from_millis(50));
        if ready() {
            return Ok(());
        }
    }
    Err(format!("Timeout: {context}"))
}
