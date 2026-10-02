use std::fs;
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use chrono::{Local, NaiveDateTime};
use serde::{Deserialize, Serialize};
use windows::Win32::Foundation::{CloseHandle, HWND};
use windows::Win32::System::Threading::{
    GetExitCodeProcess, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION,
};
use windows::Win32::UI::WindowsAndMessaging::{
    IDYES, MB_ICONQUESTION, MB_ICONWARNING, MB_OK, MB_YESNO,
};
use windows::core::PCWSTR;

use crate::accessibility::to_wide;
use crate::app_windows::interpreter_select_window::InterpreterContextAction;
use crate::app_windows::youtube_transcript_window::{
    self, MultilineRefreshOptions, MultilineSearchOptions, MultilineSelectionItem,
    MultilineSelectionResult,
};
use crate::settings::Language;

const CREATE_NO_WINDOW_FLAG: u32 = 0x0800_0000;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum StreamRecordingKind {
    Radio,
    Tv,
}

pub(crate) struct ScheduledRecordingOptions<'a> {
    pub(crate) duration_minutes: u32,
    pub(crate) scheduled_id: &'a str,
    pub(crate) prefer_audio_description: bool,
}

impl StreamRecordingKind {
    fn folder_name(self) -> &'static str {
        match self {
            Self::Radio => "Registrazioni Radio",
            Self::Tv => "Registrazioni TV",
        }
    }

    fn extension(self) -> &'static str {
        match self {
            Self::Radio => "mp3",
            Self::Tv => "mp4",
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct RecordingActivity {
    id: String,
    kind: StreamRecordingKind,
    title: String,
    output_path: String,
    process_id: u32,
    started_unix: u64,
    expected_end_unix: Option<u64>,
    scheduled_id: Option<String>,
}

#[derive(Clone)]
enum RecordingListEntry {
    Scheduled {
        id: String,
        title: String,
        description: String,
    },
    Active {
        id: String,
        title: String,
        description: String,
    },
    File(PathBuf),
}

impl RecordingListEntry {
    fn id(&self) -> String {
        match self {
            Self::Scheduled { id, .. } | Self::Active { id, .. } => id.clone(),
            Self::File(path) => path.to_string_lossy().to_string(),
        }
    }

    fn as_item(&self) -> MultilineSelectionItem {
        match self {
            Self::Scheduled {
                id,
                title,
                description,
            }
            | Self::Active {
                id,
                title,
                description,
            } => MultilineSelectionItem {
                id: id.clone(),
                title: title.clone(),
                description: Some(description.clone()),
            },
            Self::File(path) => MultilineSelectionItem {
                id: path.to_string_lossy().to_string(),
                title: path
                    .file_stem()
                    .and_then(|value| value.to_str())
                    .unwrap_or("Registrazione")
                    .to_string(),
                description: fs::metadata(path)
                    .ok()
                    .map(|metadata| format_file_size(metadata.len())),
            },
        }
    }
}

pub(crate) fn start_radio_recording_and_playback(
    parent: HWND,
    url: &str,
    title: &str,
    language: Language,
) -> Result<PathBuf, String> {
    start_recording_and_playback(
        parent,
        url,
        title,
        None,
        false,
        StreamRecordingKind::Radio,
        Some(language),
    )
}

pub(crate) fn start_tv_recording_and_playback(
    parent: HWND,
    url: &str,
    title: &str,
    user_agent: &str,
    prefer_audio_description: bool,
) -> Result<PathBuf, String> {
    start_recording_and_playback(
        parent,
        url,
        title,
        Some(user_agent),
        prefer_audio_description,
        StreamRecordingKind::Tv,
        None,
    )
}

fn start_recording_and_playback(
    parent: HWND,
    url: &str,
    title: &str,
    user_agent: Option<&str>,
    prefer_audio_description: bool,
    kind: StreamRecordingKind,
    ui_language: Option<Language>,
) -> Result<PathBuf, String> {
    let url = url.trim();
    if url.is_empty() {
        return Err(recording_text(
            ui_language,
            "radio.recording_error_empty_url",
            "L'indirizzo dello stream è vuoto.",
        ));
    }

    let output_path = next_recording_path(kind, title, ui_language)?;
    let playback_result = match kind {
        StreamRecordingKind::Radio => {
            crate::launch_stream_url_in_mpv(parent, url, Some(title), None, None, None)
        }
        StreamRecordingKind::Tv => crate::launch_tv_stream_for_recording_in_mpv(
            parent,
            url,
            title,
            user_agent.unwrap_or_default(),
            prefer_audio_description,
        ),
    };

    if let Err(err) = playback_result {
        remove_recording_file(&output_path, "player launch failure");
        return Err(err);
    }

    let Some(mpv_process_id) = crate::active_mpv_process_id(parent) else {
        remove_recording_file(&output_path, "missing player process");
        return Err(recording_text(
            ui_language,
            "radio.recording_error_link_player",
            "Il player è stato avviato, ma non è stato possibile collegare la registrazione al suo processo.",
        ));
    };

    if kind == StreamRecordingKind::Tv
        && !crate::wait_for_active_mpv_tracks(parent, Duration::from_secs(12))
    {
        crate::stop_managed_mpv_playback(parent);
        remove_recording_file(&output_path, "TV loading timeout");
        return Err(recording_text(
            ui_language,
            "radio.recording_error_link_player",
            "Il canale TV non ha completato il caricamento e la registrazione non è stata avviata.",
        ));
    }
    if kind == StreamRecordingKind::Tv && prefer_audio_description {
        // stream-record non deve partire mentre mpv sta ancora cambiando la
        // rendition audio/video HLS: alcuni stream si fermano al primo segmento.
        if let Err(error) = crate::stabilize_active_mpv_audiodescription_tracks_for_recording(
            parent,
            Duration::from_secs(6),
        ) {
            crate::stop_managed_mpv_playback(parent);
            remove_recording_file(&output_path, "TV track stabilization failure");
            return Err(error);
        }
    }

    let activity = match create_activity(kind, title, &output_path, None, None) {
        Ok(activity) => activity,
        Err(error) => {
            crate::stop_managed_mpv_playback(parent);
            remove_recording_file(&output_path, "recording activity creation failure");
            return Err(error);
        }
    };

    if kind == StreamRecordingKind::Tv {
        let open_audio_description_after = crate::with_state(parent, |state| {
            state.settings.language == Language::Italian
                && state.settings.audio_description_after_tv_recording
        })
        .unwrap_or(false);
        let temp_path = match mpv_stream_recording_temp_path(&output_path) {
            Ok(path) => path,
            Err(error) => {
                remove_activity(&activity.id);
                crate::stop_managed_mpv_playback(parent);
                return Err(error);
            }
        };
        if let Err(error) = crate::start_active_mpv_stream_recording(parent, &temp_path) {
            remove_activity(&activity.id);
            crate::stop_managed_mpv_playback(parent);
            remove_recording_file(&temp_path, "mpv stream recording startup failure");
            remove_recording_file(&output_path, "mpv stream recording startup failure");
            return Err(error);
        }

        crate::log_debug(&format!(
            "Managed mpv TV recording started: mpv_pid={mpv_process_id} temp={} output={}",
            temp_path.display(),
            output_path.display()
        ));
        let activity_id = activity.id.clone();
        let output_for_thread = output_path.clone();
        thread::spawn(move || {
            monitor_mpv_recording_until_player_closes(
                parent,
                mpv_process_id,
                temp_path,
                output_for_thread,
                activity_id,
                open_audio_description_after,
            );
        });
        return Ok(output_path);
    }

    let stop = Arc::new(AtomicBool::new(false));
    let result_rx = spawn_internal_recording(
        url.to_string(),
        output_path.clone(),
        user_agent.map(ToOwned::to_owned),
        kind,
        prefer_audio_description,
        Arc::clone(&stop),
    );

    thread::sleep(Duration::from_millis(350));
    match result_rx.try_recv() {
        Ok(Ok(())) => {
            remove_activity(&activity.id);
            crate::stop_managed_mpv_playback(parent);
            if !recording_output_is_usable(&output_path) {
                remove_recording_file(&output_path, "internal FFmpeg immediate completion");
                return Err(
                    "La registrazione è terminata immediatamente senza produrre un file valido."
                        .to_string(),
                );
            }
            return Ok(output_path);
        }
        Ok(Err(error)) => {
            remove_activity(&activity.id);
            crate::stop_managed_mpv_playback(parent);
            remove_recording_file(&output_path, "internal FFmpeg startup failure");
            return Err(error);
        }
        Err(TryRecvError::Disconnected) => {
            remove_activity(&activity.id);
            crate::stop_managed_mpv_playback(parent);
            remove_recording_file(&output_path, "internal FFmpeg channel disconnected");
            return Err(
                "Il motore FFmpeg interno si è chiuso durante l'avvio della registrazione."
                    .to_string(),
            );
        }
        Err(TryRecvError::Empty) => {}
    }

    crate::log_debug(&format!(
        "Internal FFmpeg stream recording started: kind={kind:?} mpv_pid={mpv_process_id} output={}",
        output_path.display()
    ));

    let output_for_thread = output_path.clone();
    let activity_id = activity.id.clone();
    thread::spawn(move || {
        monitor_internal_recording_until_player_closes(
            result_rx,
            stop,
            mpv_process_id,
            output_for_thread,
            activity_id,
        );
    });

    Ok(output_path)
}

pub(crate) fn record_stream_for_duration(
    url: &str,
    title: &str,
    user_agent: Option<&str>,
    kind: StreamRecordingKind,
    ui_language: Option<Language>,
    options: ScheduledRecordingOptions<'_>,
) -> Result<PathBuf, String> {
    let _power_awake = crate::power_awake::acquire("scheduled-stream-recording");
    let url = url.trim();
    if url.is_empty() {
        return Err(recording_text(
            ui_language,
            "radio.recording_error_empty_url",
            "L'indirizzo dello stream è vuoto.",
        ));
    }
    if options.duration_minutes == 0 {
        return Err("La durata della registrazione deve essere maggiore di zero.".to_string());
    }

    // A cancellation marker belongs only to the currently running occurrence.
    // Remove any stale marker before publishing the activity in the recordings list;
    // after that point the UI can safely create a fresh marker to stop this run.
    clear_scheduled_cancel_request(options.scheduled_id);

    let output_path = next_recording_path(kind, title, ui_language)?;
    let duration = Duration::from_secs(u64::from(options.duration_minutes) * 60);
    let expected_end = unix_now().saturating_add(duration.as_secs());
    let activity = create_activity(
        kind,
        title,
        &output_path,
        Some(expected_end),
        Some(options.scheduled_id),
    )?;

    let result = match kind {
        StreamRecordingKind::Radio => {
            let stop = Arc::new(AtomicBool::new(false));
            let timer_stop = Arc::clone(&stop);
            let scheduled_id = options.scheduled_id.to_string();
            thread::spawn(move || {
                let started = Instant::now();
                loop {
                    if scheduled_cancel_requested(&scheduled_id) {
                        crate::log_debug(&format!(
                            "Scheduled radio recording observed graceful stop request id={scheduled_id}"
                        ));
                        timer_stop.store(true, Ordering::Release);
                        break;
                    }
                    let elapsed = started.elapsed();
                    if elapsed >= duration {
                        timer_stop.store(true, Ordering::Release);
                        break;
                    }
                    thread::sleep(
                        duration
                            .saturating_sub(elapsed)
                            .min(Duration::from_millis(250)),
                    );
                }
            });
            run_internal_recording(
                url,
                &output_path,
                user_agent,
                kind,
                options.prefer_audio_description,
                stop,
            )
        }
        StreamRecordingKind::Tv => record_tv_for_duration_with_hidden_mpv(
            url,
            &output_path,
            user_agent,
            options.prefer_audio_description,
            duration,
            options.scheduled_id,
        ),
    };

    remove_activity(&activity.id);
    clear_scheduled_cancel_request(options.scheduled_id);
    if let Err(error) = result {
        remove_recording_file(&output_path, "scheduled recording failure");
        return Err(error);
    }
    if !recording_output_is_usable(&output_path) {
        remove_recording_file(&output_path, "scheduled recording too small");
        return Err("Il motore di registrazione non ha prodotto un file valido.".to_string());
    }
    Ok(output_path)
}

fn spawn_internal_recording(
    url: String,
    output_path: PathBuf,
    user_agent: Option<String>,
    kind: StreamRecordingKind,
    prefer_audio_description: bool,
    stop: Arc<AtomicBool>,
) -> Receiver<Result<(), String>> {
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        let result = run_internal_recording(
            &url,
            &output_path,
            user_agent.as_deref(),
            kind,
            prefer_audio_description,
            stop,
        );
        crate::log_if_err!(sender.send(result), "Sending recording result failed");
    });
    receiver
}

fn run_internal_recording(
    url: &str,
    output_path: &Path,
    user_agent: Option<&str>,
    kind: StreamRecordingKind,
    prefer_audio_description: bool,
    stop: Arc<AtomicBool>,
) -> Result<(), String> {
    let _power_awake = crate::power_awake::acquire("stream-recording");
    match kind {
        StreamRecordingKind::Radio => {
            crate::ffmpeg_export::record_live_audio_stream_to_mp3(url, output_path, stop)
        }
        StreamRecordingKind::Tv => crate::ffmpeg_export::record_live_media_stream_to_mp4(
            url,
            output_path,
            user_agent,
            prefer_audio_description,
            stop,
        ),
    }
}

fn mpv_stream_recording_temp_path(output_path: &Path) -> Result<PathBuf, String> {
    let temp_dir = crate::settings::settings_dir().join("RecordingTemp");
    fs::create_dir_all(&temp_dir).map_err(|error| {
        format!("Impossibile creare la cartella temporanea delle registrazioni: {error}")
    })?;
    let stem = output_path
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("tv_recording");
    Ok(temp_dir.join(format!("{stem}.ts")))
}

fn background_mpv_ipc_path() -> PathBuf {
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    PathBuf::from(format!(
        r"\\.\pipe\sonarpad-scheduled-mpv-{}-{suffix}",
        std::process::id()
    ))
}

fn query_mpv_property_at_ipc(ipc_path: &Path, property: &str) -> Result<serde_json::Value, String> {
    let mut pipe = crate::open_mpv_ipc_pipe(ipc_path)?;
    let request = serde_json::json!({
        "command": ["get_property", property]
    })
    .to_string();
    let response = crate::send_mpv_ipc_request_with_id(ipc_path, &mut pipe, &request, 1)?;
    Ok(response
        .get("data")
        .cloned()
        .unwrap_or(serde_json::Value::Null))
}

fn background_mpv_exited(child: &mut Child) -> Result<Option<String>, String> {
    child
        .try_wait()
        .map(|status| status.map(|value| value.to_string()))
        .map_err(|error| format!("Impossibile controllare il processo mpv nascosto: {error}"))
}

fn wait_for_background_mpv_ipc(
    child: &mut Child,
    ipc_path: &Path,
    timeout: Duration,
) -> Result<(), String> {
    let started = Instant::now();
    while started.elapsed() < timeout {
        if let Some(status) = background_mpv_exited(child)? {
            return Err(format!(
                "mpv si è chiuso durante l'apertura del flusso, stato {status}."
            ));
        }
        if crate::send_mpv_ipc_command(ipc_path, r#"{"command":["get_property","pause"]}"#).is_ok()
        {
            return Ok(());
        }
        thread::sleep(Duration::from_millis(100));
    }
    Err("mpv non ha inizializzato il canale di controllo della registrazione.".to_string())
}

fn wait_for_background_mpv_tracks(
    child: &mut Child,
    ipc_path: &Path,
    timeout: Duration,
) -> Result<serde_json::Value, String> {
    let started = Instant::now();
    while started.elapsed() < timeout {
        if let Some(status) = background_mpv_exited(child)? {
            return Err(format!(
                "mpv si è chiuso prima di caricare le tracce, stato {status}."
            ));
        }
        if let Ok(track_list) = query_mpv_property_at_ipc(ipc_path, "track-list")
            && track_list
                .as_array()
                .map(|tracks| !tracks.is_empty())
                .unwrap_or(false)
        {
            crate::log_debug(&format!(
                "Scheduled hidden mpv tracks ready after {} ms: {}",
                started.elapsed().as_millis(),
                track_list
            ));
            return Ok(track_list);
        }
        thread::sleep(Duration::from_millis(200));
    }
    Err("mpv non ha caricato le tracce del canale entro il tempo previsto.".to_string())
}

fn track_id(track: &serde_json::Value) -> Option<i64> {
    track.get("id").and_then(serde_json::Value::as_i64)
}

fn track_is_type(track: &serde_json::Value, kind: &str) -> bool {
    track
        .get("type")
        .and_then(serde_json::Value::as_str)
        .map(|value| value.eq_ignore_ascii_case(kind))
        .unwrap_or(false)
}

fn track_is_selected(track: &serde_json::Value) -> bool {
    track
        .get("selected")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false)
}

fn select_background_mpv_tracks(
    ipc_path: &Path,
    track_list: &serde_json::Value,
    prefer_audio_description: bool,
) -> Result<(Option<i64>, Option<i64>, bool), String> {
    let Some(tracks) = track_list.as_array() else {
        return Err("mpv ha restituito un elenco tracce non valido.".to_string());
    };

    let audiodescription_id = prefer_audio_description
        .then(|| crate::audiodescription_mpv_audio_track_id(track_list))
        .flatten();
    let selected_video = tracks
        .iter()
        .find(|track| track_is_type(track, "video") && track_is_selected(track));
    let fallback_video = selected_video.or_else(|| {
        tracks
            .iter()
            .filter(|track| track_is_type(track, "video"))
            .max_by_key(|track| {
                track
                    .get("hls-bitrate")
                    .and_then(serde_json::Value::as_i64)
                    .unwrap_or_default()
            })
    });
    let preferred_video_id = audiodescription_id
        .and_then(|audio_id| crate::preferred_tv_video_track_id(tracks, Some(audio_id)))
        .or_else(|| fallback_video.and_then(track_id));
    let preferred_audio_id = if let Some(audio_id) = audiodescription_id {
        Some(audio_id)
    } else if prefer_audio_description {
        crate::preferred_mpv_audio_track_id(track_list)
    } else {
        tracks
            .iter()
            .find(|track| track_is_type(track, "audio") && track_is_selected(track))
            .and_then(track_id)
            .or_else(|| {
                let video_program = fallback_video
                    .and_then(|track| track.get("program-id"))
                    .and_then(serde_json::Value::as_i64);
                tracks
                    .iter()
                    .find(|track| {
                        track_is_type(track, "audio")
                            && video_program.is_some_and(|program| {
                                track.get("program-id").and_then(serde_json::Value::as_i64)
                                    == Some(program)
                            })
                    })
                    .and_then(track_id)
            })
            .or_else(|| {
                tracks
                    .iter()
                    .find(|track| track_is_type(track, "audio"))
                    .and_then(track_id)
            })
    };

    if let Some(audio_id) = preferred_audio_id {
        let command = serde_json::json!({
            "command": ["set_property", "aid", audio_id]
        })
        .to_string();
        crate::send_mpv_ipc_command(ipc_path, &command)?;
    }
    if let Some(video_id) = preferred_video_id {
        let command = serde_json::json!({
            "command": ["set_property", "vid", video_id]
        })
        .to_string();
        crate::send_mpv_ipc_command(ipc_path, &command)?;
    }

    Ok((
        preferred_audio_id,
        preferred_video_id,
        audiodescription_id.is_some(),
    ))
}

fn wait_for_background_mpv_track_stability(
    child: &mut Child,
    ipc_path: &Path,
    audio_id: Option<i64>,
    video_id: Option<i64>,
    timeout: Duration,
) -> Result<(), String> {
    let started = Instant::now();
    let mut stable_since = None;
    while started.elapsed() < timeout {
        if let Some(status) = child
            .try_wait()
            .map_err(|error| format!("Impossibile verificare mpv: {error}"))?
        {
            return Err(format!(
                "mpv si è chiuso durante la selezione delle tracce TV ({status})."
            ));
        }

        let track_list = query_mpv_property_at_ipc(ipc_path, "track-list")?;
        let selected = track_list.as_array().is_some_and(|tracks| {
            let audio_ready = audio_id.is_none_or(|audio_id| {
                tracks.iter().any(|track| {
                    track_is_type(track, "audio")
                        && track_id(track) == Some(audio_id)
                        && track_is_selected(track)
                })
            });
            let video_ready = video_id.is_none_or(|video_id| {
                tracks.iter().any(|track| {
                    track_is_type(track, "video")
                        && track_id(track) == Some(video_id)
                        && track_is_selected(track)
                })
            });
            audio_ready && video_ready
        });

        if selected {
            let since = stable_since.get_or_insert_with(Instant::now);
            if since.elapsed() >= Duration::from_millis(750) {
                crate::log_debug(&format!(
                    "Scheduled TV recording tracks stabilized: audio_id={} video_id={}",
                    audio_id
                        .map(|value| value.to_string())
                        .unwrap_or_else(|| "none".to_string()),
                    video_id
                        .map(|value| value.to_string())
                        .unwrap_or_else(|| "none".to_string())
                ));
                return Ok(());
            }
        } else {
            stable_since = None;
        }
        thread::sleep(Duration::from_millis(100));
    }

    Err("Le tracce TV della registrazione programmata non si sono stabilizzate.".to_string())
}

fn remux_mpv_stream_recording(temp_path: &Path, output_path: &Path) -> Result<(), String> {
    if !recording_output_is_usable(temp_path) {
        remove_recording_file(temp_path, "empty mpv stream recording");
        return Err("mpv non ha prodotto un flusso registrato utilizzabile.".to_string());
    }
    let result = crate::ffmpeg_export::remux_media_file_to_mp4_with_preferred_audio_stream(
        temp_path,
        output_path,
        None,
        None,
        None,
    );
    remove_recording_file(temp_path, "mpv stream recording cleanup");
    result?;
    if !recording_output_is_usable(output_path) {
        remove_recording_file(output_path, "unusable remuxed mpv recording");
        return Err(
            "La conversione finale della registrazione mpv non ha prodotto un MP4 valido."
                .to_string(),
        );
    }
    Ok(())
}

fn record_tv_for_duration_with_hidden_mpv(
    url: &str,
    output_path: &Path,
    user_agent: Option<&str>,
    prefer_audio_description: bool,
    duration: Duration,
    scheduled_id: &str,
) -> Result<(), String> {
    let mpv_exe = crate::installed_mpv_runtime_executable()?;
    let mpv_dir = mpv_exe
        .parent()
        .map(Path::to_path_buf)
        .ok_or_else(|| "Cartella di mpv non valida.".to_string())?;
    let temp_path = mpv_stream_recording_temp_path(output_path)?;
    remove_recording_file(&temp_path, "hidden mpv recording preparation");
    remove_recording_file(output_path, "hidden mpv output preparation");
    let ipc_path = background_mpv_ipc_path();

    let mut command = Command::new(&mpv_exe);
    command
        .current_dir(&mpv_dir)
        .arg(url)
        .arg("--no-config")
        .arg("--terminal=no")
        .arg("--input-default-bindings=no")
        .arg("--osc=no")
        .arg("--force-window=no")
        .arg("--vo=null")
        .arg("--ao=null")
        .arg("--pause=yes")
        .arg("--aid=auto")
        .arg("--audio-channels=stereo")
        .arg(format!("--input-ipc-server={}", ipc_path.display()))
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .creation_flags(CREATE_NO_WINDOW_FLAG);
    if url.to_ascii_lowercase().contains(".m3u8") {
        command.arg("--hls-bitrate=max");
    }
    if let Some(user_agent) = user_agent.map(str::trim).filter(|value| !value.is_empty()) {
        command.arg(format!("--user-agent={user_agent}"));
    }

    let mut child = command
        .spawn()
        .map_err(|error| format!("Impossibile avviare mpv in modalità registrazione: {error}"))?;
    crate::log_debug(&format!(
        "Scheduled hidden mpv TV recording process started: pid={} temp={} output={} duration_secs={}",
        child.id(),
        temp_path.display(),
        output_path.display(),
        duration.as_secs()
    ));

    let setup_result = (|| -> Result<(), String> {
        wait_for_background_mpv_ipc(&mut child, &ipc_path, Duration::from_secs(12))?;
        let track_list =
            wait_for_background_mpv_tracks(&mut child, &ipc_path, Duration::from_secs(20))?;
        let (audio_id, video_id, selected_audiodescription) =
            select_background_mpv_tracks(&ipc_path, &track_list, prefer_audio_description)?;
        if selected_audiodescription {
            wait_for_background_mpv_track_stability(
                &mut child,
                &ipc_path,
                audio_id,
                video_id,
                Duration::from_secs(6),
            )?;
        } else {
            thread::sleep(Duration::from_millis(200));
        }
        let record_command = serde_json::json!({
            "command": ["set_property", "stream-record", temp_path.to_string_lossy()]
        })
        .to_string();
        crate::send_mpv_ipc_command(&ipc_path, &record_command)?;
        let active_record_path = query_mpv_property_at_ipc(&ipc_path, "stream-record")?;
        if active_record_path
            .as_str()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .is_none()
        {
            return Err("mpv non ha attivato il file della registrazione programmata.".to_string());
        }
        crate::send_mpv_ipc_command(&ipc_path, r#"{"command":["set_property","pause",false]}"#)?;
        Ok(())
    })();
    if let Err(error) = setup_result {
        if let Err(kill_error) = child.kill() {
            crate::log_debug(&format!(
                "Scheduled hidden mpv kill after setup failure failed: {kill_error}"
            ));
        }
        if let Err(wait_error) = child.wait() {
            crate::log_debug(&format!(
                "Scheduled hidden mpv wait after setup failure failed: {wait_error}"
            ));
        }
        remove_recording_file(&temp_path, "hidden mpv setup failure");
        return Err(error);
    }

    let recording_started = Instant::now();
    let mut cancelled_by_user = false;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => {}
            Err(error) => {
                if let Err(kill_error) = child.kill() {
                    crate::log_debug(&format!(
                        "Scheduled hidden mpv kill after status failure failed: {kill_error}"
                    ));
                }
                if let Err(wait_error) = child.wait() {
                    crate::log_debug(&format!(
                        "Scheduled hidden mpv wait after status failure failed: {wait_error}"
                    ));
                }
                remove_recording_file(&temp_path, "hidden mpv status failure");
                return Err(format!(
                    "Impossibile controllare mpv durante la registrazione: {error}"
                ));
            }
        }

        let elapsed = recording_started.elapsed();
        if scheduled_cancel_requested(scheduled_id) {
            cancelled_by_user = true;
            crate::log_debug(&format!(
                "Scheduled TV recording observed graceful stop request id={scheduled_id} elapsed_secs={:.3}",
                elapsed.as_secs_f64()
            ));
        }
        if cancelled_by_user || elapsed >= duration {
            let quit_result = crate::send_mpv_ipc_command(&ipc_path, r#"{"command":["quit"]}"#);
            if let Err(error) = quit_result {
                crate::log_debug(&format!(
                    "Scheduled hidden mpv graceful quit failed; terminating process: {error}"
                ));
                if let Err(kill_error) = child.kill() {
                    crate::log_debug(&format!(
                        "Scheduled hidden mpv forced termination failed: {kill_error}"
                    ));
                }
            }
            break match child.wait() {
                Ok(status) => status,
                Err(error) => {
                    remove_recording_file(&temp_path, "hidden mpv final wait failure");
                    return Err(format!("Impossibile attendere la fine di mpv: {error}"));
                }
            };
        }

        let remaining = duration.saturating_sub(elapsed);
        thread::sleep(remaining.min(Duration::from_millis(250)));
    };
    thread::sleep(Duration::from_millis(200));
    let elapsed = recording_started.elapsed();
    crate::log_debug(&format!(
        "Scheduled hidden mpv TV recording process ended: status={} elapsed_secs={:.3} temp_bytes={}",
        status,
        elapsed.as_secs_f64(),
        fs::metadata(&temp_path)
            .map(|metadata| metadata.len())
            .unwrap_or_default()
    ));
    if !status.success() {
        remove_recording_file(&temp_path, "hidden mpv unsuccessful exit");
        return Err(format!(
            "mpv ha terminato la registrazione con stato {status}."
        ));
    }
    if !cancelled_by_user && elapsed.saturating_add(Duration::from_secs(1)) < duration {
        remove_recording_file(&temp_path, "hidden mpv early exit");
        return Err(format!(
            "mpv ha terminato la registrazione troppo presto: {:.1} secondi su {}.",
            elapsed.as_secs_f64(),
            duration.as_secs()
        ));
    }
    remux_mpv_stream_recording(&temp_path, output_path)
}

fn monitor_mpv_recording_until_player_closes(
    parent: HWND,
    mpv_process_id: u32,
    temp_path: PathBuf,
    output_path: PathBuf,
    activity_id: String,
    open_audio_description_after: bool,
) {
    let _power_awake = crate::power_awake::acquire("stream-recording");
    while is_process_alive(mpv_process_id) {
        thread::sleep(Duration::from_millis(500));
    }
    // Il processo è terminato: il file stream-record non è più aperto da mpv.
    thread::sleep(Duration::from_millis(150));
    finalize_mpv_recording(
        parent,
        &temp_path,
        &output_path,
        &activity_id,
        open_audio_description_after,
    );
}

fn finalize_mpv_recording(
    parent: HWND,
    temp_path: &Path,
    output_path: &Path,
    activity_id: &str,
    open_audio_description_after: bool,
) {
    remove_activity(activity_id);
    let temp_bytes = fs::metadata(temp_path)
        .map(|metadata| metadata.len())
        .unwrap_or(0);
    let result = remux_mpv_stream_recording(temp_path, output_path);
    let usable = result.is_ok() && recording_output_is_usable(output_path);
    crate::log_debug(&format!(
        "Managed mpv TV recording finalized: success={} temp_bytes={} output_bytes={} temp={} output={} error={}",
        usable,
        temp_bytes,
        fs::metadata(output_path)
            .map(|metadata| metadata.len())
            .unwrap_or(0),
        temp_path.display(),
        output_path.display(),
        result.as_ref().err().map(String::as_str).unwrap_or("")
    ));
    if !usable {
        remove_recording_file(output_path, "unusable mpv stream recording");
    } else if open_audio_description_after {
        let posted = crate::post_audio_description_for_completed_tv_recording(parent, output_path);
        crate::log_debug(&format!(
            "Completed TV recording audio-description handoff: posted={} output={}",
            posted,
            output_path.display()
        ));
    }
}

fn monitor_internal_recording_until_player_closes(
    result_rx: Receiver<Result<(), String>>,
    stop: Arc<AtomicBool>,
    mpv_process_id: u32,
    output_path: PathBuf,
    activity_id: String,
) {
    loop {
        match result_rx.try_recv() {
            Ok(result) => {
                finalize_internal_recording(result, &output_path, &activity_id);
                return;
            }
            Err(TryRecvError::Disconnected) => {
                finalize_internal_recording(
                    Err(
                        "Il worker FFmpeg interno si è chiuso senza restituire un risultato."
                            .to_string(),
                    ),
                    &output_path,
                    &activity_id,
                );
                return;
            }
            Err(TryRecvError::Empty) => {}
        }

        if !is_process_alive(mpv_process_id) {
            stop.store(true, Ordering::Release);
            let result = result_rx.recv().unwrap_or_else(|_| {
                Err(
                    "Il worker FFmpeg interno non ha completato correttamente la chiusura."
                        .to_string(),
                )
            });
            finalize_internal_recording(result, &output_path, &activity_id);
            return;
        }
        thread::sleep(Duration::from_millis(500));
    }
}

fn finalize_internal_recording(result: Result<(), String>, output_path: &Path, activity_id: &str) {
    remove_activity(activity_id);
    let usable = result.is_ok() && recording_output_is_usable(output_path);
    crate::log_debug(&format!(
        "Internal FFmpeg stream recording finalized: success={} bytes={} output={} error={}",
        usable,
        fs::metadata(output_path)
            .map(|metadata| metadata.len())
            .unwrap_or(0),
        output_path.display(),
        result.as_ref().err().map(String::as_str).unwrap_or("")
    ));
    if !usable {
        remove_recording_file(output_path, "unusable internal recording");
    }
}

fn recording_output_is_usable(path: &Path) -> bool {
    fs::metadata(path)
        .map(|metadata| metadata.len() >= 1024)
        .unwrap_or(false)
}

fn remove_recording_file(path: &Path, context: &str) {
    let Err(err) = fs::remove_file(path) else {
        return;
    };
    if err.kind() != std::io::ErrorKind::NotFound {
        crate::log_debug(&format!(
            "Unable to remove recording file during {context}: path={} error={err}",
            path.display()
        ));
    }
}

fn is_process_alive(process_id: u32) -> bool {
    unsafe {
        let Ok(handle) = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, process_id) else {
            return false;
        };
        let mut exit_code = 0u32;
        let result = GetExitCodeProcess(handle, &mut exit_code).is_ok() && exit_code == 259;
        if let Err(err) = CloseHandle(handle) {
            crate::log_debug(&format!("Unable to close process handle: {err}"));
        }
        result
    }
}

fn activity_dir() -> PathBuf {
    crate::settings::settings_dir().join("RecordingActivities")
}

fn activity_path(id: &str) -> PathBuf {
    activity_dir().join(format!("{id}.json"))
}

fn scheduled_cancel_dir() -> PathBuf {
    crate::settings::settings_dir().join("ScheduledRecordingCancellations")
}

fn scheduled_cancel_path(id: &str) -> PathBuf {
    let safe_id = id
        .chars()
        .filter(|character| character.is_ascii_alphanumeric() || *character == '-')
        .collect::<String>();
    scheduled_cancel_dir().join(format!("{safe_id}.cancel"))
}

fn clear_scheduled_cancel_request(id: &str) {
    let path = scheduled_cancel_path(id);
    if let Err(error) = fs::remove_file(&path)
        && error.kind() != std::io::ErrorKind::NotFound
    {
        crate::log_debug(&format!(
            "Scheduled recording cancellation marker removal failed id={id} path={} error={error}",
            path.display()
        ));
    }
}

fn scheduled_cancel_requested(id: &str) -> bool {
    scheduled_cancel_path(id).is_file()
}

fn request_scheduled_cancel(id: &str) -> Result<(), String> {
    fs::create_dir_all(scheduled_cancel_dir()).map_err(|error| error.to_string())?;
    let path = scheduled_cancel_path(id);
    fs::write(&path, b"stop\n").map_err(|error| error.to_string())?;
    crate::log_debug(&format!(
        "Scheduled recording graceful stop requested id={id} marker={}",
        path.display()
    ));
    Ok(())
}

fn create_activity(
    kind: StreamRecordingKind,
    title: &str,
    output_path: &Path,
    expected_end_unix: Option<u64>,
    scheduled_id: Option<&str>,
) -> Result<RecordingActivity, String> {
    fs::create_dir_all(activity_dir()).map_err(|error| error.to_string())?;
    let activity = RecordingActivity {
        id: uuid::Uuid::new_v4().to_string(),
        kind,
        title: title.trim().to_string(),
        output_path: output_path.to_string_lossy().to_string(),
        process_id: std::process::id(),
        started_unix: unix_now(),
        expected_end_unix,
        scheduled_id: scheduled_id.map(ToOwned::to_owned),
    };
    let payload = serde_json::to_vec_pretty(&activity).map_err(|error| error.to_string())?;
    let path = activity_path(&activity.id);
    let temp_path = path.with_extension("json.tmp");
    fs::write(&temp_path, payload).map_err(|error| error.to_string())?;
    fs::rename(&temp_path, &path).map_err(|error| error.to_string())?;
    Ok(activity)
}

fn remove_activity(id: &str) {
    if let Err(error) = fs::remove_file(activity_path(id))
        && error.kind() != std::io::ErrorKind::NotFound
    {
        crate::log_debug(&format!(
            "Recording activity removal failed id={id} error={error}"
        ));
    }
}

fn list_active_recordings(kind: StreamRecordingKind) -> Vec<RecordingActivity> {
    let mut activities = Vec::new();
    let Ok(entries) = fs::read_dir(activity_dir()) else {
        return activities;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|value| value.to_str()) != Some("json") {
            continue;
        }
        let Ok(payload) = fs::read(&path) else {
            continue;
        };
        let Ok(activity) = serde_json::from_slice::<RecordingActivity>(&payload) else {
            crate::log_if_err!(
                fs::remove_file(&path),
                "Removing invalid recording activity failed"
            );
            continue;
        };
        if !is_process_alive(activity.process_id) {
            crate::log_if_err!(
                fs::remove_file(&path),
                "Removing stale recording activity failed"
            );
            continue;
        }
        if activity.kind == kind {
            activities.push(activity);
        }
    }
    activities.sort_by_key(|activity| activity.started_unix);
    activities
}

fn active_recording_from_list_id(
    kind: StreamRecordingKind,
    list_id: &str,
) -> Option<RecordingActivity> {
    let activity_id = list_id.strip_prefix("active:")?;
    let payload = fs::read(activity_path(activity_id)).ok()?;
    let activity = serde_json::from_slice::<RecordingActivity>(&payload).ok()?;
    if activity.kind != kind || !is_process_alive(activity.process_id) {
        return None;
    }
    Some(activity)
}

fn stop_active_recording(
    kind: StreamRecordingKind,
    list_id: &str,
    playback_parent: HWND,
) -> Result<(), String> {
    let activity = active_recording_from_list_id(kind, list_id)
        .ok_or_else(|| "La registrazione non risulta più in corso.".to_string())?;

    if let Some(scheduled_id) = activity.scheduled_id.as_deref() {
        request_scheduled_cancel(scheduled_id)?;
        crate::log_debug(&format!(
            "Requested graceful stop for active scheduled recording activity_id={} scheduled_id={} title={}",
            activity.id, scheduled_id, activity.title
        ));
        return Ok(());
    }

    // Immediate radio/TV recordings are tied to the managed mpv player in this
    // Sonarpad process. Stopping the player already drives the existing graceful
    // finalization path for the recording, so reuse it instead of adding a second
    // stop mechanism.
    if activity.process_id == std::process::id() {
        crate::stop_managed_mpv_playback(playback_parent);
        crate::log_debug(&format!(
            "Requested stop for active immediate recording activity_id={} title={}",
            activity.id, activity.title
        ));
        return Ok(());
    }

    Err(
        "Non è stato possibile collegare la registrazione al processo che la sta eseguendo."
            .to_string(),
    )
}

fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn next_recording_path(
    kind: StreamRecordingKind,
    title: &str,
    ui_language: Option<Language>,
) -> Result<PathBuf, String> {
    let folder = recordings_folder(kind);
    fs::create_dir_all(&folder).map_err(|err| {
        recording_format(
            ui_language,
            "radio.recording_error_create_folder",
            &[("error", err.to_string())],
            "Impossibile creare la cartella delle registrazioni: {error}",
        )
    })?;
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let safe_title = sanitize_file_name(title);
    Ok(folder.join(format!(
        "{}_{}.{}",
        if safe_title.is_empty() {
            match kind {
                StreamRecordingKind::Radio => "Radio",
                StreamRecordingKind::Tv => "TV",
            }
        } else {
            &safe_title
        },
        timestamp,
        kind.extension()
    )))
}

pub(crate) fn recordings_folder(kind: StreamRecordingKind) -> PathBuf {
    let settings = crate::settings::load_settings();
    match kind {
        StreamRecordingKind::Radio => {
            if settings.radio_save_folder.trim().is_empty() {
                PathBuf::from(crate::settings::default_radio_save_folder())
            } else {
                PathBuf::from(settings.radio_save_folder)
            }
        }
        StreamRecordingKind::Tv => {
            if settings.tv_save_folder.trim().is_empty() {
                PathBuf::from(crate::settings::default_tv_save_folder())
            } else {
                PathBuf::from(settings.tv_save_folder)
            }
        }
    }
}

fn mixed_legacy_recordings_folder() -> PathBuf {
    let settings = crate::settings::load_settings();
    if settings.podcast_save_folder.trim().is_empty() {
        PathBuf::from(crate::settings::default_podcast_save_folder())
    } else {
        PathBuf::from(settings.podcast_save_folder)
    }
}

fn legacy_recordings_folder(kind: StreamRecordingKind) -> PathBuf {
    crate::settings::settings_dir().join(kind.folder_name())
}

fn sanitize_file_name(value: &str) -> String {
    let mut result = value
        .trim()
        .chars()
        .map(|ch| match ch {
            '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*' => '_',
            ch if ch.is_control() => '_',
            ch => ch,
        })
        .collect::<String>();
    while result.ends_with(' ') || result.ends_with('.') {
        result.pop();
    }
    result.chars().take(100).collect()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum OpenRecordingsResult {
    Closed,
    ReturnToSearch,
    PlaybackStarted,
}

pub(crate) fn open_recordings(
    parent: HWND,
    playback_parent: HWND,
    language: Language,
    kind: StreamRecordingKind,
) -> OpenRecordingsResult {
    let mut selected_id: Option<String> = None;
    let mut close_silently_if_empty = false;
    loop {
        let entries = build_recording_entries(language, kind);
        if entries.is_empty() {
            if close_silently_if_empty {
                crate::log_debug(
                    "Recordings list is empty after deletion; closing without an error.",
                );
                return OpenRecordingsResult::Closed;
            }
            let text = match kind {
                StreamRecordingKind::Radio => translated(
                    language,
                    "radio.recordings_empty",
                    "Non ci sono registrazioni radio.",
                ),
                StreamRecordingKind::Tv => translated(
                    language,
                    "recordings.tv_empty",
                    "Non ci sono registrazioni TV.",
                ),
            };
            let title = translated(language, "app.warning_title", "Attenzione");
            let title_wide = to_wide(&title);
            let text_wide = to_wide(&text);
            crate::message_box_modal(
                parent,
                PCWSTR(text_wide.as_ptr()),
                PCWSTR(title_wide.as_ptr()),
                MB_OK | MB_ICONWARNING,
            );
            return OpenRecordingsResult::Closed;
        }

        let items = entries
            .iter()
            .map(RecordingListEntry::as_item)
            .collect::<Vec<_>>();

        let delete_label = translated(language, "radio.delete_recording", "Elimina registrazione");
        let delete_confirmation = match kind {
            StreamRecordingKind::Radio => translated(
                language,
                "radio.confirm_delete_recording",
                "Eliminare la registrazione radio selezionata?",
            ),
            StreamRecordingKind::Tv => crate::i18n::tr_tv("tv.confirm_delete_recording"),
        };
        let delete_confirmation_title = crate::settings::confirm_title(language);
        let recording_deleted = Arc::new(AtomicBool::new(false));
        let recording_deleted_handler = Arc::clone(&recording_deleted);
        let delete_action = InterpreterContextAction {
            label: delete_label,
            ctrl_c_shortcut: false,
            delete_shortcut: true,
            children: Vec::new(),
            enabled: Arc::new(|id| Path::new(id).is_file()),
            handler: Arc::new(move |id| {
                let dialog = crate::get_foreground_window_safe();
                let confirmation_wide = to_wide(&delete_confirmation);
                let title_wide = to_wide(&delete_confirmation_title);
                let response = crate::message_box_modal(
                    dialog,
                    PCWSTR(confirmation_wide.as_ptr()),
                    PCWSTR(title_wide.as_ptr()),
                    MB_YESNO | MB_ICONQUESTION,
                );
                if response != IDYES {
                    return;
                }

                match fs::remove_file(&id) {
                    Ok(()) => {
                        recording_deleted_handler.store(true, Ordering::SeqCst);
                        crate::log_debug(&format!(
                            "Recording deleted after confirmation; refreshing list immediately: {id}"
                        ));
                        if dialog.0 != 0 && crate::is_window_handle_valid(dialog) {
                            crate::log_if_err!(crate::post_message_w_safe(
                                dialog,
                                windows::Win32::UI::WindowsAndMessaging::WM_CLOSE,
                                windows::Win32::Foundation::WPARAM(0),
                                windows::Win32::Foundation::LPARAM(0),
                            ));
                        }
                    }
                    Err(err) => {
                        crate::log_debug(&format!("Recording delete failed for {id}: {err}"));
                    }
                }
            }),
        };

        let stop_label = translated(
            language,
            "recordings.stop_in_progress",
            "Interrompi registrazione",
        );
        let stop_confirmation = translated(
            language,
            "recordings.stop_in_progress_confirm",
            "Interrompere la registrazione in corso? Il contenuto registrato finora verrà conservato se il file è valido.",
        );
        let stop_confirmation_title = crate::settings::confirm_title(language);
        let playback_parent_value = playback_parent.0;
        let stop_action = InterpreterContextAction {
            label: stop_label,
            ctrl_c_shortcut: false,
            delete_shortcut: false,
            children: Vec::new(),
            enabled: Arc::new(|id| id.starts_with("active:")),
            handler: Arc::new(move |id| {
                let dialog = crate::get_foreground_window_safe();
                let confirmation_wide = to_wide(&stop_confirmation);
                let title_wide = to_wide(&stop_confirmation_title);
                let response = crate::message_box_modal(
                    dialog,
                    PCWSTR(confirmation_wide.as_ptr()),
                    PCWSTR(title_wide.as_ptr()),
                    MB_YESNO | MB_ICONQUESTION,
                );
                if response != IDYES {
                    return;
                }

                if let Err(error) = stop_active_recording(kind, &id, HWND(playback_parent_value)) {
                    crate::show_error(dialog, language, &error);
                }
            }),
        };

        let cancel_scheduled_label = translated(
            language,
            "recordings.cancel_scheduled",
            "Annulla registrazione programmata",
        );
        let cancel_scheduled_confirmation = translated(
            language,
            "recordings.cancel_scheduled_confirm",
            "Annullare la registrazione programmata selezionata?",
        );
        let cancel_scheduled_confirmation_title = crate::settings::confirm_title(language);
        let scheduled_cancelled = Arc::new(AtomicBool::new(false));
        let scheduled_cancelled_handler = Arc::clone(&scheduled_cancelled);
        let cancel_scheduled_action = InterpreterContextAction {
            label: cancel_scheduled_label,
            ctrl_c_shortcut: false,
            delete_shortcut: false,
            children: Vec::new(),
            enabled: Arc::new(|id| id.starts_with("scheduled:")),
            handler: Arc::new(move |id| {
                let dialog = crate::get_foreground_window_safe();
                let confirmation_wide = to_wide(&cancel_scheduled_confirmation);
                let title_wide = to_wide(&cancel_scheduled_confirmation_title);
                let response = crate::message_box_modal(
                    dialog,
                    PCWSTR(confirmation_wide.as_ptr()),
                    PCWSTR(title_wide.as_ptr()),
                    MB_YESNO | MB_ICONQUESTION,
                );
                if response != IDYES {
                    return;
                }

                let Some(scheduled_id) = id.strip_prefix("scheduled:") else {
                    return;
                };
                match crate::app_windows::scheduled_recording_window::cancel_scheduled_recording(
                    scheduled_id,
                ) {
                    Ok(()) => {
                        scheduled_cancelled_handler.store(true, Ordering::SeqCst);
                        if dialog.0 != 0 && crate::is_window_handle_valid(dialog) {
                            crate::log_if_err!(crate::post_message_w_safe(
                                dialog,
                                windows::Win32::UI::WindowsAndMessaging::WM_CLOSE,
                                windows::Win32::Foundation::WPARAM(0),
                                windows::Win32::Foundation::LPARAM(0),
                            ));
                        }
                    }
                    Err(error) => crate::show_error(dialog, language, &error),
                }
            }),
        };

        let title = match kind {
            StreamRecordingKind::Radio => {
                translated(language, "radio.recordings", "Registrazioni radio")
            }
            StreamRecordingKind::Tv => {
                translated(language, "recordings.tv_title", "Registrazioni TV")
            }
        };
        let result = youtube_transcript_window::select_multiline_items_with_search(
            parent,
            language,
            title,
            items,
            selected_id.clone(),
            MultilineSearchOptions {
                initial_query: String::new(),
                search_button_label: translated(
                    language,
                    "recordings.back_to_search",
                    "Torna alla ricerca",
                ),
                show_search_edit: false,
                secondary_action_label: None,
                context_actions: vec![stop_action, cancel_scheduled_action, delete_action],
                right_arrow_accepts_selection: true,
                left_arrow_closes: true,
                escape_stops_active_player: kind == StreamRecordingKind::Tv,
                refresh: Some(MultilineRefreshOptions {
                    interval_ms: 1_000,
                    loader: Arc::new(move || {
                        build_recording_entries(language, kind)
                            .into_iter()
                            .map(|entry| entry.as_item())
                            .collect()
                    }),
                }),
            },
        );

        if recording_deleted.swap(false, Ordering::SeqCst) {
            selected_id = None;
            close_silently_if_empty = true;
            continue;
        }
        if scheduled_cancelled.swap(false, Ordering::SeqCst) {
            selected_id = None;
            close_silently_if_empty = true;
            continue;
        }
        close_silently_if_empty = false;

        match result {
            MultilineSelectionResult::Selected(id) => {
                selected_id = Some(id.clone());
                // The list can refresh while it is open, so resolve the selected ID
                // against a fresh snapshot rather than the entries used at creation.
                let current_entries = build_recording_entries(language, kind);
                let selected = current_entries.iter().find(|entry| entry.id() == id);
                let Some(RecordingListEntry::File(path)) = selected else {
                    continue;
                };
                let playback_result = match kind {
                    StreamRecordingKind::Radio => {
                        let path_text = path.to_string_lossy().to_string();
                        let title = path
                            .file_name()
                            .and_then(|name| name.to_str())
                            .unwrap_or("Registrazione radio");
                        crate::log_debug(&format!(
                            "Opening radio recording through stream player: {}",
                            path.display()
                        ));
                        crate::launch_stream_url_in_mpv(
                            playback_parent,
                            &path_text,
                            Some(title),
                            None,
                            None,
                            None,
                        )
                    }
                    StreamRecordingKind::Tv => {
                        crate::launch_local_tv_recording_in_mpv(playback_parent, path)
                    }
                };
                if let Err(err) = playback_result {
                    crate::show_error(parent, language, &err);
                    if kind == StreamRecordingKind::Radio {
                        return OpenRecordingsResult::ReturnToSearch;
                    }
                    continue;
                }
                if kind == StreamRecordingKind::Radio {
                    return OpenRecordingsResult::PlaybackStarted;
                }
                continue;
            }
            MultilineSelectionResult::Search(_) => {
                return OpenRecordingsResult::ReturnToSearch;
            }
            MultilineSelectionResult::Cancelled => return OpenRecordingsResult::Closed,
            MultilineSelectionResult::SecondaryAction => {}
        }
    }
}

fn build_recording_entries(
    language: Language,
    kind: StreamRecordingKind,
) -> Vec<RecordingListEntry> {
    let active = list_active_recordings(kind);
    let active_schedule_ids = active
        .iter()
        .filter_map(|activity| activity.scheduled_id.clone())
        .collect::<std::collections::HashSet<_>>();
    let active_output_paths = active
        .iter()
        .map(|activity| PathBuf::from(&activity.output_path))
        .collect::<std::collections::HashSet<_>>();

    let mut entries = active
        .into_iter()
        .map(|activity| {
            let status = translated(
                language,
                "recordings.status_in_progress",
                "Registrazione in corso",
            );
            let description = remove_repeated_status_prefix(
                active_recording_description(language, &activity),
                &status,
            );
            RecordingListEntry::Active {
                id: format!("active:{}", activity.id),
                title: format!("{} — {}", activity.title, status),
                description,
            }
        })
        .collect::<Vec<_>>();

    entries.extend(
        crate::app_windows::scheduled_recording_window::list_scheduled_recordings(kind)
            .into_iter()
            .filter(|item| !active_schedule_ids.contains(item.id.as_str()))
            .map(|item| {
                let status = translated(
                    language,
                    "recordings.status_scheduled",
                    "Registrazione programmata",
                );
                RecordingListEntry::Scheduled {
                    id: format!("scheduled:{}", item.id),
                    title: format!("{} — {}", item.title, status),
                    description: remove_repeated_status_prefix(
                        scheduled_recording_description(
                            language,
                            item.start_at,
                            item.duration_minutes,
                        ),
                        &status,
                    ),
                }
            }),
    );

    entries.extend(
        list_recordings(kind)
            .into_iter()
            .filter(|path| !active_output_paths.contains(path))
            .map(RecordingListEntry::File),
    );
    entries
}

fn remove_repeated_status_prefix(description: String, status: &str) -> String {
    let trimmed = description.trim_start();
    let Some(remainder) = trimmed.strip_prefix(status) else {
        return description;
    };
    let remainder = remainder.trim_start_matches(|character: char| {
        character.is_whitespace()
            || matches!(
                character,
                '.' | ':' | ';' | ',' | '-' | '–' | '—' | '。' | '：' | '；'
            )
    });
    if remainder.is_empty() {
        return description;
    }

    let mut characters = remainder.chars();
    let Some(first) = characters.next() else {
        return description;
    };
    first.to_uppercase().chain(characters).collect()
}

fn active_recording_description(language: Language, activity: &RecordingActivity) -> String {
    let started = format_unix_time(activity.started_unix);
    if let Some(expected_end) = activity.expected_end_unix {
        let end = format_unix_time(expected_end);
        recording_format(
            Some(language),
            "recordings.in_progress_times",
            &[("start", started), ("end", end)],
            "Registrazione in corso. Iniziata alle {start}; termine previsto alle {end}.",
        )
    } else {
        recording_format(
            Some(language),
            "recordings.in_progress_since",
            &[("start", started)],
            "Registrazione in corso dalle {start}.",
        )
    }
}

fn scheduled_recording_description(
    language: Language,
    start_at: NaiveDateTime,
    duration_minutes: u32,
) -> String {
    let now = Local::now().naive_local();
    let seconds = (start_at - now).num_seconds();
    let date = start_at.format("%d/%m/%Y").to_string();
    let time = start_at.format("%H:%M").to_string();
    if (0..=3600).contains(&seconds) {
        let minutes = ((seconds + 59) / 60).max(0);
        let key = if minutes == 1 {
            "recordings.scheduled_countdown_one"
        } else {
            "recordings.scheduled_countdown"
        };
        let fallback = if minutes == 1 {
            "Registrazione programmata alle {time}. Manca 1 minuto all'inizio. Durata: {duration} minuti."
        } else {
            "Registrazione programmata alle {time}. Mancano {minutes} minuti all'inizio. Durata: {duration} minuti."
        };
        recording_format(
            Some(language),
            key,
            &[
                ("time", time),
                ("minutes", minutes.to_string()),
                ("duration", duration_minutes.to_string()),
            ],
            fallback,
        )
    } else {
        recording_format(
            Some(language),
            "recordings.scheduled_datetime",
            &[
                ("date", date),
                ("time", time),
                ("duration", duration_minutes.to_string()),
            ],
            "Registrazione programmata per il {date} alle {time}. Durata: {duration} minuti.",
        )
    }
}

fn format_unix_time(timestamp: u64) -> String {
    let system_time = UNIX_EPOCH + Duration::from_secs(timestamp);
    let datetime: chrono::DateTime<Local> = system_time.into();
    datetime.format("%H:%M").to_string()
}

fn list_recordings(kind: StreamRecordingKind) -> Vec<PathBuf> {
    let mut folders = vec![recordings_folder(kind)];
    for legacy in [
        mixed_legacy_recordings_folder(),
        PathBuf::from(crate::settings::default_podcast_save_folder()),
        legacy_recordings_folder(kind),
    ] {
        if !folders.iter().any(|folder| folder == &legacy) {
            folders.push(legacy);
        }
    }

    let mut files = folders
        .into_iter()
        .flat_map(|folder| {
            fs::read_dir(folder)
                .ok()
                .into_iter()
                .flatten()
                .filter_map(Result::ok)
                .map(|entry| entry.path())
                .collect::<Vec<_>>()
        })
        .filter(|path| path.is_file())
        .filter(|path| !is_podcast_recording_file(path))
        .filter(|path| {
            let extension = path
                .extension()
                .and_then(|value| value.to_str())
                .map(str::to_ascii_lowercase);
            match (kind, extension.as_deref()) {
                (StreamRecordingKind::Radio, Some(extension)) => {
                    matches!(extension, "mp3" | "mka" | "aac" | "wav")
                }
                (StreamRecordingKind::Tv, Some(extension)) => {
                    matches!(extension, "mp4" | "mkv" | "ts")
                }
                _ => false,
            }
        })
        .collect::<Vec<_>>();
    files.sort_by(|a, b| {
        fs::metadata(b)
            .and_then(|metadata| metadata.modified())
            .ok()
            .cmp(
                &fs::metadata(a)
                    .and_then(|metadata| metadata.modified())
                    .ok(),
            )
    });
    files
}

fn is_podcast_recording_file(path: &Path) -> bool {
    path.file_name()
        .and_then(|value| value.to_str())
        .map(|name| name.to_ascii_lowercase().starts_with("podcast_"))
        .unwrap_or(false)
}

fn format_file_size(bytes: u64) -> String {
    if bytes >= 1_073_741_824 {
        format!("{:.2} GB", bytes as f64 / 1_073_741_824.0)
    } else if bytes >= 1_048_576 {
        format!("{:.1} MB", bytes as f64 / 1_048_576.0)
    } else if bytes >= 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else {
        format!("{bytes} byte")
    }
}

fn recording_text(language: Option<Language>, key: &str, fallback: &str) -> String {
    language
        .map(|language| translated(language, key, fallback))
        .unwrap_or_else(|| fallback.to_string())
}

fn recording_format(
    language: Option<Language>,
    key: &str,
    args: &[(&str, String)],
    fallback: &str,
) -> String {
    if let Some(language) = language {
        let borrowed = args
            .iter()
            .map(|(name, value)| (*name, value.as_str()))
            .collect::<Vec<_>>();
        let value = crate::i18n::tr_f(language, key, &borrowed);
        if value != key {
            return value;
        }
    }
    let mut value = fallback.to_string();
    for (name, replacement) in args {
        value = value.replace(&format!("{{{name}}}"), replacement);
    }
    value
}

fn translated(language: Language, key: &str, fallback: &str) -> String {
    let value = crate::i18n::tr(language, key);
    if value == key {
        fallback.to_string()
    } else {
        value
    }
}
