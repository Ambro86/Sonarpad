use crate::ffmpeg_dyn::*;
use crate::ffmpeg_source::{FfmpegApi, FfmpegSource, ffmpeg_api, list_audio_streams};
use crate::log_debug;
use crate::sapi4_engine;
use crate::sapi5_engine;
use crate::settings::AppSettings;
use crate::subtitle_wasapi::{decode_mp3_to_pcm, resample_pcm};
use crate::subtitles::{SubtitleCue, find_subtitle_for_media, load_subtitles};
use crate::tts_engine;
use rodio::Source;
use sha2::Digest;
use std::collections::VecDeque;
use std::ffi::CString;
use std::fs;
use std::path::{Path, PathBuf};
use std::ptr;
use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use windows::Win32::Foundation::HWND;

const MIX_SUFFIX: &str = "_tts_mix_";
const FILM_BASE_VOLUME: f32 = 0.7;
const DESC_VOLUME: f32 = 0.7;
const DUCK_MULTIPLIER: f32 = 0.35;
const DUCK_PAD_BEFORE_SEC: f32 = 0.0;
const DUCK_PAD_AFTER_SEC: f32 = 0.0;
const DUCK_END_TRIM_SEC: f32 = 0.90;
const DUCK_START_DELAY_SEC: f32 = 0.28;

static MIX_OUTPUTS: OnceLock<Mutex<Vec<PathBuf>>> = OnceLock::new();
const DEFAULT_AAC_BITRATE: i64 = 192_000;
const AVIO_FLAG_WRITE: i32 = 2;
const AV_CODEC_FLAG_QSCALE_FALLBACK: i32 = 1 << 1;
const FF_QP2LAMBDA_FALLBACK: i32 = 118;
const AV_NOPTS_VALUE_I64: i64 = i64::MIN;

struct MuxProgressState {
    start_us: Option<i64>,
    cursor_us: i64,
    last_pct: Option<u32>,
    last_hls_log_pct_bucket: Option<u32>,
}

struct HlsProgressEstimate {
    estimated_total_bytes: u64,
    variant_url: String,
    bandwidth_bits_per_sec: u64,
    duration_secs: f64,
    total_duration_us: i64,
}

struct HlsRecordingInputs {
    video_url: String,
    audio_url: String,
}

struct HlsAudioRendition {
    group_id: String,
    uri: Option<String>,
    language: String,
    name: String,
    is_default: bool,
    autoselect: bool,
}

struct HlsVideoVariant {
    url: String,
    bandwidth_bits_per_sec: u64,
    audio_group: Option<String>,
}

struct MuxProgressSample<'a> {
    pts: i64,
    dts: i64,
    duration: i64,
    time_base: AVRational,
    out_path: &'a Path,
    hls_progress_estimate: Option<&'a HlsProgressEstimate>,
}

fn ffmpeg_error_text(api: &FfmpegApi, code: i32) -> String {
    crate::ffmpeg_source::ffmpeg_err(api, code)
}

#[derive(Clone)]
pub struct MixExportOptions {
    pub ducking: bool,
}

/// A synthesized audio-description cue ready for final mixing. `samples` are
/// interleaved floating-point PCM in the declared format. Extended cues pause
/// the source timeline and are inserted without consuming movie samples.
#[derive(Clone)]
pub struct AudioDescriptionMixCue {
    pub start_sec: f64,
    pub samples: Arc<[f32]>,
    pub sample_rate: u32,
    pub channels: u16,
    pub extended_pause: bool,
}

#[derive(Clone)]
pub struct AudioDescriptionExportOptions {
    pub ducking_db: f32,
    pub fade_ms: u32,
    pub bitrate_kbps: u32,
    pub cancel: Arc<AtomicBool>,
}

struct CueAudio {
    start_sample: u64,
    samples: Arc<[f32]>,
}

struct ActiveCue {
    samples: Arc<[f32]>,
    read_offset: usize,
}

fn tts_cache_dir(media_path: &Path, settings: &AppSettings) -> Result<PathBuf, String> {
    let base_dir = crate::settings::settings_dir().join("subtitle_cache");
    let mut hasher = sha2::Sha256::new();
    hasher.update(media_path.to_string_lossy().as_bytes());
    let engine_key = match settings.tts_engine {
        crate::settings::TtsEngine::Edge => "edge",
        crate::settings::TtsEngine::Sapi5 => "sapi5",
        crate::settings::TtsEngine::Sapi4 => "sapi4",
        crate::settings::TtsEngine::Google => "google",
    };
    hasher.update(engine_key.as_bytes());
    hasher.update(settings.tts_voice.as_bytes());
    let hash = hex::encode(hasher.finalize());
    Ok(base_dir.join(&hash[..16]))
}

fn parse_sapi4_voice_index(voice: &str) -> i32 {
    if let Some(hash_pos) = voice.find('#') {
        let rest = &voice[hash_pos + 1..];
        if let Some(pipe_pos) = rest.find('|') {
            return rest[..pipe_pos].parse::<i32>().unwrap_or(0);
        }
        return rest.parse::<i32>().unwrap_or(0);
    }
    0
}

fn ensure_tts_cache(
    media_path: &Path,
    subtitle_path: &Path,
    settings: &AppSettings,
) -> Result<Vec<SubtitleCue>, String> {
    let cues = load_subtitles(subtitle_path)?;
    if cues.is_empty() {
        return Err("Subtitle: no cues to export".to_string());
    }
    let cache_dir = tts_cache_dir(media_path, settings)?;
    if !cache_dir.exists() {
        std::fs::create_dir_all(&cache_dir)
            .map_err(|e| format!("Subtitle: cache dir create failed: {}", e))?;
    }
    for (idx, cue) in cues.iter().enumerate() {
        let ext = match settings.tts_engine {
            crate::settings::TtsEngine::Edge => "mp3",
            _ => "wav",
        };
        let path = cache_dir.join(format!("cue_{:04}.{}", idx, ext));
        if path.exists() {
            continue;
        }
        let text = cue.text.trim();
        if text.is_empty() {
            continue;
        }
        let chunks = tts_engine::split_into_tts_chunks(text, false, &[], settings.tts_engine);
        let has_overrides = chunks.iter().any(|chunk| chunk.override_voice.is_some());

        match settings.tts_engine {
            crate::settings::TtsEngine::Edge => {
                if has_overrides {
                    let cancel = Arc::new(std::sync::atomic::AtomicBool::new(false));
                    let options = tts_engine::AudiobookCommonOptions {
                        voice: &settings.tts_voice,
                        output: &path,
                        progress_hwnd: HWND(0),
                        cancel,
                        language: settings.language,
                        part_naming_mode: crate::settings::AudiobookPartNamingMode::TitleNumber,
                        part_announcement_mode:
                            crate::settings::AudiobookPartAnnouncementMode::None,
                        audiobook_title: "",
                        audiobook_bitrate_kbps: settings.audiobook_m4b_bitrate,
                        rate: settings.tts_rate,
                        pitch: settings.tts_pitch,
                        volume: settings.tts_volume,
                        sapi4_threads: None,
                    };
                    let config = tts_engine::MixedAudiobookConfig {
                        main_engine: settings.tts_engine,
                    };
                    let mut progress = 0usize;
                    tts_engine::render_mixed_audiobook_part(
                        &chunks,
                        &mut progress,
                        &path,
                        &options,
                        &config,
                    )?;
                } else {
                    let rt = tokio::runtime::Runtime::new()
                        .map_err(|e| format!("Subtitle: failed to create runtime: {}", e))?;
                    let request_id = uuid::Uuid::new_v4().simple().to_string();
                    match rt.block_on(tts_engine::download_audio_chunk(
                        text,
                        &settings.tts_voice,
                        &request_id,
                        settings.tts_rate,
                        settings.tts_pitch,
                        settings.tts_volume,
                        settings.language,
                    )) {
                        Ok(bytes) => {
                            std::fs::write(&path, bytes).map_err(|e| {
                                format!("Subtitle: failed to write audio chunk: {}", e)
                            })?;
                        }
                        Err(err) => {
                            return Err(format!("Subtitle: download failed: {}", err));
                        }
                    }
                }
            }
            crate::settings::TtsEngine::Google => {
                let cancel = Arc::new(std::sync::atomic::AtomicBool::new(false));
                let options = tts_engine::AudiobookCommonOptions {
                    voice: &settings.tts_voice,
                    output: &path,
                    progress_hwnd: HWND(0),
                    cancel,
                    language: settings.language,
                    part_naming_mode: crate::settings::AudiobookPartNamingMode::TitleNumber,
                    part_announcement_mode: crate::settings::AudiobookPartAnnouncementMode::None,
                    audiobook_title: "",
                    audiobook_bitrate_kbps: settings.audiobook_m4b_bitrate,
                    rate: settings.tts_rate,
                    pitch: settings.tts_pitch,
                    volume: settings.tts_volume,
                    sapi4_threads: None,
                };
                let config = tts_engine::MixedAudiobookConfig {
                    main_engine: settings.tts_engine,
                };
                let mut progress = 0usize;
                tts_engine::render_mixed_audiobook_part(
                    &chunks,
                    &mut progress,
                    &path,
                    &options,
                    &config,
                )?;
            }
            crate::settings::TtsEngine::Sapi5 => {
                if has_overrides {
                    let cancel = Arc::new(std::sync::atomic::AtomicBool::new(false));
                    let options = tts_engine::AudiobookCommonOptions {
                        voice: &settings.tts_voice,
                        output: &path,
                        progress_hwnd: HWND(0),
                        cancel,
                        language: settings.language,
                        part_naming_mode: crate::settings::AudiobookPartNamingMode::TitleNumber,
                        part_announcement_mode:
                            crate::settings::AudiobookPartAnnouncementMode::None,
                        audiobook_title: "",
                        audiobook_bitrate_kbps: settings.audiobook_m4b_bitrate,
                        rate: settings.tts_rate,
                        pitch: settings.tts_pitch,
                        volume: settings.tts_volume,
                        sapi4_threads: None,
                    };
                    let config = tts_engine::MixedAudiobookConfig {
                        main_engine: settings.tts_engine,
                    };
                    let mut progress = 0usize;
                    tts_engine::render_mixed_audiobook_part(
                        &chunks,
                        &mut progress,
                        &path,
                        &options,
                        &config,
                    )?;
                } else {
                    let cancel = Arc::new(std::sync::atomic::AtomicBool::new(false));
                    let options = sapi5_engine::SapiExportOptions {
                        chunks: &[text.to_string()],
                        voice_name: &settings.tts_voice,
                        output_path: &path,
                        language: settings.language,
                        rate: settings.tts_rate,
                        pitch: settings.tts_pitch,
                        volume: settings.tts_volume,
                        audiobook_bitrate_kbps: settings.audiobook_m4b_bitrate,
                        cancel,
                    };
                    sapi5_engine::speak_sapi_to_file(options, |_| {})?;
                }
            }
            crate::settings::TtsEngine::Sapi4 => {
                if has_overrides {
                    let cancel = Arc::new(std::sync::atomic::AtomicBool::new(false));
                    let options = tts_engine::AudiobookCommonOptions {
                        voice: &settings.tts_voice,
                        output: &path,
                        progress_hwnd: HWND(0),
                        cancel,
                        language: settings.language,
                        part_naming_mode: crate::settings::AudiobookPartNamingMode::TitleNumber,
                        part_announcement_mode:
                            crate::settings::AudiobookPartAnnouncementMode::None,
                        audiobook_title: "",
                        audiobook_bitrate_kbps: settings.audiobook_m4b_bitrate,
                        rate: settings.tts_rate,
                        pitch: settings.tts_pitch,
                        volume: settings.tts_volume,
                        sapi4_threads: None,
                    };
                    let config = tts_engine::MixedAudiobookConfig {
                        main_engine: settings.tts_engine,
                    };
                    let mut progress = 0usize;
                    tts_engine::render_mixed_audiobook_part(
                        &chunks,
                        &mut progress,
                        &path,
                        &options,
                        &config,
                    )?;
                } else {
                    let cancel = Arc::new(std::sync::atomic::AtomicBool::new(false));
                    let voice_idx = parse_sapi4_voice_index(&settings.tts_voice);
                    let options = sapi4_engine::Sapi4Options {
                        rate: settings.tts_rate,
                        pitch: settings.tts_pitch,
                        volume: settings.tts_volume,
                        mp3_bitrate_kbps: settings.audiobook_m4b_bitrate,
                        cancel,
                    };
                    sapi4_engine::speak_sapi4_to_file(
                        &[text.to_string()],
                        voice_idx,
                        &path,
                        options,
                        |_| {},
                    )?;
                }
            }
        }
    }
    Ok(cues)
}

fn collect_tts_audio(
    cues: &[SubtitleCue],
    media_path: &Path,
    settings: &AppSettings,
    sample_rate: u32,
    channels: u16,
    offset_secs: f64,
) -> Result<VecDeque<CueAudio>, String> {
    let cache_dir = tts_cache_dir(media_path, settings)?;
    let mut pending = VecDeque::new();
    for (idx, cue) in cues.iter().enumerate() {
        let mp3_path = cache_dir.join(format!("cue_{:04}.mp3", idx));
        let wav_path = cache_dir.join(format!("cue_{:04}.wav", idx));
        let path = if mp3_path.exists() {
            mp3_path
        } else if wav_path.exists() {
            wav_path
        } else {
            continue;
        };
        let bytes =
            std::fs::read(&path).map_err(|e| format!("Subtitle: failed to read cache: {}", e))?;
        let (samples, src_rate, src_ch) = decode_mp3_to_pcm(&bytes)?;
        let resampled = resample_pcm(&samples, src_rate, src_ch, sample_rate, channels);
        let start_secs = (cue.start.as_secs_f64() + offset_secs).max(0.0);
        let start_sample = (start_secs * sample_rate as f64 * channels as f64) as u64;
        pending.push_back(CueAudio {
            start_sample,
            samples: Arc::from(resampled),
        });
    }
    Ok(pending)
}

fn build_duck_intervals(
    pending: &VecDeque<CueAudio>,
    sample_rate: u32,
    channels: u16,
) -> Vec<(u64, u64)> {
    if pending.is_empty() {
        return Vec::new();
    }
    let samples_per_sec = sample_rate as f32 * channels as f32;
    let pad_before = (DUCK_PAD_BEFORE_SEC * samples_per_sec) as i64;
    let pad_after = (DUCK_PAD_AFTER_SEC * samples_per_sec) as i64;
    let start_delay = (DUCK_START_DELAY_SEC * samples_per_sec) as i64;
    let end_trim = (DUCK_END_TRIM_SEC * samples_per_sec) as i64;

    let mut intervals = Vec::new();
    for cue in pending.iter() {
        let start = cue.start_sample as i64;
        let mut duck_start = start - pad_before + start_delay;
        let duck_end = start + cue.samples.len() as i64 + pad_after - end_trim;
        if duck_end <= duck_start {
            continue;
        }
        if duck_start < 0 {
            duck_start = 0;
        }
        if duck_end < 0 {
            continue;
        }
        intervals.push((duck_start as u64, duck_end as u64));
    }
    if intervals.is_empty() {
        return intervals;
    }
    intervals.sort_by_key(|(start, _)| *start);
    let mut merged: Vec<(u64, u64)> = Vec::with_capacity(intervals.len());
    for (start, end) in intervals {
        if let Some(last) = merged.last_mut()
            && start <= last.1
        {
            last.1 = last.1.max(end);
            continue;
        }
        merged.push((start, end));
    }
    merged
}

fn register_mix_output(path: &Path) {
    let list = MIX_OUTPUTS.get_or_init(|| Mutex::new(Vec::new()));
    if let Ok(mut outputs) = list.lock()
        && !outputs.iter().any(|p| p == path)
    {
        outputs.push(path.to_path_buf());
    }
}

fn dict_set_str(
    api: &FfmpegApi,
    dict: &mut *mut AVDictionary,
    key: &str,
    value: &str,
) -> Result<(), String> {
    let key_c = CString::new(key).map_err(|_| "FFmpeg: invalid dict key".to_string())?;
    let value_c = CString::new(value).map_err(|_| "FFmpeg: invalid dict value".to_string())?;
    let ret = unsafe { (api.av_dict_set)(dict, key_c.as_ptr(), value_c.as_ptr(), 0) };
    if ret < 0 {
        return Err(format!("FFmpeg: failed to set {} (code {})", key, ret));
    }
    Ok(())
}

fn open_input_with_network_options(
    api: &FfmpegApi,
    input: *const i8,
    context: *mut *mut AVFormatContext,
    user_agent: Option<&str>,
    referer: Option<&str>,
    network_options: bool,
) -> i32 {
    let mut options: *mut AVDictionary = ptr::null_mut();
    if network_options {
        crate::log_if_err!(dict_set_str(api, &mut options, "reconnect", "1"));
        crate::log_if_err!(dict_set_str(api, &mut options, "reconnect_streamed", "1"));
        crate::log_if_err!(dict_set_str(api, &mut options, "reconnect_delay_max", "5"));
        crate::log_if_err!(dict_set_str(api, &mut options, "rw_timeout", "15000000"));
        crate::log_if_err!(dict_set_str(api, &mut options, "probesize", "10000000"));
        crate::log_if_err!(dict_set_str(
            api,
            &mut options,
            "analyzeduration",
            "10000000",
        ));
    }
    if let Some(user_agent) = user_agent.map(str::trim).filter(|value| !value.is_empty()) {
        crate::log_if_err!(dict_set_str(api, &mut options, "user_agent", user_agent));
    }
    if let Some(referer) = referer.map(str::trim).filter(|value| !value.is_empty()) {
        crate::log_if_err!(dict_set_str(api, &mut options, "referer", referer));
    }
    let result = crate::ffmpeg_source::avformat_open_input_safe(
        api,
        context,
        input,
        ptr::null_mut(),
        &mut options,
    );
    crate::ffmpeg_source::av_dict_free_safe(api, &mut options);
    result
}

fn segment_format_from_path(path: &Path) -> Option<&'static str> {
    let ext = path.extension()?.to_str()?.to_ascii_lowercase();
    match ext.as_str() {
        "mp3" => Some("mp3"),
        "m4a" | "m4b" | "mp4" => Some("ipod"),
        "aac" => Some("adts"),
        "ogg" => Some("ogg"),
        "opus" => Some("opus"),
        "flac" => Some("flac"),
        "wav" => Some("wav"),
        "mkv" => Some("matroska"),
        _ => None,
    }
}

#[derive(Clone, Copy, Default)]
struct SegmentTimestampState {
    next_dts: Option<i64>,
    last_dts: Option<i64>,
}

fn repair_segment_packet_timestamps(
    packet: *mut AVPacket,
    state: &mut SegmentTimestampState,
) -> bool {
    if packet.is_null() {
        return false;
    }

    let mut repaired = false;
    unsafe {
        let duration = (*packet).duration.max(1);
        let pts_missing = (*packet).pts == AV_NOPTS_VALUE_I64;
        let dts_missing = (*packet).dts == AV_NOPTS_VALUE_I64;

        if dts_missing {
            (*packet).dts = state
                .next_dts
                .or_else(|| {
                    if !pts_missing {
                        Some((*packet).pts)
                    } else {
                        None
                    }
                })
                .unwrap_or(0);
            repaired = true;
        }
        if pts_missing {
            (*packet).pts = (*packet).dts;
            repaired = true;
        }

        if let Some(last_dts) = state.last_dts
            && (*packet).dts <= last_dts
        {
            let shift = last_dts.saturating_add(1).saturating_sub((*packet).dts);
            (*packet).dts = (*packet).dts.saturating_add(shift);
            (*packet).pts = (*packet).pts.saturating_add(shift);
            repaired = true;
        }

        if (*packet).pts < (*packet).dts {
            (*packet).pts = (*packet).dts;
            repaired = true;
        }

        state.last_dts = Some((*packet).dts);
        state.next_dts = Some((*packet).dts.saturating_add(duration));
    }
    repaired
}

pub fn media_duration_seconds(path: &Path) -> Option<f64> {
    let api = ffmpeg_api().ok()?;
    let input_c = CString::new(path.to_string_lossy().as_bytes()).ok()?;
    let mut in_ctx: *mut AVFormatContext = ptr::null_mut();
    let open_ret = crate::ffmpeg_source::avformat_open_input_safe(
        api,
        &mut in_ctx,
        input_c.as_ptr(),
        ptr::null_mut(),
        ptr::null_mut(),
    );
    if open_ret < 0 || in_ctx.is_null() {
        return None;
    }
    let stream_info_ret =
        crate::ffmpeg_source::avformat_find_stream_info_safe(api, in_ctx, ptr::null_mut());
    if stream_info_ret < 0 {
        log_debug(&format!(
            "FFmpeg: media duration stream info failed with code {}",
            stream_info_ret
        ));
    }
    let duration = unsafe { (*in_ctx).duration };
    crate::ffmpeg_source::avformat_close_input_safe(api, &mut in_ctx);
    if duration > 0 {
        Some(duration as f64 / 1_000_000.0)
    } else {
        None
    }
}

pub(crate) fn media_duration_and_start_seconds(path: &Path) -> Option<(f64, f64)> {
    let api = ffmpeg_api().ok()?;
    let input_c = CString::new(path.to_string_lossy().as_bytes()).ok()?;
    let mut in_ctx: *mut AVFormatContext = ptr::null_mut();
    let open_ret = crate::ffmpeg_source::avformat_open_input_safe(
        api,
        &mut in_ctx,
        input_c.as_ptr(),
        ptr::null_mut(),
        ptr::null_mut(),
    );
    if open_ret < 0 || in_ctx.is_null() {
        return None;
    }
    let stream_info_ret =
        crate::ffmpeg_source::avformat_find_stream_info_safe(api, in_ctx, ptr::null_mut());
    if stream_info_ret < 0 {
        log_debug(&format!(
            "FFmpeg: media timing stream info failed with code {}",
            stream_info_ret
        ));
    }
    let duration = unsafe { (*in_ctx).duration };
    let start_time = unsafe { (*in_ctx).start_time };
    crate::ffmpeg_source::avformat_close_input_safe(api, &mut in_ctx);
    if duration <= 0 {
        return None;
    }
    let start_seconds = if start_time != AV_NOPTS_VALUE_I64 {
        start_time as f64 / 1_000_000.0
    } else {
        0.0
    };
    Some((duration as f64 / 1_000_000.0, start_seconds))
}

pub fn media_duration_secs(path: &Path) -> Option<u64> {
    media_duration_seconds(path).map(|duration| duration.ceil() as u64)
}

#[derive(Clone, Copy, Default)]
struct SegmentMediaOptions {
    audio_only: bool,
    video_only: bool,
    tolerate_invalid_analysis_packets: bool,
    preferred_audio_stream_index: Option<i32>,
}

pub fn segment_media_file(
    input_path: &Path,
    output_pattern: &Path,
    segment_seconds: u32,
    start_number: u32,
    progress: Option<&mut dyn FnMut(u32)>,
) -> Result<(), String> {
    segment_media_file_inner(
        input_path,
        output_pattern,
        segment_seconds,
        start_number,
        SegmentMediaOptions::default(),
        None,
        progress,
    )
}

pub(crate) fn segment_media_file_for_analysis(
    input_path: &Path,
    output_pattern: &Path,
    segment_seconds: u32,
    start_number: u32,
    preferred_audio_stream_index: Option<i32>,
    cancel: &Arc<AtomicBool>,
    progress: Option<&mut dyn FnMut(u32)>,
) -> Result<(), String> {
    segment_media_file_inner(
        input_path,
        output_pattern,
        segment_seconds,
        start_number,
        SegmentMediaOptions {
            tolerate_invalid_analysis_packets: true,
            preferred_audio_stream_index,
            ..SegmentMediaOptions::default()
        },
        Some(cancel),
        progress,
    )
}

pub(crate) fn segment_media_file_for_analysis_video_only(
    input_path: &Path,
    output_pattern: &Path,
    segment_seconds: u32,
    start_number: u32,
    cancel: &Arc<AtomicBool>,
    progress: Option<&mut dyn FnMut(u32)>,
) -> Result<(), String> {
    segment_media_file_inner(
        input_path,
        output_pattern,
        segment_seconds,
        start_number,
        SegmentMediaOptions {
            video_only: true,
            tolerate_invalid_analysis_packets: true,
            ..SegmentMediaOptions::default()
        },
        Some(cancel),
        progress,
    )
}

pub fn segment_audio_file(
    input_path: &Path,
    output_pattern: &Path,
    segment_seconds: u32,
    start_number: u32,
) -> Result<(), String> {
    segment_media_file_inner(
        input_path,
        output_pattern,
        segment_seconds,
        start_number,
        SegmentMediaOptions {
            audio_only: true,
            ..SegmentMediaOptions::default()
        },
        None,
        None,
    )
}

fn segment_media_file_inner(
    input_path: &Path,
    output_pattern: &Path,
    segment_seconds: u32,
    start_number: u32,
    options: SegmentMediaOptions,
    cancel: Option<&Arc<AtomicBool>>,
    mut progress: Option<&mut dyn FnMut(u32)>,
) -> Result<(), String> {
    let SegmentMediaOptions {
        audio_only,
        video_only,
        tolerate_invalid_analysis_packets,
        preferred_audio_stream_index,
    } = options;
    if cancel.is_some_and(|flag| flag.load(Ordering::Relaxed)) {
        return Err("cancelled".to_string());
    }
    let api = ffmpeg_api()?;
    let input_c = CString::new(input_path.to_string_lossy().as_bytes())
        .map_err(|_| "FFmpeg: invalid input path".to_string())?;
    let output_c = CString::new(output_pattern.to_string_lossy().as_bytes())
        .map_err(|_| "FFmpeg: invalid output pattern".to_string())?;
    let format_c =
        CString::new("segment").map_err(|_| "FFmpeg: invalid segment format".to_string())?;

    if segment_seconds == 0 {
        return Err("FFmpeg: segment time must be > 0".to_string());
    }

    let mut in_ctx: *mut AVFormatContext = ptr::null_mut();
    let mut out_ctx: *mut AVFormatContext = ptr::null_mut();

    let mut input_options: *mut AVDictionary = ptr::null_mut();
    dict_set_str(api, &mut input_options, "fflags", "+genpts+discardcorrupt")?;
    let open_ret = crate::ffmpeg_source::avformat_open_input_safe(
        api,
        &mut in_ctx,
        input_c.as_ptr(),
        ptr::null_mut(),
        &mut input_options,
    );
    crate::ffmpeg_source::av_dict_free_safe(api, &mut input_options);
    if open_ret < 0 || in_ctx.is_null() {
        return Err("FFmpeg: failed to open input".to_string());
    }
    if crate::ffmpeg_source::avformat_find_stream_info_safe(api, in_ctx, ptr::null_mut()) < 0 {
        crate::ffmpeg_source::avformat_close_input_safe(api, &mut in_ctx);
        return Err("FFmpeg: input stream info failed".to_string());
    }

    let total_duration_us = unsafe {
        if (*in_ctx).duration > 0 {
            Some((*in_ctx).duration)
        } else {
            None
        }
    };

    let alloc_ret = crate::ffmpeg_source::avformat_alloc_output_context2_safe(
        api,
        &mut out_ctx,
        ptr::null_mut(),
        format_c.as_ptr(),
        output_c.as_ptr(),
    );
    if alloc_ret < 0 || out_ctx.is_null() {
        crate::ffmpeg_source::avformat_close_input_safe(api, &mut in_ctx);
        return Err("FFmpeg: failed to allocate segment output".to_string());
    }

    let nb_streams = crate::ffmpeg_source::av_format_context_nb_streams_safe(in_ctx) as usize;
    let mut stream_map: Vec<*mut AVStream> = vec![ptr::null_mut(); nb_streams];
    let mut timestamp_states: Vec<SegmentTimestampState> =
        vec![SegmentTimestampState::default(); nb_streams];
    let mut mapped_streams = 0usize;
    for (i, stream_slot) in stream_map.iter_mut().enumerate() {
        let in_stream = unsafe { *(*in_ctx).streams.add(i) };
        if in_stream.is_null() {
            continue;
        }
        let codecpar = crate::ffmpeg_source::av_stream_codecpar_safe(in_stream);
        if codecpar.is_null() {
            continue;
        }
        let codec_type = crate::ffmpeg_source::av_codecpar_codec_type_safe(codecpar);
        let keep = if audio_only {
            codec_type == AVMediaType_AVMEDIA_TYPE_AUDIO && mapped_streams == 0
        } else if video_only {
            codec_type == AVMediaType_AVMEDIA_TYPE_VIDEO
        } else if codec_type == AVMediaType_AVMEDIA_TYPE_AUDIO {
            preferred_audio_stream_index.is_none_or(|preferred| preferred == i as i32)
        } else {
            codec_type == AVMediaType_AVMEDIA_TYPE_VIDEO
        };
        if !keep {
            continue;
        }
        let out_stream = crate::ffmpeg_source::avformat_new_stream_safe(api, out_ctx, ptr::null());
        if out_stream.is_null() {
            crate::ffmpeg_source::avformat_free_context_safe(api, out_ctx);
            crate::ffmpeg_source::avformat_close_input_safe(api, &mut in_ctx);
            return Err("FFmpeg: failed to create output stream".to_string());
        }
        let codecpar_copy_ret = crate::ffmpeg_source::avcodec_parameters_copy_safe(
            api,
            crate::ffmpeg_source::av_stream_codecpar_safe(out_stream),
            codecpar,
        );
        if codecpar_copy_ret < 0 {
            crate::ffmpeg_source::avformat_free_context_safe(api, out_ctx);
            crate::ffmpeg_source::avformat_close_input_safe(api, &mut in_ctx);
            return Err(format!(
                "FFmpeg: failed to copy codec parameters (code {})",
                codecpar_copy_ret
            ));
        }
        unsafe {
            (*out_stream).time_base = crate::ffmpeg_source::av_stream_time_base_safe(in_stream);
            (*crate::ffmpeg_source::av_stream_codecpar_safe(out_stream)).codec_tag = 0;
        }
        *stream_slot = out_stream;
        mapped_streams += 1;
    }

    if mapped_streams == 0 {
        crate::ffmpeg_source::avformat_free_context_safe(api, out_ctx);
        crate::ffmpeg_source::avformat_close_input_safe(api, &mut in_ctx);
        return Err(if audio_only {
            "FFmpeg: audio stream not found".to_string()
        } else {
            "FFmpeg: no supported media streams found".to_string()
        });
    }

    let mut dict: *mut AVDictionary = ptr::null_mut();
    dict_set_str(api, &mut dict, "segment_time", &segment_seconds.to_string())?;
    let start_number = start_number.max(1);
    dict_set_str(
        api,
        &mut dict,
        "segment_start_number",
        &start_number.to_string(),
    )?;
    dict_set_str(api, &mut dict, "reset_timestamps", "1")?;
    if let Some(fmt) = segment_format_from_path(output_pattern) {
        dict_set_str(api, &mut dict, "segment_format", fmt)?;
    }

    let header_ret = crate::ffmpeg_source::avformat_write_header_safe(api, out_ctx, &mut dict);
    crate::ffmpeg_source::av_dict_free_safe(api, &mut dict);
    if header_ret < 0 {
        let error_text = ffmpeg_error_text(api, header_ret);
        log_debug(&format!(
            "FFmpeg: failed to write segment header: {} ({}) output={} audio_only={} video_only={} preferred_audio_stream_index={:?}",
            error_text,
            header_ret,
            output_pattern.display(),
            audio_only,
            video_only,
            preferred_audio_stream_index
        ));
        crate::ffmpeg_source::avformat_free_context_safe(api, out_ctx);
        crate::ffmpeg_source::avformat_close_input_safe(api, &mut in_ctx);
        return Err(format!(
            "FFmpeg: failed to write segment header: {} ({})",
            error_text, header_ret
        ));
    }

    if let Some(cb) = progress.as_deref_mut() {
        cb(0);
    }
    let mut last_pct = 0u32;
    let mut repaired_timestamp_packets = 0u64;
    let mut skipped_invalid_analysis_packets = 0u32;
    let mut segment_write_error: Option<String> = None;
    let mut pkt = crate::ffmpeg_source::av_packet_alloc_safe(api);
    if pkt.is_null() {
        let trailer_ret = crate::ffmpeg_source::av_write_trailer_safe(api, out_ctx);
        if trailer_ret < 0 {
            log_debug(&format!(
                "FFmpeg: av_write_trailer failed during alloc cleanup: {}",
                trailer_ret
            ));
        }
        crate::ffmpeg_source::avformat_free_context_safe(api, out_ctx);
        crate::ffmpeg_source::avformat_close_input_safe(api, &mut in_ctx);
        return Err("FFmpeg: packet alloc failed".to_string());
    }

    loop {
        if cancel.is_some_and(|flag| flag.load(Ordering::Relaxed)) {
            segment_write_error = Some("cancelled".to_string());
            break;
        }
        let read_ret = crate::ffmpeg_source::av_read_frame_safe(api, in_ctx, pkt);
        if read_ret < 0 {
            break;
        }
        let input_index = crate::ffmpeg_source::av_packet_stream_index_safe(pkt);
        if input_index < 0
            || input_index as usize >= stream_map.len()
            || stream_map[input_index as usize].is_null()
        {
            crate::ffmpeg_source::av_packet_unref_safe(api, pkt);
            continue;
        }
        let in_stream = unsafe { *(*in_ctx).streams.add(input_index as usize) };
        let out_stream = stream_map[input_index as usize];
        if let (Some(total), Some(cb)) = (total_duration_us, progress.as_deref_mut()) {
            let pts = unsafe { (*pkt).pts };
            if total > 0 && pts != AV_NOPTS_VALUE_I64 {
                let tb = crate::ffmpeg_source::av_stream_time_base_safe(in_stream);
                if tb.den != 0 {
                    let pos_us = (pts as i128)
                        .saturating_mul(tb.num as i128)
                        .saturating_mul(1_000_000)
                        / (tb.den as i128);
                    if pos_us > 0 {
                        let pct = ((pos_us.min(total as i128) * 100) / (total as i128)) as u32;
                        let pct = pct.min(99);
                        if pct > last_pct {
                            last_pct = pct;
                            cb(pct);
                        }
                    }
                }
            }
        }
        crate::ffmpeg_source::av_packet_set_stream_index_safe(
            pkt,
            crate::ffmpeg_source::av_stream_index_safe(out_stream),
        );
        crate::ffmpeg_source::av_packet_rescale_ts_safe(
            api,
            pkt,
            crate::ffmpeg_source::av_stream_time_base_safe(in_stream),
            crate::ffmpeg_source::av_stream_time_base_safe(out_stream),
        );
        let repaired_packet =
            repair_segment_packet_timestamps(pkt, &mut timestamp_states[input_index as usize]);
        if repaired_packet {
            repaired_timestamp_packets = repaired_timestamp_packets.saturating_add(1);
        }
        let write_ret = crate::ffmpeg_source::av_interleaved_write_frame_safe(api, out_ctx, pkt);
        if write_ret < 0 {
            if tolerate_invalid_analysis_packets
                && write_ret == -libc::EINVAL
                && skipped_invalid_analysis_packets < 8
            {
                skipped_invalid_analysis_packets =
                    skipped_invalid_analysis_packets.saturating_add(1);
                log_debug(&format!(
                    "FFmpeg: skipping invalid analysis packet stream={} repaired={} error={} ({})",
                    input_index,
                    repaired_packet,
                    ffmpeg_error_text(api, write_ret),
                    write_ret
                ));
                crate::ffmpeg_source::av_packet_unref_safe(api, pkt);
                continue;
            }
            segment_write_error = Some(format!(
                "FFmpeg: segment write failed: {} ({})",
                ffmpeg_error_text(api, write_ret),
                write_ret
            ));
            crate::ffmpeg_source::av_packet_unref_safe(api, pkt);
            break;
        }
        crate::ffmpeg_source::av_packet_unref_safe(api, pkt);
    }

    let trailer_ret = crate::ffmpeg_source::av_write_trailer_safe(api, out_ctx);
    if trailer_ret < 0 {
        log_debug(&format!("FFmpeg: av_write_trailer failed: {}", trailer_ret));
    }
    crate::ffmpeg_source::av_packet_free_safe(api, &mut pkt);
    crate::ffmpeg_source::avformat_free_context_safe(api, out_ctx);
    crate::ffmpeg_source::avformat_close_input_safe(api, &mut in_ctx);
    if repaired_timestamp_packets > 0 {
        log_debug(&format!(
            "FFmpeg: repaired timestamps for {} packet(s) while segmenting {}",
            repaired_timestamp_packets,
            input_path.display()
        ));
    }
    if skipped_invalid_analysis_packets > 0 {
        log_debug(&format!(
            "FFmpeg: skipped {} invalid packet(s) while preparing analysis chunks for {}",
            skipped_invalid_analysis_packets,
            input_path.display()
        ));
    }
    if let Some(error) = segment_write_error {
        return Err(error);
    }
    if trailer_ret < 0 {
        return Err(format!(
            "FFmpeg: failed to finalize segmented output: {} ({})",
            ffmpeg_error_text(api, trailer_ret),
            trailer_ret
        ));
    }
    if let Some(cb) = progress.as_mut() {
        cb(100);
    }
    Ok(())
}

pub fn merge_audio_files_with_chapters_copy(
    input_files: &[PathBuf],
    output_path: &Path,
    chapter_titles: Option<&[String]>,
) -> Result<(), String> {
    merge_audio_files_copy_internal(input_files, output_path, chapter_titles, true)
}

pub(crate) fn concatenate_audio_files_copy(
    input_files: &[PathBuf],
    output_path: &Path,
) -> Result<(), String> {
    merge_audio_files_copy_internal(input_files, output_path, None, false)
}

fn merge_audio_files_copy_internal(
    input_files: &[PathBuf],
    output_path: &Path,
    chapter_titles: Option<&[String]>,
    include_chapters: bool,
) -> Result<(), String> {
    if input_files.len() < 2 {
        return Err("FFmpeg: at least 2 input files are required".to_string());
    }
    let api = ffmpeg_api()?;

    let mut durations_ms: Vec<u64> = Vec::with_capacity(input_files.len());
    for input in input_files {
        let source = FfmpegSource::try_new(input, 0, None, None)
            .map_err(|e| format!("FFmpeg: failed to probe {}: {}", input.display(), e))?;
        let duration = source
            .total_duration()
            .ok_or_else(|| format!("FFmpeg: missing duration for {}", input.display()))?;
        let ms = duration.as_millis().min(u64::MAX as u128) as u64;
        if ms == 0 {
            return Err(format!("FFmpeg: zero duration for {}", input.display()));
        }
        durations_ms.push(ms);
    }

    let out_c = CString::new(output_path.to_string_lossy().as_bytes())
        .map_err(|_| "FFmpeg: invalid output path".to_string())?;
    let mut out_ctx: *mut AVFormatContext = ptr::null_mut();
    let alloc_ret = crate::ffmpeg_source::avformat_alloc_output_context2_safe(
        api,
        &mut out_ctx,
        ptr::null_mut(),
        ptr::null(),
        out_c.as_ptr(),
    );
    if alloc_ret < 0 || out_ctx.is_null() {
        return Err("FFmpeg: failed to allocate output context".to_string());
    }

    let mut first_in_ctx: *mut AVFormatContext = ptr::null_mut();
    let first_c = CString::new(input_files[0].to_string_lossy().as_bytes())
        .map_err(|_| "FFmpeg: invalid first input path".to_string())?;
    let first_open = crate::ffmpeg_source::avformat_open_input_safe(
        api,
        &mut first_in_ctx,
        first_c.as_ptr(),
        ptr::null_mut(),
        ptr::null_mut(),
    );
    if first_open < 0 || first_in_ctx.is_null() {
        crate::ffmpeg_source::avformat_free_context_safe(api, out_ctx);
        return Err("FFmpeg: failed to open first input".to_string());
    }
    if crate::ffmpeg_source::avformat_find_stream_info_safe(api, first_in_ctx, ptr::null_mut()) < 0
    {
        unsafe {
            (api.avformat_close_input)(&mut first_in_ctx);
            (api.avformat_free_context)(out_ctx);
        }
        return Err("FFmpeg: failed to read first input stream info".to_string());
    }
    let first_audio_idx = crate::ffmpeg_source::av_find_best_stream_safe(
        api,
        first_in_ctx,
        AVMediaType_AVMEDIA_TYPE_AUDIO,
        -1,
        -1,
        ptr::null_mut(),
        0,
    );
    if first_audio_idx < 0 {
        unsafe {
            (api.avformat_close_input)(&mut first_in_ctx);
            (api.avformat_free_context)(out_ctx);
        }
        return Err("FFmpeg: no audio stream in first input".to_string());
    }
    let in_stream = unsafe { *(*first_in_ctx).streams.add(first_audio_idx as usize) };
    let out_stream = crate::ffmpeg_source::avformat_new_stream_safe(api, out_ctx, ptr::null());
    if out_stream.is_null() {
        unsafe {
            (api.avformat_close_input)(&mut first_in_ctx);
            (api.avformat_free_context)(out_ctx);
        }
        return Err("FFmpeg: failed to create output stream".to_string());
    }
    unsafe {
        if (api.avcodec_parameters_copy)((*out_stream).codecpar, (*in_stream).codecpar) < 0 {
            (api.avformat_close_input)(&mut first_in_ctx);
            (api.avformat_free_context)(out_ctx);
            return Err("FFmpeg: codec parameters copy failed".to_string());
        }
        (*out_stream).time_base = (*in_stream).time_base;
    }

    if include_chapters {
        let chapter_count = input_files.len();
        let chapters_ptr = unsafe {
            (api.av_mallocz)(
                (chapter_count * std::mem::size_of::<*mut AVChapter>()) as libc::size_t,
            ) as *mut *mut AVChapter
        };
        if chapters_ptr.is_null() {
            unsafe {
                (api.avformat_close_input)(&mut first_in_ctx);
                (api.avformat_free_context)(out_ctx);
            }
            return Err("FFmpeg: failed to allocate chapter array".to_string());
        }
        let mut cumulative_ms = 0u64;
        for (idx, dur_ms) in durations_ms.iter().enumerate() {
            let chapter =
                unsafe { (api.av_mallocz)(std::mem::size_of::<AVChapter>() as libc::size_t) }
                    as *mut AVChapter;
            if chapter.is_null() {
                unsafe {
                    (api.avformat_close_input)(&mut first_in_ctx);
                    (api.avformat_free_context)(out_ctx);
                }
                return Err("FFmpeg: failed to allocate chapter".to_string());
            }
            let start_ms = cumulative_ms as i64;
            let end_ms = cumulative_ms.saturating_add(*dur_ms) as i64;
            cumulative_ms = cumulative_ms.saturating_add(*dur_ms);
            let title = chapter_titles
                .and_then(|titles| titles.get(idx))
                .map(|s| s.trim())
                .filter(|s| !s.is_empty())
                .map(|s| s.to_string())
                .unwrap_or_else(|| format!("Part {:02}", idx + 1));
            let mut dict: *mut AVDictionary = ptr::null_mut();
            dict_set_str(api, &mut dict, "title", &title)?;
            unsafe {
                (*chapter).id = idx as i64;
                (*chapter).time_base = AVRational { num: 1, den: 1000 };
                (*chapter).start = start_ms;
                (*chapter).end = end_ms;
                (*chapter).metadata = dict;
                *chapters_ptr.add(idx) = chapter;
            }
        }
        unsafe {
            (*out_ctx).chapters = chapters_ptr;
            (*out_ctx).nb_chapters = chapter_count as u32;
        }
    }

    let mut io: *mut AVIOContext = ptr::null_mut();
    let open_io =
        crate::ffmpeg_source::avio_open_safe(api, &mut io, out_c.as_ptr(), AVIO_FLAG_WRITE);
    if open_io < 0 {
        unsafe {
            (api.avformat_close_input)(&mut first_in_ctx);
            (api.avformat_free_context)(out_ctx);
        }
        return Err("FFmpeg: failed to open output IO".to_string());
    }
    unsafe {
        (*out_ctx).pb = io;
    }

    let header_ret =
        crate::ffmpeg_source::avformat_write_header_safe(api, out_ctx, ptr::null_mut());
    if header_ret < 0 {
        unsafe {
            (api.avio_closep)(&mut io);
            (api.avformat_close_input)(&mut first_in_ctx);
            (api.avformat_free_context)(out_ctx);
        }
        return Err("FFmpeg: failed to write output header".to_string());
    }
    unsafe {
        (api.avformat_close_input)(&mut first_in_ctx);
    }
    let out_tb = unsafe { (*out_stream).time_base };

    let mut global_last_dts = 0i64;
    let mut has_global_last_dts = false;
    let mut part_offsets_ts: Vec<i64> = Vec::with_capacity(durations_ms.len());
    let mut cumulative_ms: i64 = 0;
    for dur_ms in &durations_ms {
        part_offsets_ts.push(rescale_q(
            cumulative_ms,
            AVRational { num: 1, den: 1000 },
            out_tb,
        ));
        cumulative_ms = cumulative_ms.saturating_add(*dur_ms as i64);
    }
    for (idx, input) in input_files.iter().enumerate() {
        let mut in_ctx: *mut AVFormatContext = ptr::null_mut();
        let input_c = CString::new(input.to_string_lossy().as_bytes())
            .map_err(|_| "FFmpeg: invalid input path".to_string())?;
        let open_ret = crate::ffmpeg_source::avformat_open_input_safe(
            api,
            &mut in_ctx,
            input_c.as_ptr(),
            ptr::null_mut(),
            ptr::null_mut(),
        );
        if open_ret < 0 || in_ctx.is_null() {
            unsafe {
                (api.avio_closep)(&mut io);
                (api.avformat_free_context)(out_ctx);
            };
            return Err(format!("FFmpeg: failed to open {}", input.display()));
        }
        if crate::ffmpeg_source::avformat_find_stream_info_safe(api, in_ctx, ptr::null_mut()) < 0 {
            unsafe {
                (api.avformat_close_input)(&mut in_ctx);
                (api.avio_closep)(&mut io);
                (api.avformat_free_context)(out_ctx);
            }
            return Err(format!(
                "FFmpeg: failed stream info for {}",
                input.display()
            ));
        }
        let audio_idx = crate::ffmpeg_source::av_find_best_stream_safe(
            api,
            in_ctx,
            AVMediaType_AVMEDIA_TYPE_AUDIO,
            -1,
            -1,
            ptr::null_mut(),
            0,
        );
        if audio_idx < 0 {
            unsafe {
                (api.avformat_close_input)(&mut in_ctx);
                (api.avio_closep)(&mut io);
                (api.avformat_free_context)(out_ctx);
            }
            return Err(format!("FFmpeg: no audio stream in {}", input.display()));
        }
        let in_stream = unsafe { *(*in_ctx).streams.add(audio_idx as usize) };
        let in_tb = unsafe { (*in_stream).time_base };
        let fallback_frame_duration = unsafe {
            let codecpar = (*in_stream).codecpar;
            if codecpar.is_null() {
                1024i64
            } else {
                let frame_size = (*codecpar).frame_size as i64;
                let sample_rate = (*codecpar).sample_rate as i64;
                if frame_size > 0 && sample_rate > 0 {
                    let dur = rescale_q(
                        frame_size,
                        AVRational {
                            num: 1,
                            den: sample_rate as i32,
                        },
                        out_tb,
                    );
                    dur.max(1)
                } else {
                    1024i64
                }
            }
        };
        let mut pkt = crate::ffmpeg_source::av_packet_alloc_safe(api);
        if pkt.is_null() {
            unsafe {
                (api.avformat_close_input)(&mut in_ctx);
                (api.avio_closep)(&mut io);
                (api.avformat_free_context)(out_ctx);
            }
            return Err("FFmpeg: packet allocation failed".to_string());
        }
        let part_offset_ts = part_offsets_ts.get(idx).copied().unwrap_or(0);
        let mut part_cursor_ts = if has_global_last_dts {
            global_last_dts.max(part_offset_ts)
        } else {
            part_offset_ts
        };
        let mut part_max_end_ts = part_cursor_ts;
        let mut packets_written_in_part: usize = 0;
        loop {
            let read_ret = crate::ffmpeg_source::av_read_frame_safe(api, in_ctx, pkt);
            if read_ret < 0 {
                break;
            }
            if crate::ffmpeg_source::av_packet_stream_index_safe(pkt) != audio_idx {
                crate::ffmpeg_source::av_packet_unref_safe(api, pkt);
                continue;
            }
            unsafe {
                (*pkt).stream_index = (*out_stream).index;
                crate::ffmpeg_source::av_packet_rescale_ts_safe(api, pkt, in_tb, out_tb);
                let out_duration = if (*pkt).duration > 0 {
                    (*pkt).duration
                } else {
                    fallback_frame_duration
                };
                (*pkt).duration = out_duration;
                // Deterministic packet timeline for chapter merge:
                // avoids invalid NOPTS/DTS from segmented AAC parts.
                let mut normalized_dts = part_cursor_ts;
                if has_global_last_dts && normalized_dts <= global_last_dts {
                    normalized_dts = global_last_dts.saturating_add(out_duration);
                }
                let normalized_pts = normalized_dts;
                (*pkt).dts = normalized_dts;
                (*pkt).pts = normalized_pts;
                (*pkt).pos = -1;
                let wret = crate::ffmpeg_source::av_interleaved_write_frame_safe(api, out_ctx, pkt);
                if wret >= 0 {
                    packets_written_in_part = packets_written_in_part.saturating_add(1);
                    global_last_dts = normalized_dts;
                    has_global_last_dts = true;
                    let end_ts = normalized_dts.saturating_add(out_duration);
                    part_cursor_ts = end_ts;
                    if end_ts > part_max_end_ts {
                        part_max_end_ts = end_ts;
                    }
                }
                crate::ffmpeg_source::av_packet_unref_safe(api, pkt);
                if wret < 0 {
                    let err_text = ffmpeg_error_text(api, wret);
                    log_debug(&format!(
                        "FFmpeg merge debug: part={} in_tb={}/{} out_tb={}/{} out_dur={} part_offset={} norm_pts={} norm_dts={} last_dts={}",
                        idx + 1,
                        in_tb.num,
                        in_tb.den,
                        out_tb.num,
                        out_tb.den,
                        out_duration,
                        part_offset_ts,
                        normalized_pts,
                        normalized_dts,
                        global_last_dts
                    ));
                    crate::ffmpeg_source::av_packet_free_safe(api, &mut pkt);
                    (api.avformat_close_input)(&mut in_ctx);
                    (api.avio_closep)(&mut io);
                    (api.avformat_free_context)(out_ctx);
                    return Err(format!(
                        "FFmpeg: write frame failed on part {} ({})",
                        idx + 1,
                        err_text
                    ));
                }
            }
        }
        log_debug(&format!(
            "FFmpeg merge: part {} packets_written={} part_offset={} max_end={}",
            idx + 1,
            packets_written_in_part,
            part_offset_ts,
            part_max_end_ts
        ));
        if packets_written_in_part == 0 {
            unsafe {
                crate::ffmpeg_source::av_packet_free_safe(api, &mut pkt);
                (api.avformat_close_input)(&mut in_ctx);
                (api.avio_closep)(&mut io);
                (api.avformat_free_context)(out_ctx);
            }
            return Err(format!(
                "FFmpeg: no audio packets written while merging part {}",
                idx + 1
            ));
        }
        unsafe {
            crate::ffmpeg_source::av_packet_free_safe(api, &mut pkt);
            (api.avformat_close_input)(&mut in_ctx);
        }
    }

    unsafe {
        let trailer_ret = crate::ffmpeg_source::av_write_trailer_safe(api, out_ctx);
        if trailer_ret < 0 {
            log_debug(&format!(
                "FFmpeg: av_write_trailer failed in merge_audio_files_with_chapters_copy: {}",
                trailer_ret
            ));
        }
        (api.avio_closep)(&mut io);
        (api.avformat_free_context)(out_ctx);
    }
    Ok(())
}

pub fn cleanup_tts_artifacts() -> Result<(), String> {
    let mut errors = Vec::new();
    let outputs = MIX_OUTPUTS.get_or_init(|| Mutex::new(Vec::new()));
    if let Ok(mut list) = outputs.lock() {
        for path in list.drain(..) {
            if path.exists()
                && let Err(e) = std::fs::remove_file(&path)
            {
                errors.push(format!(
                    "Subtitle: failed to delete mixed output {}: {}",
                    path.display(),
                    e
                ));
            }
        }
    }

    let cache_dir = crate::settings::settings_dir().join("subtitle_cache");
    if cache_dir.exists()
        && let Err(e) = std::fs::remove_dir_all(&cache_dir)
    {
        errors.push(format!(
            "Subtitle: failed to delete cache {}: {}",
            cache_dir.display(),
            e
        ));
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors.join(" | "))
    }
}

fn pick_encoder_sample_fmt(codec: *const AVCodec) -> AVSampleFormat {
    unsafe {
        if codec.is_null() {
            return AVSampleFormat_AV_SAMPLE_FMT_FLTP;
        }
        let fmts = (*codec).sample_fmts;
        if fmts.is_null() {
            return AVSampleFormat_AV_SAMPLE_FMT_FLTP;
        }
        let mut idx = 0;
        loop {
            let fmt = *fmts.add(idx);
            if fmt == AVSampleFormat_AV_SAMPLE_FMT_NONE {
                break;
            }
            if fmt == AVSampleFormat_AV_SAMPLE_FMT_FLTP {
                return fmt;
            }
            if fmt == AVSampleFormat_AV_SAMPLE_FMT_FLT {
                return fmt;
            }
            idx += 1;
        }
    }
    AVSampleFormat_AV_SAMPLE_FMT_FLTP
}

fn pick_encoder_sample_fmt_with_preference(
    codec: *const AVCodec,
    preferred: Option<AVSampleFormat>,
) -> AVSampleFormat {
    unsafe {
        if codec.is_null() {
            return preferred.unwrap_or(AVSampleFormat_AV_SAMPLE_FMT_FLTP);
        }
        let fmts = (*codec).sample_fmts;
        if fmts.is_null() {
            return preferred.unwrap_or(AVSampleFormat_AV_SAMPLE_FMT_FLTP);
        }
        let mut idx = 0;
        let mut first = None;
        loop {
            let fmt = *fmts.add(idx);
            if fmt == AVSampleFormat_AV_SAMPLE_FMT_NONE {
                break;
            }
            if first.is_none() {
                first = Some(fmt);
            }
            if let Some(pref) = preferred
                && fmt == pref
            {
                return fmt;
            }
            if fmt == AVSampleFormat_AV_SAMPLE_FMT_FLTP {
                return fmt;
            }
            if fmt == AVSampleFormat_AV_SAMPLE_FMT_FLT {
                return fmt;
            }
            if fmt == AVSampleFormat_AV_SAMPLE_FMT_S16 {
                return fmt;
            }
            idx += 1;
        }
        first.unwrap_or(AVSampleFormat_AV_SAMPLE_FMT_FLTP)
    }
}

fn rescale_q(value: i64, src: AVRational, dst: AVRational) -> i64 {
    if src.den == 0 || dst.num == 0 {
        return 0;
    }
    let num = value as i128 * src.num as i128 * dst.den as i128;
    let den = src.den as i128 * dst.num as i128;
    if den == 0 { 0 } else { (num / den) as i64 }
}

fn read_next_packet_for_stream(
    api: &FfmpegApi,
    ctx: *mut AVFormatContext,
    pkt: *mut AVPacket,
    stream_idx: i32,
) -> bool {
    loop {
        let ret = crate::ffmpeg_source::av_read_frame_safe(api, ctx, pkt);
        if ret < 0 {
            return false;
        }
        let idx = crate::ffmpeg_source::av_packet_stream_index_safe(pkt);
        if idx == stream_idx {
            return true;
        }
        crate::ffmpeg_source::av_packet_unref_safe(api, pkt);
    }
}

fn packet_reference_timestamp(packet: *mut AVPacket) -> Option<i64> {
    if packet.is_null() {
        return None;
    }
    unsafe {
        match (
            ((*packet).dts != AV_NOPTS_VALUE_I64).then_some((*packet).dts),
            ((*packet).pts != AV_NOPTS_VALUE_I64).then_some((*packet).pts),
        ) {
            (Some(dts), Some(pts)) => Some(dts.min(pts)),
            (Some(dts), None) => Some(dts),
            (None, Some(pts)) => Some(pts),
            (None, None) => None,
        }
    }
}

fn relative_packet_timestamp(packet: *mut AVPacket, origin: Option<i64>) -> i64 {
    let timestamp = packet_reference_timestamp(packet).unwrap_or(0);
    origin
        .map(|start| timestamp.saturating_sub(start))
        .unwrap_or(timestamp)
}

fn normalize_live_packet_timestamps(packet: *mut AVPacket, origin: Option<i64>) {
    let Some(origin) = origin else {
        return;
    };
    if packet.is_null() {
        return;
    }
    unsafe {
        if (*packet).pts != AV_NOPTS_VALUE_I64 {
            (*packet).pts = (*packet).pts.saturating_sub(origin);
        }
        if (*packet).dts != AV_NOPTS_VALUE_I64 {
            (*packet).dts = (*packet).dts.saturating_sub(origin);
        }
    }
}

fn encode_mixed_audio_to_m4a(
    api: &FfmpegApi,
    input_path: &Path,
    subtitle_path: &Path,
    settings: &AppSettings,
    options: &MixExportOptions,
    out_audio_path: &Path,
) -> Result<(), String> {
    let cues = ensure_tts_cache(input_path, subtitle_path, settings)?;

    let mut source = FfmpegSource::try_new(input_path, 0, None, None)?;
    let sample_rate = source.sample_rate();
    let channels = source.channels();

    let offset_secs = settings.subtitle_offset_ms as f64 / 1000.0;
    let mut pending = collect_tts_audio(
        &cues,
        input_path,
        settings,
        sample_rate,
        channels,
        offset_secs,
    )?;
    let duck_intervals = if options.ducking {
        build_duck_intervals(&pending, sample_rate, channels)
    } else {
        Vec::new()
    };
    let ducking_enabled = options.ducking && !duck_intervals.is_empty();
    let mut duck_idx = 0usize;
    let mut duck_end = 0u64;
    let mut duck_active = false;
    let mut duck_next_change = u64::MAX;
    if let Some((start, end)) = duck_intervals.first() {
        duck_end = *end;
        duck_next_change = *start;
    }
    let mut active: Vec<ActiveCue> = Vec::new();

    let out_path_c = CString::new(out_audio_path.to_string_lossy().as_bytes())
        .map_err(|_| "FFmpeg: invalid output path".to_string())?;
    let mut out_ctx: *mut AVFormatContext = ptr::null_mut();
    let alloc_ret = crate::ffmpeg_source::avformat_alloc_output_context2_safe(
        api,
        &mut out_ctx,
        ptr::null_mut(),
        ptr::null(),
        out_path_c.as_ptr(),
    );
    if alloc_ret < 0 || out_ctx.is_null() {
        return Err("FFmpeg: failed to allocate output context".to_string());
    }

    let codec = crate::ffmpeg_source::avcodec_find_encoder_safe(api, AVCodecID_AV_CODEC_ID_AAC);
    if codec.is_null() {
        crate::ffmpeg_source::avformat_free_context_safe(api, out_ctx);
        return Err("FFmpeg: AAC encoder not found".to_string());
    }
    let stream = crate::ffmpeg_source::avformat_new_stream_safe(api, out_ctx, codec);
    if stream.is_null() {
        crate::ffmpeg_source::avformat_free_context_safe(api, out_ctx);
        return Err("FFmpeg: failed to create output stream".to_string());
    }
    let mut codec_ctx = crate::ffmpeg_source::avcodec_alloc_context3_safe(api, codec);
    if codec_ctx.is_null() {
        crate::ffmpeg_source::avformat_free_context_safe(api, out_ctx);
        return Err("FFmpeg: failed to allocate encoder context".to_string());
    }
    let enc_fmt = pick_encoder_sample_fmt(codec);
    unsafe {
        (*codec_ctx).sample_rate = sample_rate as i32;
        (*codec_ctx).bit_rate = DEFAULT_AAC_BITRATE;
        (*codec_ctx).sample_fmt = enc_fmt;
        (*codec_ctx).time_base = AVRational {
            num: 1,
            den: sample_rate as i32,
        };
        let mut out_layout: AVChannelLayout = std::mem::zeroed();
        (api.av_channel_layout_default)(&mut out_layout, channels as i32);
        (*codec_ctx).ch_layout = out_layout;
    }
    let open_ret = crate::ffmpeg_source::avcodec_open2_safe(api, codec_ctx, codec, ptr::null_mut());
    if open_ret < 0 {
        unsafe {
            (api.avcodec_free_context)(&mut codec_ctx);
            (api.avformat_free_context)(out_ctx);
        }
        return Err("FFmpeg: failed to open encoder".to_string());
    }
    unsafe {
        (*stream).time_base = (*codec_ctx).time_base;
        let par_ret = (api.avcodec_parameters_from_context)((*stream).codecpar, codec_ctx);
        if par_ret < 0 {
            (api.avcodec_free_context)(&mut codec_ctx);
            (api.avformat_free_context)(out_ctx);
            return Err("FFmpeg: failed to copy encoder parameters".to_string());
        }
    }

    let mut io: *mut AVIOContext = ptr::null_mut();
    let open_io =
        crate::ffmpeg_source::avio_open_safe(api, &mut io, out_path_c.as_ptr(), AVIO_FLAG_WRITE);
    if open_io < 0 {
        unsafe {
            (api.avcodec_free_context)(&mut codec_ctx);
            (api.avformat_free_context)(out_ctx);
        }
        return Err("FFmpeg: failed to open output file".to_string());
    }
    unsafe {
        (*out_ctx).pb = io;
    }
    let header_ret =
        crate::ffmpeg_source::avformat_write_header_safe(api, out_ctx, ptr::null_mut());
    if header_ret < 0 {
        unsafe {
            (api.avio_closep)(&mut io);
            (api.avcodec_free_context)(&mut codec_ctx);
            (api.avformat_free_context)(out_ctx);
        }
        return Err("FFmpeg: failed to write output header".to_string());
    }

    let frame_size =
        crate::ffmpeg_source::av_codec_context_frame_size_safe(codec_ctx).max(1024) as usize;
    let mut frame = crate::ffmpeg_source::av_frame_alloc_safe(api);
    if frame.is_null() {
        unsafe {
            (api.avio_closep)(&mut io);
            (api.avcodec_free_context)(&mut codec_ctx);
            (api.avformat_free_context)(out_ctx);
        }
        return Err("FFmpeg: failed to allocate frame".to_string());
    }
    unsafe {
        (*frame).nb_samples = frame_size as i32;
        (*frame).format = enc_fmt;
        (*frame).sample_rate = sample_rate as i32;
        let mut frame_layout: AVChannelLayout = std::mem::zeroed();
        (api.av_channel_layout_default)(&mut frame_layout, channels as i32);
        (*frame).ch_layout = frame_layout;
        if (api.av_frame_get_buffer)(frame, 0) < 0 {
            (api.av_frame_free)(&mut frame);
            (api.avio_closep)(&mut io);
            (api.avcodec_free_context)(&mut codec_ctx);
            (api.avformat_free_context)(out_ctx);
            return Err("FFmpeg: failed to allocate frame buffer".to_string());
        }
    }

    let mut in_layout: AVChannelLayout = crate::zeroed_safe();
    let mut out_layout: AVChannelLayout = crate::zeroed_safe();
    unsafe {
        (api.av_channel_layout_default)(&mut in_layout, channels as i32);
        (api.av_channel_layout_default)(&mut out_layout, channels as i32);
    }
    let mut swr_ctx: *mut SwrContext = ptr::null_mut();
    let swr_ret = unsafe {
        (api.swr_alloc_set_opts2)(
            &mut swr_ctx,
            &out_layout,
            enc_fmt,
            sample_rate as i32,
            &in_layout,
            AVSampleFormat_AV_SAMPLE_FMT_FLT,
            sample_rate as i32,
            0,
            ptr::null_mut(),
        )
    };
    if swr_ret < 0 || swr_ctx.is_null() {
        unsafe {
            (api.av_frame_free)(&mut frame);
            (api.avio_closep)(&mut io);
            (api.avcodec_free_context)(&mut codec_ctx);
            (api.avformat_free_context)(out_ctx);
        }
        return Err("FFmpeg: failed to init resampler".to_string());
    }
    if crate::ffmpeg_source::swr_init_safe(api, swr_ctx) < 0 {
        unsafe {
            (api.swr_free)(&mut swr_ctx);
            (api.av_frame_free)(&mut frame);
            (api.avio_closep)(&mut io);
            (api.avcodec_free_context)(&mut codec_ctx);
            (api.avformat_free_context)(out_ctx);
        }
        return Err("FFmpeg: failed to init resampler".to_string());
    }

    let mut mixed_buf: VecDeque<f32> = VecDeque::new();
    let mut sample_index: u64 = 0;
    let mut next_pts: i64 = 0;
    let channel_count = channels as usize;
    let base_film_gain = if options.ducking {
        FILM_BASE_VOLUME
    } else {
        1.0
    };
    let ducked_film_gain = FILM_BASE_VOLUME * DUCK_MULTIPLIER;
    let desc_gain = if options.ducking { DESC_VOLUME } else { 1.0 };

    let read_next_sample = |pending: &mut VecDeque<CueAudio>,
                            active: &mut Vec<ActiveCue>,
                            current_sample: u64|
     -> f32 {
        while let Some(front) = pending.front() {
            if current_sample >= front.start_sample {
                if let Some(cue) = pending.pop_front() {
                    active.push(ActiveCue {
                        samples: cue.samples,
                        read_offset: 0,
                    });
                }
            } else {
                break;
            }
        }
        let mut tts_sample = 0.0f32;
        active.retain_mut(|cue| {
            if cue.read_offset < cue.samples.len() {
                tts_sample += cue.samples[cue.read_offset];
                cue.read_offset += 1;
                true
            } else {
                false
            }
        });
        tts_sample
    };

    loop {
        while mixed_buf.len() < frame_size * channel_count {
            match source.next() {
                Some(orig) => {
                    let tts = read_next_sample(&mut pending, &mut active, sample_index);
                    let mut film_gain = base_film_gain;
                    if ducking_enabled && sample_index >= duck_next_change {
                        if duck_active {
                            duck_active = false;
                            duck_idx = duck_idx.saturating_add(1);
                            if let Some((start, end)) = duck_intervals.get(duck_idx) {
                                duck_end = *end;
                                duck_next_change = *start;
                            } else {
                                duck_next_change = u64::MAX;
                            }
                        } else {
                            duck_active = true;
                            duck_next_change = duck_end;
                        }
                    }
                    if duck_active {
                        film_gain = ducked_film_gain;
                    }
                    let mixed = orig * film_gain + tts * desc_gain;
                    mixed_buf.push_back(mixed.clamp(-1.0, 1.0));
                    sample_index = sample_index.saturating_add(1);
                }
                None => break,
            }
        }
        if mixed_buf.is_empty() {
            break;
        }
        let mut input_frame = Vec::with_capacity(frame_size * channel_count);
        while input_frame.len() < frame_size * channel_count {
            if let Some(v) = mixed_buf.pop_front() {
                input_frame.push(v);
            } else {
                input_frame.push(0.0);
            }
        }
        unsafe {
            if (api.av_frame_make_writable)(frame) < 0 {
                (api.swr_free)(&mut swr_ctx);
                (api.av_frame_free)(&mut frame);
                (api.avio_closep)(&mut io);
                (api.avcodec_free_context)(&mut codec_ctx);
                (api.avformat_free_context)(out_ctx);
                (api.av_channel_layout_uninit)(&mut in_layout);
                (api.av_channel_layout_uninit)(&mut out_layout);
                return Err("FFmpeg: frame not writable".to_string());
            }
        }
        let mut in_ptr = input_frame.as_ptr() as *const u8;
        let in_ptrs = &mut in_ptr as *mut *const u8;
        let out_count = crate::ffmpeg_source::av_frame_nb_samples_safe(frame);
        let conv = crate::ffmpeg_source::swr_convert_safe(
            api,
            swr_ctx,
            crate::ffmpeg_source::av_frame_data_mut_ptr_safe(frame),
            out_count,
            in_ptrs,
            out_count,
        );
        if conv < 0 {
            return Err("FFmpeg: resample failed".to_string());
        }
        unsafe {
            (*frame).pts = next_pts;
        }
        next_pts += out_count as i64;
        let send_ret = crate::ffmpeg_source::avcodec_send_frame_safe(api, codec_ctx, frame);
        if send_ret < 0 {
            return Err("FFmpeg: send_frame failed".to_string());
        }
        loop {
            let mut pkt = crate::ffmpeg_source::av_packet_alloc_safe(api);
            if pkt.is_null() {
                return Err("FFmpeg: packet alloc failed".to_string());
            }
            let recv = crate::ffmpeg_source::avcodec_receive_packet_safe(api, codec_ctx, pkt);
            if recv == 0 {
                unsafe {
                    (*pkt).stream_index = (*stream).index;
                    crate::ffmpeg_source::av_packet_rescale_ts_safe(
                        api,
                        pkt,
                        (*codec_ctx).time_base,
                        (*stream).time_base,
                    );
                    let wret =
                        crate::ffmpeg_source::av_interleaved_write_frame_safe(api, out_ctx, pkt);
                    crate::ffmpeg_source::av_packet_unref_safe(api, pkt);
                    crate::ffmpeg_source::av_packet_free_safe(api, &mut pkt);
                    if wret < 0 {
                        return Err("FFmpeg: write audio packet failed".to_string());
                    }
                }
            } else {
                crate::ffmpeg_source::av_packet_free_safe(api, &mut pkt);
                break;
            }
        }
    }

    unsafe {
        (api.avcodec_send_frame)(codec_ctx, ptr::null());
        loop {
            let mut pkt = (api.av_packet_alloc)();
            if pkt.is_null() {
                break;
            }
            let recv = (api.avcodec_receive_packet)(codec_ctx, pkt);
            if recv == 0 {
                (*pkt).stream_index = (*stream).index;
                crate::ffmpeg_source::av_packet_rescale_ts_safe(
                    api,
                    pkt,
                    (*codec_ctx).time_base,
                    (*stream).time_base,
                );
                crate::ffmpeg_source::av_interleaved_write_frame_safe(api, out_ctx, pkt);
                crate::ffmpeg_source::av_packet_unref_safe(api, pkt);
                crate::ffmpeg_source::av_packet_free_safe(api, &mut pkt);
            } else {
                crate::ffmpeg_source::av_packet_free_safe(api, &mut pkt);
                break;
            }
        }
        let trailer_ret = crate::ffmpeg_source::av_write_trailer_safe(api, out_ctx);
        if trailer_ret < 0 {
            log_debug(&format!("FFmpeg: av_write_trailer failed: {}", trailer_ret));
        }
        (api.swr_free)(&mut swr_ctx);
        (api.av_frame_free)(&mut frame);
        (api.avio_closep)(&mut io);
        (api.avcodec_free_context)(&mut codec_ctx);
        (api.avformat_free_context)(out_ctx);
        (api.av_channel_layout_uninit)(&mut in_layout);
        (api.av_channel_layout_uninit)(&mut out_layout);
    }

    Ok(())
}

#[derive(Clone)]
struct PreparedAudioDescriptionCue {
    start_sample: u64,
    samples: Arc<[f32]>,
    extended_pause: bool,
}

fn merge_audio_description_intervals(
    mut intervals: Vec<(u64, u64)>,
    merge_gap_frames: u64,
) -> Vec<(u64, u64)> {
    intervals.retain(|(start, end)| end > start);
    intervals.sort_unstable_by_key(|(start, _)| *start);
    let mut merged: Vec<(u64, u64)> = Vec::new();
    for (start, end) in intervals {
        if let Some(last) = merged.last_mut()
            && start <= last.1.saturating_add(merge_gap_frames)
        {
            last.1 = last.1.max(end);
        } else {
            merged.push((start, end));
        }
    }
    merged
}

fn audio_description_smooth_fade(position: f32) -> f32 {
    let position = position.clamp(0.0, 1.0);
    0.5 - 0.5 * (std::f32::consts::PI * position).cos()
}

fn audio_description_duck_gain(
    frame: u64,
    intervals: &[(u64, u64)],
    cursor: &mut usize,
    attack_frames: u64,
    preduck_frames: u64,
    release_frames: u64,
    duck_gain: f32,
) -> f32 {
    while let Some((_, end)) = intervals.get(*cursor) {
        if frame > end.saturating_add(release_frames) {
            *cursor = cursor.saturating_add(1);
        } else {
            break;
        }
    }
    let Some((start, end)) = intervals.get(*cursor).copied() else {
        return 1.0;
    };

    let full_duck_start = start.saturating_sub(preduck_frames);
    if attack_frames > 0 && frame < full_duck_start {
        let attack_start = full_duck_start.saturating_sub(attack_frames);
        if frame >= attack_start {
            let position = (frame - attack_start) as f32 / attack_frames as f32;
            let eased = audio_description_smooth_fade(position);
            return 1.0 + (duck_gain - 1.0) * eased;
        }
    }
    if frame >= full_duck_start && frame <= end {
        return duck_gain;
    }
    if release_frames > 0 && frame > end && frame <= end.saturating_add(release_frames) {
        let position = (frame - end) as f32 / release_frames as f32;
        let eased = audio_description_smooth_fade(position);
        return duck_gain + (1.0 - duck_gain) * eased;
    }
    1.0
}

fn mix_audio_description_sample(
    original_sample: f32,
    film_gain: f32,
    narration_sample: f32,
) -> f32 {
    (original_sample * film_gain + narration_sample).clamp(-1.0, 1.0)
}

fn audio_description_wav_padding_samples(samples_written: u64, channels: u16) -> u16 {
    let channels = channels.max(1) as u64;
    let remainder = samples_written % channels;
    if remainder == 0 {
        0
    } else {
        (channels - remainder) as u16
    }
}

fn audio_description_temp_wav_path(output_path: &Path) -> PathBuf {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let parent = output_path.parent().unwrap_or_else(|| Path::new("."));
    parent.join(format!(
        ".sonarpad_audio_description_{}_{}.wav",
        std::process::id(),
        stamp
    ))
}

/// Mix and export an audio-described MP3 without launching an external FFmpeg
/// process. Source decoding and final libmp3lame encoding both use Sonarpad's
/// dynamically loaded FFmpeg libraries. Pyannote is not involved here: its
/// protected intervals have already been used by the exact TTS scheduler.
pub fn export_audio_description_mp3(
    input_path: &Path,
    output_path: &Path,
    preferred_audio_stream_index: Option<i32>,
    cues: &[AudioDescriptionMixCue],
    options: &AudioDescriptionExportOptions,
    mut progress: Option<&mut dyn FnMut(u32)>,
) -> Result<(), String> {
    if cues.is_empty() {
        return Err("Audio description: no synthesized cues to export".to_string());
    }
    if options.cancel.load(Ordering::Relaxed) {
        return Err("cancelled".to_string());
    }

    let mut source = FfmpegSource::try_new(input_path, 0, None, preferred_audio_stream_index)?;
    let sample_rate = source.sample_rate().max(1);
    let channels = source.channels().max(1);
    let channel_count = channels as usize;
    let total_source_samples = source.total_duration().and_then(|duration| {
        let value = duration.as_secs_f64() * sample_rate as f64 * channels as f64;
        value.is_finite().then_some(value.max(1.0) as u64)
    });

    let mut prepared = Vec::with_capacity(cues.len());
    for cue in cues {
        if cue.samples.is_empty() || !cue.start_sec.is_finite() {
            continue;
        }
        let converted = if cue.sample_rate == sample_rate && cue.channels == channels {
            cue.samples.to_vec()
        } else {
            resample_pcm(
                cue.samples.as_ref(),
                cue.sample_rate.max(1),
                cue.channels.max(1),
                sample_rate,
                channels,
            )
        };
        if converted.is_empty() {
            continue;
        }
        let start_frame = (cue.start_sec.max(0.0) * sample_rate as f64).round() as u64;
        prepared.push(PreparedAudioDescriptionCue {
            start_sample: start_frame.saturating_mul(channels as u64),
            samples: Arc::from(converted),
            extended_pause: cue.extended_pause,
        });
    }
    prepared.sort_by_key(|cue| cue.start_sample);
    if prepared.is_empty() {
        return Err("Audio description: synthesized cues are empty or invalid".to_string());
    }

    let normal_cues: Vec<_> = prepared
        .iter()
        .filter(|cue| !cue.extended_pause)
        .cloned()
        .collect();
    let pause_cues: Vec<_> = prepared
        .iter()
        .filter(|cue| cue.extended_pause)
        .cloned()
        .collect();
    let duck_gain = 10_f32.powf(options.ducking_db.min(0.0) / 20.0);
    let attack_frames = (sample_rate as u64)
        .saturating_mul(options.fade_ms as u64)
        .saturating_div(1000);
    // Professional audio-description envelope.  `fade_ms` remains the stored
    // attack value for project compatibility; pre-duck and release scale with
    // it so older projects keep their relative timing while new projects use
    // 280 ms attack, 180 ms pre-duck and 600 ms release.
    let preduck_frames = attack_frames.saturating_mul(9).saturating_div(14);
    let release_frames = attack_frames.saturating_mul(15).saturating_div(7);
    let duck_intervals = merge_audio_description_intervals(
        normal_cues
            .iter()
            .map(|cue| {
                let start_frame = cue.start_sample / channels as u64;
                let cue_frames = (cue.samples.len() / channel_count).max(1) as u64;
                (start_frame, start_frame.saturating_add(cue_frames))
            })
            .collect(),
        attack_frames
            .saturating_add(preduck_frames)
            .saturating_add(release_frames),
    );

    let temp_wav = audio_description_temp_wav_path(output_path);
    let wav_spec = hound::WavSpec {
        channels,
        sample_rate,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    log_debug(&format!(
        "Audio description export: staging WAV sample_rate={} channels={} final_mp3_channels={}",
        sample_rate,
        channels,
        channels.min(2)
    ));
    let mix_result = (|| -> Result<(), String> {
        let mut writer = hound::WavWriter::create(&temp_wav, wav_spec)
            .map_err(|error| format!("Audio description: create staging WAV failed: {error}"))?;
        let mut pending: VecDeque<CueAudio> = normal_cues
            .into_iter()
            .map(|cue| CueAudio {
                start_sample: cue.start_sample,
                samples: cue.samples,
            })
            .collect();
        let mut active: Vec<ActiveCue> = Vec::new();
        let mut pending_pauses: VecDeque<PreparedAudioDescriptionCue> = pause_cues.into();
        let mut active_pause: Option<ActiveCue> = None;
        let mut source_sample_index = 0_u64;
        let mut written_samples = 0_u64;
        let mut duck_cursor = 0_usize;
        let mut current_film_gain = 1.0_f32;
        let mut last_reported = 0_u32;

        loop {
            if options.cancel.load(Ordering::Relaxed) {
                return Err("cancelled".to_string());
            }

            if active_pause.is_none()
                && let Some(next_pause) = pending_pauses.front()
                && source_sample_index >= next_pause.start_sample
                && let Some(next_pause) = pending_pauses.pop_front()
            {
                active_pause = Some(ActiveCue {
                    samples: next_pause.samples,
                    read_offset: 0,
                });
            }

            if let Some(pause) = active_pause.as_mut() {
                if pause.read_offset < pause.samples.len() {
                    let value = pause.samples[pause.read_offset].clamp(-1.0, 1.0);
                    pause.read_offset += 1;
                    writer
                        .write_sample((value * i16::MAX as f32).round() as i16)
                        .map_err(|error| {
                            format!("Audio description: staging WAV write failed: {error}")
                        })?;
                    written_samples = written_samples.saturating_add(1);
                    continue;
                }
                active_pause = None;
                continue;
            }

            let Some(original_sample) = source.next() else {
                if let Some(next_pause) = pending_pauses.front()
                    && next_pause.start_sample <= source_sample_index
                {
                    continue;
                }
                break;
            };

            while let Some(front) = pending.front() {
                if source_sample_index >= front.start_sample {
                    if let Some(cue) = pending.pop_front() {
                        active.push(ActiveCue {
                            samples: cue.samples,
                            read_offset: 0,
                        });
                    }
                } else {
                    break;
                }
            }
            let mut narration_sample = 0.0_f32;
            active.retain_mut(|cue| {
                if cue.read_offset < cue.samples.len() {
                    narration_sample += cue.samples[cue.read_offset];
                    cue.read_offset += 1;
                    true
                } else {
                    false
                }
            });

            if source_sample_index.is_multiple_of(channels as u64) {
                let frame = source_sample_index / channels as u64;
                current_film_gain = audio_description_duck_gain(
                    frame,
                    &duck_intervals,
                    &mut duck_cursor,
                    attack_frames,
                    preduck_frames,
                    release_frames,
                    duck_gain,
                );
            }
            let mixed =
                mix_audio_description_sample(original_sample, current_film_gain, narration_sample);
            writer
                .write_sample((mixed * i16::MAX as f32).round() as i16)
                .map_err(|error| format!("Audio description: staging WAV write failed: {error}"))?;
            written_samples = written_samples.saturating_add(1);
            source_sample_index = source_sample_index.saturating_add(1);

            if let Some(total) = total_source_samples {
                let pct = ((source_sample_index.saturating_mul(94)) / total).min(94) as u32;
                if pct > last_reported {
                    last_reported = pct;
                    if let Some(callback) = progress.as_deref_mut() {
                        callback(pct);
                    }
                }
            }
        }
        let padding_samples = audio_description_wav_padding_samples(written_samples, channels);
        if padding_samples > 0 {
            log_debug(&format!(
                "Audio description export: incomplete final frame (written_samples={} channels={}); padding {} silent sample(s)",
                written_samples, channels, padding_samples
            ));
            for _ in 0..padding_samples {
                writer.write_sample(0_i16).map_err(|error| {
                    format!("Audio description: staging WAV padding failed: {error}")
                })?;
            }
        }
        writer
            .finalize()
            .map_err(|error| format!("Audio description: finalize staging WAV failed: {error}"))?;
        Ok(())
    })();

    if let Err(error) = mix_result {
        if let Err(cleanup_error) = fs::remove_file(&temp_wav) {
            crate::log_debug(&format!(
                "Audio description: temporary WAV cleanup failed: {cleanup_error}"
            ));
        }
        return Err(error);
    }

    if let Some(callback) = progress.as_mut() {
        callback(95);
    }
    let convert_settings = ConvertAudioSettings {
        format: ConvertAudioFormat::Mp3,
        quality: ConvertAudioQuality::BitrateKbps(options.bitrate_kbps.clamp(64, 320)),
    };
    let encode_result = {
        let mut encode_progress = |pct: u32| {
            if let Some(callback) = progress.as_mut() {
                callback(95 + pct.saturating_mul(5) / 10_000);
            }
        };
        convert_audio_file(
            &temp_wav,
            output_path,
            &convert_settings,
            Some(options.cancel.clone()),
            Some(&mut encode_progress),
        )
    };
    if let Err(error) = fs::remove_file(&temp_wav) {
        log_debug(&format!(
            "Audio description: unable to remove staging WAV {}: {}",
            temp_wav.display(),
            error
        ));
    }
    encode_result?;
    if let Some(callback) = progress.as_mut() {
        callback(100);
    }
    Ok(())
}

#[cfg(test)]
mod audio_description_export_tests {
    use super::{
        audio_description_duck_gain, merge_audio_description_intervals,
        mix_audio_description_sample,
    };

    #[test]
    fn merges_touching_duck_intervals() {
        assert_eq!(
            merge_audio_description_intervals(vec![(20, 30), (5, 10), (10, 15)], 0),
            vec![(5, 15), (20, 30)]
        );
    }

    #[test]
    fn merges_intervals_separated_by_less_than_one_fade() {
        assert_eq!(
            merge_audio_description_intervals(vec![(10, 20), (28, 40)], 10),
            vec![(10, 40)]
        );
    }

    #[test]
    fn omni_port_in_place_mix_preserves_length_and_changes_only_target_ranges() {
        let original = vec![0.5_f32; 300];
        let original_snapshot = original.clone();
        let intervals = vec![(100_u64, 200_u64)];
        let mut cursor = 0_usize;
        let mixed: Vec<f32> = original
            .iter()
            .enumerate()
            .map(|(frame, sample)| {
                let gain = audio_description_duck_gain(
                    frame as u64,
                    &intervals,
                    &mut cursor,
                    25,
                    10,
                    50,
                    0.2,
                );
                let narration = if (140..160).contains(&frame) {
                    0.25
                } else {
                    0.0
                };
                mix_audio_description_sample(*sample, gain, narration)
            })
            .collect();

        assert_eq!(mixed.len(), original.len());
        assert_eq!(original, original_snapshot);
        assert!((mixed[25] - original[25]).abs() < 0.001);
        assert!(mixed[120] < original[120] - 0.3);
        assert!((mixed[150] - 0.35).abs() < 0.001);
        assert!((mixed[250] - original[250]).abs() < 0.001);
    }

    #[test]
    fn duck_gain_uses_preduck_and_smooth_asymmetric_fades() {
        let intervals = vec![(100, 200)];
        let mut cursor = 0;
        // 25-frame attack starts at frame 65 and reaches full duck 10 frames
        // before narration begins.
        assert!(
            (audio_description_duck_gain(64, &intervals, &mut cursor, 25, 10, 50, 0.2) - 1.0).abs()
                < 0.001
        );
        assert!(
            (audio_description_duck_gain(90, &intervals, &mut cursor, 25, 10, 50, 0.2) - 0.2).abs()
                < 0.001
        );
        assert!(
            (audio_description_duck_gain(100, &intervals, &mut cursor, 25, 10, 50, 0.2) - 0.2)
                .abs()
                < 0.001
        );
        assert!(
            (audio_description_duck_gain(200, &intervals, &mut cursor, 25, 10, 50, 0.2) - 0.2)
                .abs()
                < 0.001
        );
        // Mid-release remains exactly halfway between ducked and full gain,
        // while the cosine curve makes the edges gentler than a linear fade.
        assert!(
            (audio_description_duck_gain(225, &intervals, &mut cursor, 25, 10, 50, 0.2) - 0.6)
                .abs()
                < 0.001
        );
        assert!(
            (audio_description_duck_gain(250, &intervals, &mut cursor, 25, 10, 50, 0.2) - 1.0)
                .abs()
                < 0.001
        );

        let mut cursor = 0;
        let quarter_attack =
            audio_description_duck_gain(71, &intervals, &mut cursor, 25, 10, 50, 0.2);
        let linear_quarter = 1.0 + (0.2 - 1.0) * (6.0 / 25.0);
        assert!(quarter_attack > linear_quarter);
    }
}

const AVERROR_EOF_FALLBACK: i32 =
    -((b'E' as i32) | ((b'O' as i32) << 8) | ((b'F' as i32) << 16) | ((b' ' as i32) << 24));

fn ffmpeg_is_eagain(code: i32) -> bool {
    code == -(EAGAIN as i32)
}

struct AacAdtsToAscFilter {
    context: *mut AVBSFContext,
    output_packet: *mut AVPacket,
}

impl AacAdtsToAscFilter {
    fn create(
        api: &FfmpegApi,
        input_stream: *mut AVStream,
        output_stream: *mut AVStream,
    ) -> Result<Self, String> {
        let filter_name = CString::new("aac_adtstoasc")
            .map_err(|_| "FFmpeg: invalid AAC bitstream filter name".to_string())?;
        let filter = unsafe { (api.av_bsf_get_by_name)(filter_name.as_ptr()) };
        if filter.is_null() {
            return Err("FFmpeg: internal aac_adtstoasc filter is unavailable".to_string());
        }

        let mut context: *mut AVBSFContext = ptr::null_mut();
        let alloc_ret = unsafe { (api.av_bsf_alloc)(filter, &mut context) };
        if alloc_ret < 0 || context.is_null() {
            return Err(format!(
                "FFmpeg: av_bsf_alloc(aac_adtstoasc) failed: {} ({})",
                ffmpeg_error_text(api, alloc_ret),
                alloc_ret
            ));
        }

        let configure_result = unsafe {
            let copy_ret = crate::ffmpeg_source::avcodec_parameters_copy_safe(
                api,
                (*context).par_in,
                (*input_stream).codecpar,
            );
            if copy_ret < 0 {
                Err(format!(
                    "FFmpeg: unable to configure AAC bitstream filter: {} ({})",
                    ffmpeg_error_text(api, copy_ret),
                    copy_ret
                ))
            } else {
                (*context).time_base_in = (*input_stream).time_base;
                let init_ret = (api.av_bsf_init)(context);
                if init_ret < 0 {
                    Err(format!(
                        "FFmpeg: av_bsf_init(aac_adtstoasc) failed: {} ({})",
                        ffmpeg_error_text(api, init_ret),
                        init_ret
                    ))
                } else {
                    let out_copy_ret = crate::ffmpeg_source::avcodec_parameters_copy_safe(
                        api,
                        (*output_stream).codecpar,
                        (*context).par_out,
                    );
                    if out_copy_ret < 0 {
                        Err(format!(
                            "FFmpeg: unable to copy filtered AAC parameters: {} ({})",
                            ffmpeg_error_text(api, out_copy_ret),
                            out_copy_ret
                        ))
                    } else {
                        (*(*output_stream).codecpar).codec_tag = 0;
                        (*output_stream).time_base = (*context).time_base_out;
                        Ok(())
                    }
                }
            }
        };
        if let Err(error) = configure_result {
            unsafe { (api.av_bsf_free)(&mut context) };
            return Err(error);
        }

        let output_packet = crate::ffmpeg_source::av_packet_alloc_safe(api);
        if output_packet.is_null() {
            unsafe { (api.av_bsf_free)(&mut context) };
            return Err("FFmpeg: packet allocation failed for AAC bitstream filter".to_string());
        }

        log_debug("FFmpeg: internal aac_adtstoasc filter enabled for live MP4 recording");
        Ok(Self {
            context,
            output_packet,
        })
    }

    fn output_time_base(&self) -> AVRational {
        unsafe { (*self.context).time_base_out }
    }

    fn send(&mut self, api: &FfmpegApi, packet: *mut AVPacket) -> Result<(), String> {
        let ret = unsafe { (api.av_bsf_send_packet)(self.context, packet) };
        if ret < 0 {
            return Err(format!(
                "FFmpeg: av_bsf_send_packet(aac_adtstoasc) failed: {} ({})",
                ffmpeg_error_text(api, ret),
                ret
            ));
        }
        Ok(())
    }

    fn receive(&mut self, api: &FfmpegApi) -> Result<Option<*mut AVPacket>, String> {
        crate::ffmpeg_source::av_packet_unref_safe(api, self.output_packet);
        let ret = unsafe { (api.av_bsf_receive_packet)(self.context, self.output_packet) };
        if ret == 0 {
            return Ok(Some(self.output_packet));
        }
        if ffmpeg_is_eagain(ret) || ret == AVERROR_EOF_FALLBACK {
            return Ok(None);
        }
        Err(format!(
            "FFmpeg: av_bsf_receive_packet(aac_adtstoasc) failed: {} ({})",
            ffmpeg_error_text(api, ret),
            ret
        ))
    }

    fn flush(&mut self, api: &FfmpegApi) -> Result<(), String> {
        let ret = unsafe { (api.av_bsf_send_packet)(self.context, ptr::null_mut()) };
        if ret < 0 && ret != AVERROR_EOF_FALLBACK {
            return Err(format!(
                "FFmpeg: unable to flush aac_adtstoasc: {} ({})",
                ffmpeg_error_text(api, ret),
                ret
            ));
        }
        Ok(())
    }

    fn free(&mut self, api: &FfmpegApi) {
        if !self.output_packet.is_null() {
            crate::ffmpeg_source::av_packet_free_safe(api, &mut self.output_packet);
        }
        if !self.context.is_null() {
            unsafe { (api.av_bsf_free)(&mut self.context) };
        }
    }
}

struct MuxVideoWithAudioOptions<'a> {
    preferred_audio_stream_index: Option<i32>,
    cancel: Option<Arc<AtomicBool>>,
    progress: Option<&'a mut dyn FnMut(u32)>,
    input_user_agent: Option<&'a str>,
    input_referer: Option<&'a str>,
    graceful_stop: bool,
}

fn mux_video_with_audio(
    api: &FfmpegApi,
    video_path: &Path,
    audio_path: &Path,
    out_path: &Path,
    options: MuxVideoWithAudioOptions<'_>,
) -> Result<(), String> {
    let MuxVideoWithAudioOptions {
        preferred_audio_stream_index,
        cancel,
        mut progress,
        input_user_agent,
        input_referer,
        graceful_stop,
    } = options;
    let hls_progress_estimate = estimate_hls_progress(video_path);
    let video_c = CString::new(video_path.to_string_lossy().as_bytes())
        .map_err(|_| "FFmpeg: invalid video path".to_string())?;
    let audio_c = CString::new(audio_path.to_string_lossy().as_bytes())
        .map_err(|_| "FFmpeg: invalid audio path".to_string())?;
    let out_c = CString::new(out_path.to_string_lossy().as_bytes())
        .map_err(|_| "FFmpeg: invalid output path".to_string())?;

    let mut in_video: *mut AVFormatContext = ptr::null_mut();
    let mut in_audio: *mut AVFormatContext = ptr::null_mut();
    let mut out_ctx: *mut AVFormatContext = ptr::null_mut();

    let open_vid = open_input_with_network_options(
        api,
        video_c.as_ptr(),
        &mut in_video,
        input_user_agent,
        input_referer,
        graceful_stop,
    );
    if open_vid < 0 || in_video.is_null() {
        return Err(format!(
            "FFmpeg: failed to open video input: {} ({})",
            ffmpeg_error_text(api, open_vid),
            open_vid
        ));
    }
    let open_aud = open_input_with_network_options(
        api,
        audio_c.as_ptr(),
        &mut in_audio,
        input_user_agent,
        input_referer,
        graceful_stop,
    );
    if open_aud < 0 || in_audio.is_null() {
        crate::ffmpeg_source::avformat_close_input_safe(api, &mut in_video);
        return Err(format!(
            "FFmpeg: failed to open audio input: {} ({})",
            ffmpeg_error_text(api, open_aud),
            open_aud
        ));
    }
    if crate::ffmpeg_source::avformat_find_stream_info_safe(api, in_video, ptr::null_mut()) < 0 {
        unsafe {
            (api.avformat_close_input)(&mut in_audio);
            (api.avformat_close_input)(&mut in_video);
        }
        return Err("FFmpeg: video stream info failed".to_string());
    }
    if crate::ffmpeg_source::avformat_find_stream_info_safe(api, in_audio, ptr::null_mut()) < 0 {
        unsafe {
            (api.avformat_close_input)(&mut in_audio);
            (api.avformat_close_input)(&mut in_video);
        }
        return Err("FFmpeg: audio stream info failed".to_string());
    }
    let total_duration_us = unsafe {
        let duration = (*in_video).duration;
        if duration > 0 { Some(duration) } else { None }
    };

    let video_stream_idx = crate::ffmpeg_source::av_find_best_stream_safe(
        api,
        in_video,
        AVMediaType_AVMEDIA_TYPE_VIDEO,
        -1,
        -1,
        ptr::null_mut(),
        0,
    );
    if video_stream_idx < 0 {
        unsafe {
            (api.avformat_close_input)(&mut in_audio);
            (api.avformat_close_input)(&mut in_video);
        }
        return Err("FFmpeg: video stream not found".to_string());
    }
    let audio_stream_idx = preferred_audio_stream_index
        .filter(|preferred_index| unsafe {
            let stream_count = (*in_audio).nb_streams as usize;
            let idx = *preferred_index as usize;
            idx < stream_count && !(*in_audio).streams.is_null() && {
                let stream = *(*in_audio).streams.add(idx);
                !stream.is_null()
                    && !(*stream).codecpar.is_null()
                    && (*(*stream).codecpar).codec_type == AVMediaType_AVMEDIA_TYPE_AUDIO
            }
        })
        .unwrap_or_else(|| {
            crate::ffmpeg_source::av_find_best_stream_safe(
                api,
                in_audio,
                AVMediaType_AVMEDIA_TYPE_AUDIO,
                -1,
                -1,
                ptr::null_mut(),
                0,
            )
        });
    if audio_stream_idx < 0 {
        unsafe {
            (api.avformat_close_input)(&mut in_audio);
            (api.avformat_close_input)(&mut in_video);
        }
        return Err("FFmpeg: audio stream not found".to_string());
    }

    let alloc_ret = crate::ffmpeg_source::avformat_alloc_output_context2_safe(
        api,
        &mut out_ctx,
        ptr::null_mut(),
        ptr::null(),
        out_c.as_ptr(),
    );
    if alloc_ret < 0 || out_ctx.is_null() {
        unsafe {
            (api.avformat_close_input)(&mut in_audio);
            (api.avformat_close_input)(&mut in_video);
        }
        return Err("FFmpeg: failed to alloc output context".to_string());
    }

    let in_v_stream = unsafe { *(*in_video).streams.add(video_stream_idx as usize) };
    let in_a_stream = unsafe { *(*in_audio).streams.add(audio_stream_idx as usize) };
    let out_v_stream = crate::ffmpeg_source::avformat_new_stream_safe(api, out_ctx, ptr::null());
    let out_a_stream = crate::ffmpeg_source::avformat_new_stream_safe(api, out_ctx, ptr::null());
    if out_v_stream.is_null() || out_a_stream.is_null() {
        unsafe {
            (api.avformat_free_context)(out_ctx);
            (api.avformat_close_input)(&mut in_audio);
            (api.avformat_close_input)(&mut in_video);
        }
        return Err("FFmpeg: failed to create output streams".to_string());
    }
    unsafe {
        crate::ffmpeg_source::avcodec_parameters_copy_safe(
            api,
            (*out_v_stream).codecpar,
            (*in_v_stream).codecpar,
        );
        (*(*out_v_stream).codecpar).codec_tag = 0;
        (*out_v_stream).time_base = (*in_v_stream).time_base;
    }

    let use_aac_filter = graceful_stop
        && unsafe { (*(*in_a_stream).codecpar).codec_id == AVCodecID_AV_CODEC_ID_AAC };
    let mut aac_filter = if use_aac_filter {
        match AacAdtsToAscFilter::create(api, in_a_stream, out_a_stream) {
            Ok(filter) => Some(filter),
            Err(error) => {
                unsafe {
                    (api.avformat_free_context)(out_ctx);
                    (api.avformat_close_input)(&mut in_audio);
                    (api.avformat_close_input)(&mut in_video);
                }
                return Err(error);
            }
        }
    } else {
        unsafe {
            crate::ffmpeg_source::avcodec_parameters_copy_safe(
                api,
                (*out_a_stream).codecpar,
                (*in_a_stream).codecpar,
            );
            (*(*out_a_stream).codecpar).codec_tag = 0;
            (*out_a_stream).time_base = (*in_a_stream).time_base;
        }
        None
    };

    let mut io: *mut AVIOContext = ptr::null_mut();
    let open_io =
        crate::ffmpeg_source::avio_open_safe(api, &mut io, out_c.as_ptr(), AVIO_FLAG_WRITE);
    if open_io < 0 {
        if let Some(filter) = aac_filter.as_mut() {
            filter.free(api);
        }
        unsafe {
            (api.avformat_free_context)(out_ctx);
            (api.avformat_close_input)(&mut in_audio);
            (api.avformat_close_input)(&mut in_video);
        }
        return Err("FFmpeg: failed to open output file".to_string());
    }
    unsafe {
        (*out_ctx).pb = io;
    }
    let mut header_options: *mut AVDictionary = ptr::null_mut();
    if graceful_stop
        && let Err(error) = dict_set_str(
            api,
            &mut header_options,
            "movflags",
            "frag_keyframe+empty_moov+default_base_moof",
        )
    {
        crate::ffmpeg_source::av_dict_free_safe(api, &mut header_options);
        if let Some(filter) = aac_filter.as_mut() {
            filter.free(api);
        }
        unsafe {
            (api.avio_closep)(&mut io);
            (api.avformat_free_context)(out_ctx);
            (api.avformat_close_input)(&mut in_audio);
            (api.avformat_close_input)(&mut in_video);
        }
        return Err(error);
    }
    let header_ret =
        crate::ffmpeg_source::avformat_write_header_safe(api, out_ctx, &mut header_options);
    crate::ffmpeg_source::av_dict_free_safe(api, &mut header_options);
    if header_ret < 0 {
        if let Some(filter) = aac_filter.as_mut() {
            filter.free(api);
        }
        unsafe {
            (api.avio_closep)(&mut io);
            (api.avformat_free_context)(out_ctx);
            (api.avformat_close_input)(&mut in_audio);
            (api.avformat_close_input)(&mut in_video);
        }
        return Err("FFmpeg: failed to write header".to_string());
    }

    let mut pkt_v = crate::ffmpeg_source::av_packet_alloc_safe(api);
    let mut pkt_a = crate::ffmpeg_source::av_packet_alloc_safe(api);
    if pkt_v.is_null() || pkt_a.is_null() {
        if !pkt_v.is_null() {
            crate::ffmpeg_source::av_packet_free_safe(api, &mut pkt_v);
        }
        if !pkt_a.is_null() {
            crate::ffmpeg_source::av_packet_free_safe(api, &mut pkt_a);
        }
        if let Some(filter) = aac_filter.as_mut() {
            filter.free(api);
        }
        unsafe {
            (api.avio_closep)(&mut io);
            (api.avformat_free_context)(out_ctx);
            (api.avformat_close_input)(&mut in_audio);
            (api.avformat_close_input)(&mut in_video);
        }
        return Err("FFmpeg: packet allocation failed while starting recording".to_string());
    }
    let mut has_v = read_next_packet_for_stream(api, in_video, pkt_v, video_stream_idx);
    let mut has_a = read_next_packet_for_stream(api, in_audio, pkt_a, audio_stream_idx);
    let video_timestamp_origin = if graceful_stop && has_v {
        packet_reference_timestamp(pkt_v)
    } else {
        None
    };
    let audio_timestamp_origin = if graceful_stop && has_a {
        packet_reference_timestamp(pkt_a)
    } else {
        None
    };
    if graceful_stop {
        log_debug(&format!(
            "FFmpeg live timestamp origins: video={:?} audio={:?}",
            video_timestamp_origin, audio_timestamp_origin
        ));
        unsafe {
            log_debug(&format!(
                "FFmpeg live first packets: video_pts={} video_dts={} video_duration={} video_tb={}/{} audio_pts={} audio_dts={} audio_duration={} audio_tb={}/{}",
                (*pkt_v).pts,
                (*pkt_v).dts,
                (*pkt_v).duration,
                (*in_v_stream).time_base.num,
                (*in_v_stream).time_base.den,
                (*pkt_a).pts,
                (*pkt_a).dts,
                (*pkt_a).duration,
                (*in_a_stream).time_base.num,
                (*in_a_stream).time_base.den
            ));
        }
    }
    let mut progress_state = MuxProgressState {
        start_us: None,
        cursor_us: 0,
        last_pct: None,
        last_hls_log_pct_bucket: None,
    };
    let mut write_error: Option<String> = None;

    'mux_loop: while has_v || has_a {
        if cancel
            .as_ref()
            .map(|flag| flag.load(Ordering::Relaxed))
            .unwrap_or(false)
        {
            if graceful_stop {
                break;
            }
            unsafe {
                if !pkt_v.is_null() {
                    crate::ffmpeg_source::av_packet_free_safe(api, &mut pkt_v);
                }
                if !pkt_a.is_null() {
                    crate::ffmpeg_source::av_packet_free_safe(api, &mut pkt_a);
                }
                if let Some(filter) = aac_filter.as_mut() {
                    filter.free(api);
                }
                (api.avio_closep)(&mut io);
                (api.avformat_free_context)(out_ctx);
                (api.avformat_close_input)(&mut in_audio);
                (api.avformat_close_input)(&mut in_video);
            }
            return Err("Saving canceled.".to_string());
        }
        let write_video = if !has_a {
            true
        } else if !has_v {
            false
        } else {
            let vts = relative_packet_timestamp(pkt_v, video_timestamp_origin);
            let ats = relative_packet_timestamp(pkt_a, audio_timestamp_origin);
            let v_cmp = rescale_q(vts, unsafe { (*in_v_stream).time_base }, unsafe {
                (*out_v_stream).time_base
            });
            let a_cmp = rescale_q(ats, unsafe { (*in_a_stream).time_base }, unsafe {
                (*out_a_stream).time_base
            });
            v_cmp <= a_cmp
        };

        if write_video {
            normalize_live_packet_timestamps(pkt_v, video_timestamp_origin);
            unsafe {
                (*pkt_v).stream_index = (*out_v_stream).index;
                crate::ffmpeg_source::av_packet_rescale_ts_safe(
                    api,
                    pkt_v,
                    (*in_v_stream).time_base,
                    (*out_v_stream).time_base,
                );
                let write_ret =
                    crate::ffmpeg_source::av_interleaved_write_frame_safe(api, out_ctx, pkt_v);
                if write_ret < 0 {
                    let error = format!(
                        "FFmpeg: av_interleaved_write_frame (V) failed: {} ({})",
                        ffmpeg_error_text(api, write_ret),
                        write_ret
                    );
                    log_debug(&error);
                    write_error = Some(error);
                    crate::ffmpeg_source::av_packet_unref_safe(api, pkt_v);
                    break 'mux_loop;
                }
                if let Some(progress_cb) = progress.as_deref_mut() {
                    report_mux_progress(
                        progress_cb,
                        total_duration_us,
                        MuxProgressSample {
                            pts: (*pkt_v).pts,
                            dts: (*pkt_v).dts,
                            duration: (*pkt_v).duration,
                            time_base: (*in_v_stream).time_base,
                            out_path,
                            hls_progress_estimate: hls_progress_estimate.as_ref(),
                        },
                        &mut progress_state,
                    );
                }
                crate::ffmpeg_source::av_packet_unref_safe(api, pkt_v);
            }
            has_v = read_next_packet_for_stream(api, in_video, pkt_v, video_stream_idx);
        } else {
            normalize_live_packet_timestamps(pkt_a, audio_timestamp_origin);
            let input_progress_sample = unsafe {
                MuxProgressSample {
                    pts: (*pkt_a).pts,
                    dts: (*pkt_a).dts,
                    duration: (*pkt_a).duration,
                    time_base: (*in_a_stream).time_base,
                    out_path,
                    hls_progress_estimate: hls_progress_estimate.as_ref(),
                }
            };
            if let Some(filter) = aac_filter.as_mut() {
                if let Err(error) = filter.send(api, pkt_a) {
                    log_debug(&error);
                    write_error = Some(error);
                    break 'mux_loop;
                }
                loop {
                    let filtered_packet = match filter.receive(api) {
                        Ok(Some(packet)) => packet,
                        Ok(None) => break,
                        Err(error) => {
                            log_debug(&error);
                            write_error = Some(error);
                            break 'mux_loop;
                        }
                    };
                    unsafe {
                        (*filtered_packet).stream_index = (*out_a_stream).index;
                        crate::ffmpeg_source::av_packet_rescale_ts_safe(
                            api,
                            filtered_packet,
                            filter.output_time_base(),
                            (*out_a_stream).time_base,
                        );
                    }
                    let write_ret = crate::ffmpeg_source::av_interleaved_write_frame_safe(
                        api,
                        out_ctx,
                        filtered_packet,
                    );
                    if write_ret < 0 {
                        let error = format!(
                            "FFmpeg: av_interleaved_write_frame (A filtered) failed: {} ({})",
                            ffmpeg_error_text(api, write_ret),
                            write_ret
                        );
                        log_debug(&error);
                        write_error = Some(error);
                        break 'mux_loop;
                    }
                }
            } else {
                unsafe {
                    (*pkt_a).stream_index = (*out_a_stream).index;
                    crate::ffmpeg_source::av_packet_rescale_ts_safe(
                        api,
                        pkt_a,
                        (*in_a_stream).time_base,
                        (*out_a_stream).time_base,
                    );
                }
                let write_ret =
                    crate::ffmpeg_source::av_interleaved_write_frame_safe(api, out_ctx, pkt_a);
                if write_ret < 0 {
                    let error = format!(
                        "FFmpeg: av_interleaved_write_frame (A) failed: {} ({})",
                        ffmpeg_error_text(api, write_ret),
                        write_ret
                    );
                    log_debug(&error);
                    write_error = Some(error);
                    crate::ffmpeg_source::av_packet_unref_safe(api, pkt_a);
                    break 'mux_loop;
                }
                crate::ffmpeg_source::av_packet_unref_safe(api, pkt_a);
            }
            if let Some(progress_cb) = progress.as_deref_mut() {
                report_mux_progress(
                    progress_cb,
                    total_duration_us,
                    input_progress_sample,
                    &mut progress_state,
                );
            }
            has_a = read_next_packet_for_stream(api, in_audio, pkt_a, audio_stream_idx);
        }
    }

    if write_error.is_none()
        && let Some(filter) = aac_filter.as_mut()
    {
        if let Err(error) = filter.flush(api) {
            log_debug(&error);
            write_error = Some(error);
        } else {
            loop {
                let filtered_packet = match filter.receive(api) {
                    Ok(Some(packet)) => packet,
                    Ok(None) => break,
                    Err(error) => {
                        log_debug(&error);
                        write_error = Some(error);
                        break;
                    }
                };
                unsafe {
                    (*filtered_packet).stream_index = (*out_a_stream).index;
                    crate::ffmpeg_source::av_packet_rescale_ts_safe(
                        api,
                        filtered_packet,
                        filter.output_time_base(),
                        (*out_a_stream).time_base,
                    );
                }
                let write_ret = crate::ffmpeg_source::av_interleaved_write_frame_safe(
                    api,
                    out_ctx,
                    filtered_packet,
                );
                if write_ret < 0 {
                    let error = format!(
                        "FFmpeg: av_interleaved_write_frame (A filter flush) failed: {} ({})",
                        ffmpeg_error_text(api, write_ret),
                        write_ret
                    );
                    log_debug(&error);
                    write_error = Some(error);
                    break;
                }
            }
        }
    }

    unsafe {
        let trailer_ret = crate::ffmpeg_source::av_write_trailer_safe(api, out_ctx);
        if trailer_ret < 0 {
            log_debug(&format!("FFmpeg: av_write_trailer failed: {}", trailer_ret));
        }
        if !pkt_v.is_null() {
            crate::ffmpeg_source::av_packet_free_safe(api, &mut pkt_v);
        }
        if !pkt_a.is_null() {
            crate::ffmpeg_source::av_packet_free_safe(api, &mut pkt_a);
        }
        if let Some(filter) = aac_filter.as_mut() {
            filter.free(api);
        }
        (api.avio_closep)(&mut io);
        (api.avformat_free_context)(out_ctx);
        (api.avformat_close_input)(&mut in_audio);
        (api.avformat_close_input)(&mut in_video);
    }
    if let Some(error) = write_error {
        return Err(error);
    }
    Ok(())
}

fn report_mux_progress(
    progress_cb: &mut dyn FnMut(u32),
    total_duration_us: Option<i64>,
    sample: MuxProgressSample<'_>,
    progress_state: &mut MuxProgressState,
) {
    let pts_us = if sample.pts != AV_NOPTS_VALUE_I64 {
        Some(rescale_q(
            sample.pts,
            sample.time_base,
            AVRational {
                num: 1,
                den: 1_000_000,
            },
        ))
    } else {
        None
    };
    let dts_us = if sample.dts != AV_NOPTS_VALUE_I64 {
        Some(rescale_q(
            sample.dts,
            sample.time_base,
            AVRational {
                num: 1,
                den: 1_000_000,
            },
        ))
    } else {
        None
    };
    let duration_us = if sample.duration > 0 {
        rescale_q(
            sample.duration,
            sample.time_base,
            AVRational {
                num: 1,
                den: 1_000_000,
            },
        )
        .max(0)
    } else {
        0
    };
    if let Some(estimate) = sample.hls_progress_estimate {
        let mut current_us = pts_us.or(dts_us).unwrap_or(progress_state.cursor_us);
        if duration_us > 0 && current_us <= progress_state.cursor_us {
            current_us = progress_state.cursor_us.saturating_add(duration_us);
        }
        progress_state.cursor_us = progress_state.cursor_us.max(current_us);
        let start_us = *progress_state
            .start_us
            .get_or_insert(progress_state.cursor_us.max(0));
        let elapsed_us = progress_state
            .cursor_us
            .saturating_sub(start_us)
            .clamp(0, estimate.total_duration_us);
        let mut pct = (((elapsed_us as u128) * 100) / estimate.total_duration_us as u128) as u32;
        if elapsed_us > 0 && pct == 0 {
            pct = 1;
        }
        if pct > 0 {
            let pct = pct.min(99);
            let log_bucket = if pct < 5 { pct } else { (pct / 10) * 10 };
            if progress_state
                .last_hls_log_pct_bucket
                .is_none_or(|previous| previous != log_bucket)
            {
                progress_state.last_hls_log_pct_bucket = Some(log_bucket);
                let written = fs::metadata(sample.out_path)
                    .map(|meta| meta.len())
                    .unwrap_or(0);
                log_debug(&format!(
                    "FFmpeg HLS progress: out={} elapsed_us={} total_duration_us={} written_bytes={} estimated_total_bytes={} pct={} bandwidth_bps={} duration_secs={:.3} variant={}",
                    sample.out_path.display(),
                    elapsed_us,
                    estimate.total_duration_us,
                    written,
                    estimate.estimated_total_bytes,
                    pct,
                    estimate.bandwidth_bits_per_sec,
                    estimate.duration_secs,
                    estimate.variant_url
                ));
            }
            if progress_state
                .last_pct
                .is_some_and(|previous| previous == pct)
            {
                return;
            }
            progress_state.last_pct = Some(pct);
            progress_cb(pct);
            return;
        }
        if estimate.estimated_total_bytes > 0
            && let Ok(meta) = fs::metadata(sample.out_path)
        {
            let written = meta.len().min(estimate.estimated_total_bytes);
            let pct = ((written as u128 * 100) / estimate.estimated_total_bytes as u128) as u32;
            let pct = pct.clamp(1, 99);
            let log_bucket = if pct < 5 { pct } else { (pct / 10) * 10 };
            if progress_state
                .last_hls_log_pct_bucket
                .is_none_or(|previous| previous != log_bucket)
            {
                progress_state.last_hls_log_pct_bucket = Some(log_bucket);
                log_debug(&format!(
                    "FFmpeg HLS progress: out={} elapsed_us=0 total_duration_us={} written_bytes={} estimated_total_bytes={} pct={} bandwidth_bps={} duration_secs={:.3} variant={}",
                    sample.out_path.display(),
                    estimate.total_duration_us,
                    written,
                    estimate.estimated_total_bytes,
                    pct,
                    estimate.bandwidth_bits_per_sec,
                    estimate.duration_secs,
                    estimate.variant_url
                ));
            }
            if progress_state
                .last_pct
                .is_some_and(|previous| previous == pct)
            {
                return;
            }
            progress_state.last_pct = Some(pct);
            progress_cb(pct);
            return;
        }
    }
    let Some(total_duration_us) = total_duration_us.filter(|value| *value > 0) else {
        return;
    };
    let mut current_us = pts_us.or(dts_us).unwrap_or(progress_state.cursor_us);
    if duration_us > 0 && current_us <= progress_state.cursor_us {
        current_us = progress_state.cursor_us.saturating_add(duration_us);
    }
    progress_state.cursor_us = progress_state.cursor_us.max(current_us);
    let start_us = *progress_state
        .start_us
        .get_or_insert(progress_state.cursor_us.max(0));
    let elapsed_us = progress_state
        .cursor_us
        .saturating_sub(start_us)
        .clamp(0, total_duration_us);
    let mut pct = (((elapsed_us as u128) * 100) / total_duration_us as u128) as u32;
    if elapsed_us > 0 && pct == 0 {
        pct = 1;
    }
    let pct = pct.min(99);
    if progress_state
        .last_pct
        .is_some_and(|previous| previous == pct)
    {
        return;
    }
    progress_state.last_pct = Some(pct);
    progress_cb(pct);
}

fn resolve_hls_recording_inputs(
    input_url: &str,
    prefer_audio_description: bool,
) -> Option<HlsRecordingInputs> {
    let input_url = input_url.trim();
    if !input_url.to_ascii_lowercase().contains(".m3u8") {
        return None;
    }

    let bytes = crate::curl_client::CurlClient::fetch_url_impersonated(input_url).ok()?;
    let playlist = String::from_utf8(bytes).ok()?;
    if !playlist.lines().any(|line| {
        line.trim_start()
            .to_ascii_uppercase()
            .starts_with("#EXT-X-STREAM-INF:")
    }) {
        return None;
    }

    let mut audio_renditions = Vec::new();
    let mut variants = Vec::new();
    let mut pending_variant_attributes: Option<Vec<(String, String)>> = None;

    for line in playlist.lines() {
        let trimmed = line.trim();
        if let Some(attributes) = trimmed.strip_prefix("#EXT-X-MEDIA:") {
            let attributes = parse_hls_attributes(attributes);
            if hls_attribute(&attributes, "TYPE")
                .is_some_and(|value| value.eq_ignore_ascii_case("AUDIO"))
            {
                let Some(group_id) = hls_attribute(&attributes, "GROUP-ID") else {
                    continue;
                };
                audio_renditions.push(HlsAudioRendition {
                    group_id: group_id.to_string(),
                    uri: hls_attribute(&attributes, "URI").map(|value| value.to_string()),
                    language: hls_attribute(&attributes, "LANGUAGE")
                        .unwrap_or_default()
                        .to_string(),
                    name: hls_attribute(&attributes, "NAME")
                        .unwrap_or_default()
                        .to_string(),
                    is_default: hls_yes_attribute(&attributes, "DEFAULT"),
                    autoselect: hls_yes_attribute(&attributes, "AUTOSELECT"),
                });
            }
            continue;
        }
        if let Some(attributes) = trimmed.strip_prefix("#EXT-X-STREAM-INF:") {
            pending_variant_attributes = Some(parse_hls_attributes(attributes));
            continue;
        }
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let Some(attributes) = pending_variant_attributes.take() else {
            continue;
        };
        let bandwidth_bits_per_sec = hls_attribute(&attributes, "AVERAGE-BANDWIDTH")
            .or_else(|| hls_attribute(&attributes, "BANDWIDTH"))
            .and_then(|value| value.parse::<u64>().ok())
            .unwrap_or(0);
        variants.push(HlsVideoVariant {
            url: resolve_hls_child_url(input_url, trimmed),
            bandwidth_bits_per_sec,
            audio_group: hls_attribute(&attributes, "AUDIO").map(|value| value.to_string()),
        });
    }

    let selected_variant = variants
        .into_iter()
        .max_by_key(|variant| variant.bandwidth_bits_per_sec)?;
    let audio_url = selected_variant
        .audio_group
        .as_deref()
        .and_then(|group_id| {
            select_hls_audio_rendition(&audio_renditions, group_id, prefer_audio_description)
        })
        .and_then(|rendition| rendition.uri.as_deref())
        .map(|uri| resolve_hls_child_url(input_url, uri))
        .unwrap_or_else(|| selected_variant.url.clone());

    log_debug(&format!(
        "FFmpeg HLS recording inputs: master={} video={} audio={} bandwidth_bps={} prefer_audio_description={}",
        input_url,
        selected_variant.url,
        audio_url,
        selected_variant.bandwidth_bits_per_sec,
        prefer_audio_description
    ));

    Some(HlsRecordingInputs {
        video_url: selected_variant.url,
        audio_url,
    })
}

fn select_hls_audio_rendition<'a>(
    renditions: &'a [HlsAudioRendition],
    group_id: &str,
    prefer_audio_description: bool,
) -> Option<&'a HlsAudioRendition> {
    renditions
        .iter()
        .filter(|rendition| rendition.group_id == group_id)
        .max_by_key(|rendition| {
            let description =
                format!("{} {}", rendition.language, rendition.name).to_ascii_lowercase();
            let is_audio_description = description.contains("audiodesc")
                || description.contains("audio desc")
                || description.contains("description")
                || description.split_whitespace().any(|part| part == "des");
            let is_italian = description.contains("ital")
                || description
                    .split_whitespace()
                    .any(|part| part == "it" || part == "ita");
            let description_score = if prefer_audio_description && is_audio_description {
                10_000
            } else if !prefer_audio_description && !is_audio_description {
                2_000
            } else {
                0
            };
            description_score
                + usize::from(rendition.is_default) * 1_000
                + usize::from(is_italian) * 500
                + usize::from(rendition.autoselect) * 100
                + usize::from(rendition.uri.is_some())
        })
}

fn parse_hls_attributes(input: &str) -> Vec<(String, String)> {
    let mut attributes = Vec::new();
    let mut start = 0usize;
    let mut in_quotes = false;
    for (index, character) in input.char_indices() {
        match character {
            '"' => in_quotes = !in_quotes,
            ',' if !in_quotes => {
                push_hls_attribute(&mut attributes, &input[start..index]);
                start = index + character.len_utf8();
            }
            _ => {}
        }
    }
    push_hls_attribute(&mut attributes, &input[start..]);
    attributes
}

fn push_hls_attribute(attributes: &mut Vec<(String, String)>, part: &str) {
    let Some((key, value)) = part.split_once('=') else {
        return;
    };
    attributes.push((
        key.trim().to_ascii_uppercase(),
        value.trim().trim_matches('"').to_string(),
    ));
}

fn hls_attribute<'a>(attributes: &'a [(String, String)], key: &str) -> Option<&'a str> {
    attributes
        .iter()
        .find(|(attribute_key, _)| attribute_key.eq_ignore_ascii_case(key))
        .map(|(_, value)| value.as_str())
}

fn hls_yes_attribute(attributes: &[(String, String)], key: &str) -> bool {
    hls_attribute(attributes, key).is_some_and(|value| value.eq_ignore_ascii_case("YES"))
}

fn estimate_hls_progress(input_path: &Path) -> Option<HlsProgressEstimate> {
    let url = input_path.to_str()?.trim();
    if !url.contains(".m3u8") {
        return None;
    }
    let bytes = crate::curl_client::CurlClient::fetch_url_impersonated(url).ok()?;
    let playlist = String::from_utf8(bytes).ok()?;
    let (variant_url, bandwidth_bits_per_sec) = select_hls_video_variant(url, &playlist)?;
    let duration_secs = sum_hls_media_playlist_duration_secs(&variant_url)?;
    let estimated_total_bytes = ((duration_secs * bandwidth_bits_per_sec as f64) / 8.0)
        .round()
        .max(1.0) as u64;
    let total_duration_us = (duration_secs * 1_000_000.0).round().max(1.0) as i64;
    log_debug(&format!(
        "FFmpeg HLS estimate: master={} variant={} bandwidth_bps={} duration_secs={:.3} estimated_total_bytes={}",
        url, variant_url, bandwidth_bits_per_sec, duration_secs, estimated_total_bytes
    ));
    Some(HlsProgressEstimate {
        estimated_total_bytes,
        variant_url,
        bandwidth_bits_per_sec,
        duration_secs,
        total_duration_us,
    })
}

fn select_hls_video_variant(master_url: &str, playlist: &str) -> Option<(String, u64)> {
    let mut pending_bandwidth: Option<u64> = None;
    let mut best: Option<(String, u64)> = None;
    for line in playlist.lines() {
        let trimmed = line.trim();
        if let Some(attrs) = trimmed.strip_prefix("#EXT-X-STREAM-INF:") {
            pending_bandwidth = parse_hls_bandwidth(attrs);
            continue;
        }
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        if let Some(bandwidth) = pending_bandwidth.take() {
            let child = resolve_hls_child_url(master_url, trimmed);
            if best
                .as_ref()
                .map(|(_, current_bw)| bandwidth > *current_bw)
                .unwrap_or(true)
            {
                best = Some((child, bandwidth));
            }
        }
    }
    best
}

fn parse_hls_bandwidth(attrs: &str) -> Option<u64> {
    let mut average_bandwidth = None;
    let mut bandwidth = None;
    for part in attrs.split(',') {
        let Some((key, value)) = part.split_once('=') else {
            continue;
        };
        let key = key.trim();
        let value = value.trim();
        if key.eq_ignore_ascii_case("AVERAGE-BANDWIDTH") {
            average_bandwidth = value.parse::<u64>().ok();
        } else if key.eq_ignore_ascii_case("BANDWIDTH") {
            bandwidth = value.parse::<u64>().ok();
        }
    }
    average_bandwidth.or(bandwidth)
}

fn sum_hls_media_playlist_duration_secs(url: &str) -> Option<f64> {
    let bytes = crate::curl_client::CurlClient::fetch_url_impersonated(url).ok()?;
    let playlist = String::from_utf8(bytes).ok()?;
    let total = playlist.lines().fold(0.0f64, |acc, line| {
        let trimmed = line.trim();
        if let Some(value) = trimmed.strip_prefix("#EXTINF:") {
            let number = value.split(',').next().unwrap_or("").trim();
            if let Ok(seconds) = number.parse::<f64>() {
                return acc + seconds.max(0.0);
            }
        }
        acc
    });
    (total > 0.0).then_some(total)
}

fn resolve_hls_child_url(master_url: &str, child_uri: &str) -> String {
    let trimmed = child_uri.trim();
    if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
        return trimmed.to_string();
    }

    let (base_without_query, query_suffix) = master_url
        .split_once('?')
        .map(|(base, query)| (base, format!("?{query}")))
        .unwrap_or((master_url, String::new()));

    let mut base_parts = base_without_query.rsplitn(2, '/');
    let _file_name = base_parts.next();
    let parent = base_parts.next().unwrap_or(base_without_query);
    if trimmed.contains('?') {
        format!("{parent}/{trimmed}")
    } else {
        format!("{parent}/{trimmed}{query_suffix}")
    }
}

pub fn export_mixed_media(
    media_path: &Path,
    settings: &AppSettings,
    options: &MixExportOptions,
) -> Result<PathBuf, String> {
    let subtitle_path = find_subtitle_for_media(media_path)
        .ok_or_else(|| "Subtitle: not found for media".to_string())?;
    let api = ffmpeg_api()?;

    let mut hasher = sha2::Sha256::new();
    hasher.update(settings.tts_voice.as_bytes());
    hasher.update(settings.tts_rate.to_string().as_bytes());
    hasher.update(settings.tts_pitch.to_string().as_bytes());
    hasher.update(settings.tts_volume.to_string().as_bytes());
    hasher.update(settings.subtitle_offset_ms.to_string().as_bytes());
    hasher.update(if options.ducking { b"duck1" } else { b"duck0" });
    let mix_hash = hex::encode(hasher.finalize());

    let stem = media_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("media");

    // Output directory is the same as the source media file
    let output_dir = media_path
        .parent()
        .ok_or_else(|| "Subtitle: media path has no parent directory".to_string())?;

    // Cache directory for temporary files
    let cache_dir = crate::settings::settings_dir()
        .join("subtitle_cache")
        .join("mixed");
    if let Err(e) = std::fs::create_dir_all(&cache_dir) {
        return Err(format!("Subtitle: failed to create cache dir: {}", e));
    }

    // Final output goes next to the original media file
    let out_mp4 = output_dir.join(format!("{stem}{MIX_SUFFIX}{}.mp4", &mix_hash[..8]));
    // Temporary audio file in cache
    let out_audio = cache_dir.join(format!("{stem}{MIX_SUFFIX}{}.m4a", &mix_hash[..8]));

    log_debug(&format!(
        "Subtitle: output path will be {}",
        out_mp4.display()
    ));

    if out_mp4.exists() {
        log_debug(&format!(
            "Subtitle: mixed file already exists at {}",
            out_mp4.display()
        ));
        return Ok(out_mp4);
    }

    log_debug(&format!(
        "Subtitle: exporting mixed audio for {}",
        media_path.display()
    ));
    encode_mixed_audio_to_m4a(
        api,
        media_path,
        &subtitle_path,
        settings,
        options,
        &out_audio,
    )?;
    log_debug(&format!(
        "Subtitle: muxing video+audio to {}",
        out_mp4.display()
    ));
    mux_video_with_audio(
        api,
        media_path,
        &out_audio,
        &out_mp4,
        MuxVideoWithAudioOptions {
            preferred_audio_stream_index: None,
            cancel: None,
            progress: None,
            input_user_agent: None,
            input_referer: None,
            graceful_stop: false,
        },
    )?;
    if let Err(e) = std::fs::remove_file(&out_audio) {
        crate::log_debug(&format!(
            "Subtitle: failed to delete temp audio {}: {}",
            out_audio.display(),
            e
        ));
    }
    register_mix_output(&out_mp4);
    log_debug(&format!(
        "Subtitle: mixed file created successfully at {}",
        out_mp4.display()
    ));
    Ok(out_mp4)
}

pub fn is_mixed_output(path: &Path) -> bool {
    path.file_stem()
        .and_then(|s| s.to_str())
        .map(|s| s.contains(MIX_SUFFIX))
        .unwrap_or(false)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConvertAudioFormat {
    Mp3,
    Aac,
    Opus,
    Ogg,
    Flac,
    Wav,
    Aiff,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConvertAudioQuality {
    BitrateKbps(u32),
    OggQuality(u8),
    FlacCompression(u8),
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConvertAudioSettings {
    pub format: ConvertAudioFormat,
    pub quality: ConvertAudioQuality,
}

fn encoder_output_channels(format: ConvertAudioFormat, requested_channels: u16) -> u16 {
    let requested_channels = requested_channels.max(1);
    if matches!(format, ConvertAudioFormat::Mp3) {
        requested_channels.min(2)
    } else {
        requested_channels
    }
}

pub fn build_ffmpeg_args(settings: &ConvertAudioSettings) -> Vec<String> {
    match settings.format {
        ConvertAudioFormat::Mp3 => match settings.quality {
            ConvertAudioQuality::BitrateKbps(bitrate) => vec![
                "-c:a".to_string(),
                "libmp3lame".to_string(),
                "-b:a".to_string(),
                format!("{bitrate}k"),
            ],
            _ => vec!["-c:a".to_string(), "libmp3lame".to_string()],
        },
        ConvertAudioFormat::Aac => match settings.quality {
            ConvertAudioQuality::BitrateKbps(bitrate) => vec![
                "-c:a".to_string(),
                "aac".to_string(),
                "-b:a".to_string(),
                format!("{bitrate}k"),
            ],
            _ => vec!["-c:a".to_string(), "aac".to_string()],
        },
        ConvertAudioFormat::Opus => match settings.quality {
            ConvertAudioQuality::BitrateKbps(bitrate) => vec![
                "-c:a".to_string(),
                "libopus".to_string(),
                "-b:a".to_string(),
                format!("{bitrate}k"),
            ],
            _ => vec!["-c:a".to_string(), "libopus".to_string()],
        },
        ConvertAudioFormat::Ogg => match settings.quality {
            ConvertAudioQuality::OggQuality(q) => vec![
                "-c:a".to_string(),
                "libvorbis".to_string(),
                "-q:a".to_string(),
                q.to_string(),
            ],
            _ => vec!["-c:a".to_string(), "libvorbis".to_string()],
        },
        ConvertAudioFormat::Flac => match settings.quality {
            ConvertAudioQuality::FlacCompression(level) => vec![
                "-c:a".to_string(),
                "flac".to_string(),
                "-compression_level".to_string(),
                level.to_string(),
            ],
            _ => vec!["-c:a".to_string(), "flac".to_string()],
        },
        ConvertAudioFormat::Wav | ConvertAudioFormat::Aiff => {
            vec!["-c:a".to_string(), "pcm_s16le".to_string()]
        }
    }
}

pub fn validate_mp3_bitrate(value: i32) -> Result<u32, String> {
    if (64..=320).contains(&value) {
        Ok(value as u32)
    } else {
        Err("MP3 bitrate must be between 64 and 320 kbps".to_string())
    }
}

pub fn convert_audio_file(
    input_path: &Path,
    output_path: &Path,
    settings: &ConvertAudioSettings,
    cancel: Option<Arc<AtomicBool>>,
    progress: Option<&mut dyn FnMut(u32)>,
) -> Result<(), String> {
    convert_audio_file_with_stream_index(
        input_path,
        output_path,
        settings,
        ConvertAudioFileOptions {
            cancel,
            progress,
            forced_channels: None,
            preferred_stream_index: None,
            graceful_stop: false,
        },
    )
}

pub fn convert_audio_file_with_preferred_stream(
    input_path: &Path,
    output_path: &Path,
    settings: &ConvertAudioSettings,
    cancel: Option<Arc<AtomicBool>>,
    progress: Option<&mut dyn FnMut(u32)>,
    preferred_stream_index: Option<i32>,
) -> Result<(), String> {
    convert_audio_file_with_stream_index(
        input_path,
        output_path,
        settings,
        ConvertAudioFileOptions {
            cancel,
            progress,
            forced_channels: None,
            preferred_stream_index,
            graceful_stop: false,
        },
    )
}

pub fn remux_media_file_to_mp4_with_preferred_audio_stream(
    input_path: &Path,
    output_path: &Path,
    preferred_audio_stream_index: Option<i32>,
    cancel: Option<Arc<AtomicBool>>,
    progress: Option<&mut dyn FnMut(u32)>,
) -> Result<(), String> {
    let api = ffmpeg_api()?;
    mux_video_with_audio(
        api,
        input_path,
        input_path,
        output_path,
        MuxVideoWithAudioOptions {
            preferred_audio_stream_index,
            cancel,
            progress,
            input_user_agent: None,
            input_referer: None,
            graceful_stop: false,
        },
    )
}

pub fn remux_media_file_to_mp4_with_external_audio_stream(
    video_path: &Path,
    audio_path: &Path,
    output_path: &Path,
    cancel: Option<Arc<AtomicBool>>,
    progress: Option<&mut dyn FnMut(u32)>,
) -> Result<(), String> {
    let api = ffmpeg_api()?;
    mux_video_with_audio(
        api,
        video_path,
        audio_path,
        output_path,
        MuxVideoWithAudioOptions {
            preferred_audio_stream_index: None,
            cancel,
            progress,
            input_user_agent: None,
            input_referer: None,
            graceful_stop: false,
        },
    )
}

fn remove_partial_live_recording(path: &Path) {
    if let Err(error) = std::fs::remove_file(path)
        && error.kind() != std::io::ErrorKind::NotFound
    {
        log_debug(&format!(
            "FFmpeg: unable to remove partial live recording {}: {}",
            path.display(),
            error
        ));
    }
}

pub fn record_live_media_stream_to_mp4(
    input_url: &str,
    output_path: &Path,
    user_agent: Option<&str>,
    prefer_audio_description: bool,
    stop: Arc<AtomicBool>,
) -> Result<(), String> {
    const HLS_FALLBACK_USER_AGENT: &str = "Mozilla/5.0 (iPhone; CPU iPhone OS 17_5 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.5 Mobile/15E148 Safari/604.1";

    let api = ffmpeg_api()?;
    let resolved_inputs = resolve_hls_recording_inputs(input_url, prefer_audio_description);
    let video_url = resolved_inputs
        .as_ref()
        .map(|inputs| inputs.video_url.as_str())
        .unwrap_or(input_url);
    let audio_url = resolved_inputs
        .as_ref()
        .map(|inputs| inputs.audio_url.as_str())
        .unwrap_or(input_url);

    let run_attempt = |attempt_video_url: &str,
                       attempt_audio_url: &str,
                       attempt_user_agent: Option<&str>,
                       attempt_referer: Option<&str>| {
        mux_video_with_audio(
            api,
            Path::new(attempt_video_url),
            Path::new(attempt_audio_url),
            output_path,
            MuxVideoWithAudioOptions {
                preferred_audio_stream_index: None,
                cancel: Some(Arc::clone(&stop)),
                progress: None,
                input_user_agent: attempt_user_agent,
                input_referer: attempt_referer,
                graceful_stop: true,
            },
        )
    };

    let resolved_referer =
        ((video_url != input_url) || (audio_url != input_url)).then_some(input_url);

    match run_attempt(video_url, audio_url, user_agent, resolved_referer) {
        Ok(()) => Ok(()),
        Err(first_error) => {
            if stop.load(Ordering::Relaxed) {
                return Err(first_error);
            }
            remove_partial_live_recording(output_path);
            log_debug(&format!(
                "FFmpeg live recording first attempt failed; retrying resolved inputs with browser user-agent: {}",
                first_error
            ));

            match run_attempt(
                video_url,
                audio_url,
                Some(HLS_FALLBACK_USER_AGENT),
                resolved_referer,
            ) {
                Ok(()) => Ok(()),
                Err(second_error) => {
                    if stop.load(Ordering::Relaxed) {
                        return Err(second_error);
                    }
                    if video_url == input_url && audio_url == input_url {
                        return Err(second_error);
                    }
                    remove_partial_live_recording(output_path);
                    log_debug(&format!(
                        "FFmpeg live recording resolved-input retry failed; falling back to master playlist: {}",
                        second_error
                    ));
                    run_attempt(input_url, input_url, Some(HLS_FALLBACK_USER_AGENT), None).map_err(
                        |master_error| {
                            format!(
                                "{}; browser retry: {}; master fallback: {}",
                                first_error, second_error, master_error
                            )
                        },
                    )
                }
            }
        }
    }
}

pub fn record_live_audio_stream_to_mp3(
    input_url: &str,
    output_path: &Path,
    stop: Arc<AtomicBool>,
) -> Result<(), String> {
    let settings = ConvertAudioSettings {
        format: ConvertAudioFormat::Mp3,
        quality: ConvertAudioQuality::BitrateKbps(192),
    };
    convert_audio_file_with_stream_index(
        Path::new(input_url),
        output_path,
        &settings,
        ConvertAudioFileOptions {
            cancel: Some(stop),
            progress: None,
            forced_channels: None,
            preferred_stream_index: None,
            graceful_stop: true,
        },
    )
}

pub fn convert_audio_file_with_channels(
    input_path: &Path,
    output_path: &Path,
    settings: &ConvertAudioSettings,
    cancel: Option<Arc<AtomicBool>>,
    progress: Option<&mut dyn FnMut(u32)>,
    forced_channels: Option<u16>,
) -> Result<(), String> {
    convert_audio_file_with_stream_index(
        input_path,
        output_path,
        settings,
        ConvertAudioFileOptions {
            cancel,
            progress,
            forced_channels,
            preferred_stream_index: None,
            graceful_stop: false,
        },
    )
}

struct ConvertAudioFileOptions<'a> {
    cancel: Option<Arc<AtomicBool>>,
    progress: Option<&'a mut dyn FnMut(u32)>,
    forced_channels: Option<u16>,
    preferred_stream_index: Option<i32>,
    graceful_stop: bool,
}

fn convert_audio_file_with_stream_index(
    input_path: &Path,
    output_path: &Path,
    settings: &ConvertAudioSettings,
    options: ConvertAudioFileOptions<'_>,
) -> Result<(), String> {
    let ConvertAudioFileOptions {
        cancel,
        mut progress,
        forced_channels,
        preferred_stream_index,
        graceful_stop,
    } = options;
    let api = ffmpeg_api()?;
    let args = build_ffmpeg_args(settings);
    log_debug(&format!(
        "FFmpeg convert start: input={} output={} args={}",
        input_path.display(),
        output_path.display(),
        args.join(" ")
    ));

    let streams = list_audio_streams(input_path)?;
    let preferred_stream = preferred_stream_index
        .and_then(|preferred_index| {
            streams
                .iter()
                .find(|s| s.index == preferred_index)
                .map(|s| s.index)
        })
        .or_else(|| streams.iter().find(|s| s.is_default).map(|s| s.index))
        .or_else(|| streams.first().map(|s| s.index));
    let Some(stream_index) = preferred_stream else {
        return Err("FFmpeg: no audio streams found".to_string());
    };

    let pts_clock = Arc::new(AtomicI64::new(0));
    let mut source =
        FfmpegSource::try_new(input_path, 0, Some(pts_clock.clone()), Some(stream_index))?;
    let in_sample_rate = source.sample_rate();
    let in_channels = source.channels().max(1);
    let requested_out_channels = forced_channels.unwrap_or(in_channels).max(1);
    // libmp3lame only supports mono/stereo. Keep every existing mono/stereo
    // conversion untouched, but automatically downmix multichannel sources
    // (for example 5.1 movie audio used by Audio Description) to stereo.
    let out_channels = encoder_output_channels(settings.format, requested_out_channels);
    if out_channels != requested_out_channels {
        log_debug(&format!(
            "FFmpeg MP3: downmixing {} input/output channels to stereo for libmp3lame",
            requested_out_channels
        ));
    }
    let total_duration = source.total_duration();
    let total_duration_us = total_duration.and_then(|d| {
        let micros = d.as_micros();
        if micros == 0 {
            None
        } else {
            Some(micros.min(i64::MAX as u128) as i64)
        }
    });
    let total_frames = total_duration.and_then(|d| {
        if d.is_zero() {
            None
        } else {
            let frames = (d.as_secs_f64() * in_sample_rate as f64).round();
            if frames.is_finite() && frames > 0.0 {
                Some(frames as u64)
            } else {
                None
            }
        }
    });
    let out_sample_rate = match settings.format {
        ConvertAudioFormat::Aac => in_sample_rate,
        ConvertAudioFormat::Opus => 48_000,
        ConvertAudioFormat::Mp3 => {
            let requested_bitrate = match settings.quality {
                ConvertAudioQuality::BitrateKbps(bitrate) => bitrate,
                _ => 0,
            };
            if requested_bitrate > 160 && in_sample_rate <= 24_000 {
                log_debug(&format!(
                    "FFmpeg MP3: upsampling from {} Hz to 48000 Hz to honor {} kbps target",
                    in_sample_rate, requested_bitrate
                ));
                48_000
            } else {
                in_sample_rate
            }
        }
        _ => in_sample_rate,
    };

    let codec_id = match settings.format {
        ConvertAudioFormat::Mp3 => AVCodecID_AV_CODEC_ID_MP3,
        ConvertAudioFormat::Aac => AVCodecID_AV_CODEC_ID_AAC,
        ConvertAudioFormat::Opus => AVCodecID_AV_CODEC_ID_OPUS,
        ConvertAudioFormat::Ogg => AVCodecID_AV_CODEC_ID_VORBIS,
        ConvertAudioFormat::Flac => AVCodecID_AV_CODEC_ID_FLAC,
        ConvertAudioFormat::Wav | ConvertAudioFormat::Aiff => AVCodecID_AV_CODEC_ID_PCM_S16LE,
    };

    let out_path_c = CString::new(output_path.to_string_lossy().as_bytes())
        .map_err(|_| "FFmpeg: invalid output path".to_string())?;
    let mut out_ctx: *mut AVFormatContext = ptr::null_mut();
    let alloc_ret = crate::ffmpeg_source::avformat_alloc_output_context2_safe(
        api,
        &mut out_ctx,
        ptr::null_mut(),
        ptr::null(),
        out_path_c.as_ptr(),
    );
    if alloc_ret < 0 || out_ctx.is_null() {
        return Err("FFmpeg: failed to allocate output context".to_string());
    }

    let codec = if matches!(settings.format, ConvertAudioFormat::Mp3) {
        let name =
            CString::new("libmp3lame").map_err(|_| "FFmpeg: invalid encoder name".to_string())?;
        let by_name = crate::ffmpeg_source::avcodec_find_encoder_by_name_safe(api, name.as_ptr());
        if by_name.is_null() {
            crate::ffmpeg_source::avcodec_find_encoder_safe(api, codec_id)
        } else {
            by_name
        }
    } else {
        crate::ffmpeg_source::avcodec_find_encoder_safe(api, codec_id)
    };
    if codec.is_null() {
        crate::ffmpeg_source::avformat_free_context_safe(api, out_ctx);
        return Err("FFmpeg: encoder not found".to_string());
    }
    let stream = crate::ffmpeg_source::avformat_new_stream_safe(api, out_ctx, codec);
    if stream.is_null() {
        crate::ffmpeg_source::avformat_free_context_safe(api, out_ctx);
        return Err("FFmpeg: failed to create output stream".to_string());
    }

    let mut codec_ctx = crate::ffmpeg_source::avcodec_alloc_context3_safe(api, codec);
    if codec_ctx.is_null() {
        crate::ffmpeg_source::avformat_free_context_safe(api, out_ctx);
        return Err("FFmpeg: failed to allocate encoder context".to_string());
    }

    let preferred_fmt = match settings.format {
        ConvertAudioFormat::Wav | ConvertAudioFormat::Aiff => {
            Some(AVSampleFormat_AV_SAMPLE_FMT_S16)
        }
        _ => None,
    };
    let enc_fmt = pick_encoder_sample_fmt_with_preference(codec, preferred_fmt);
    unsafe {
        (*codec_ctx).sample_rate = out_sample_rate as i32;
        (*codec_ctx).sample_fmt = enc_fmt;
        (*codec_ctx).time_base = AVRational {
            num: 1,
            den: out_sample_rate as i32,
        };
        let mut out_layout: AVChannelLayout = std::mem::zeroed();
        (api.av_channel_layout_default)(&mut out_layout, out_channels as i32);
        (*codec_ctx).ch_layout = out_layout;
    }

    match (settings.format, settings.quality) {
        (ConvertAudioFormat::Mp3, ConvertAudioQuality::BitrateKbps(bitrate)) => unsafe {
            let target = (bitrate as i64).saturating_mul(1000);
            (*codec_ctx).bit_rate = target;
            (*codec_ctx).rc_min_rate = target;
            (*codec_ctx).rc_max_rate = target;
            (*codec_ctx).bit_rate_tolerance = 0;
        },
        (
            ConvertAudioFormat::Aac | ConvertAudioFormat::Opus,
            ConvertAudioQuality::BitrateKbps(bitrate),
        ) => unsafe {
            (*codec_ctx).bit_rate = (bitrate as i64).saturating_mul(1000);
        },
        (ConvertAudioFormat::Ogg, ConvertAudioQuality::OggQuality(q)) => unsafe {
            (*codec_ctx).flags |= AV_CODEC_FLAG_QSCALE_FALLBACK;
            (*codec_ctx).global_quality = (q as i32).saturating_mul(FF_QP2LAMBDA_FALLBACK);
        },
        (ConvertAudioFormat::Flac, ConvertAudioQuality::FlacCompression(level)) => unsafe {
            (*codec_ctx).compression_level = level as i32;
        },
        _ => {}
    }

    let open_ret = crate::ffmpeg_source::avcodec_open2_safe(api, codec_ctx, codec, ptr::null_mut());
    if open_ret < 0 {
        let ffmpeg_error = ffmpeg_error_text(api, open_ret);
        log_debug(&format!(
            "FFmpeg: failed to open encoder format={:?} sample_rate={} channels={} bitrate={:?}: {}",
            settings.format, out_sample_rate, out_channels, settings.quality, ffmpeg_error
        ));
        unsafe {
            (api.avcodec_free_context)(&mut codec_ctx);
            (api.avformat_free_context)(out_ctx);
        }
        return Err(format!("FFmpeg: failed to open encoder: {ffmpeg_error}"));
    }

    unsafe {
        (*stream).time_base = (*codec_ctx).time_base;
        let par_ret = (api.avcodec_parameters_from_context)((*stream).codecpar, codec_ctx);
        if par_ret < 0 {
            (api.avcodec_free_context)(&mut codec_ctx);
            (api.avformat_free_context)(out_ctx);
            return Err("FFmpeg: failed to copy encoder parameters".to_string());
        }
    }

    let mut io: *mut AVIOContext = ptr::null_mut();
    let open_io =
        crate::ffmpeg_source::avio_open_safe(api, &mut io, out_path_c.as_ptr(), AVIO_FLAG_WRITE);
    if open_io < 0 {
        unsafe {
            (api.avcodec_free_context)(&mut codec_ctx);
            (api.avformat_free_context)(out_ctx);
        }
        return Err("FFmpeg: failed to open output file".to_string());
    }
    unsafe {
        (*out_ctx).pb = io;
    }
    let header_ret =
        crate::ffmpeg_source::avformat_write_header_safe(api, out_ctx, ptr::null_mut());
    if header_ret < 0 {
        unsafe {
            (api.avio_closep)(&mut io);
            (api.avcodec_free_context)(&mut codec_ctx);
            (api.avformat_free_context)(out_ctx);
        }
        return Err("FFmpeg: failed to write output header".to_string());
    }

    let encoder_frame_size = crate::ffmpeg_source::av_codec_context_frame_size_safe(codec_ctx);
    let encoder_has_fixed_frame_size = encoder_frame_size > 0;
    let in_frame_size = if encoder_has_fixed_frame_size {
        encoder_frame_size as usize
    } else {
        1024usize
    };
    let mut in_layout: AVChannelLayout = crate::zeroed_safe();
    let mut out_layout: AVChannelLayout = crate::zeroed_safe();
    unsafe {
        (api.av_channel_layout_default)(&mut in_layout, in_channels as i32);
        (api.av_channel_layout_default)(&mut out_layout, out_channels as i32);
    }
    let mut swr_ctx: *mut SwrContext = ptr::null_mut();
    let swr_ret = unsafe {
        (api.swr_alloc_set_opts2)(
            &mut swr_ctx,
            &out_layout,
            enc_fmt,
            out_sample_rate as i32,
            &in_layout,
            AVSampleFormat_AV_SAMPLE_FMT_FLT,
            in_sample_rate as i32,
            0,
            ptr::null_mut(),
        )
    };
    if swr_ret < 0 || swr_ctx.is_null() {
        unsafe {
            (api.avio_closep)(&mut io);
            (api.avcodec_free_context)(&mut codec_ctx);
            (api.avformat_free_context)(out_ctx);
            (api.av_channel_layout_uninit)(&mut in_layout);
            (api.av_channel_layout_uninit)(&mut out_layout);
        }
        return Err("FFmpeg: failed to init resampler".to_string());
    }
    if crate::ffmpeg_source::swr_init_safe(api, swr_ctx) < 0 {
        unsafe {
            (api.swr_free)(&mut swr_ctx);
            (api.avio_closep)(&mut io);
            (api.avcodec_free_context)(&mut codec_ctx);
            (api.avformat_free_context)(out_ctx);
            (api.av_channel_layout_uninit)(&mut in_layout);
            (api.av_channel_layout_uninit)(&mut out_layout);
        }
        return Err("FFmpeg: failed to init resampler".to_string());
    }

    let mut frame = crate::ffmpeg_source::av_frame_alloc_safe(api);
    if frame.is_null() {
        unsafe {
            (api.swr_free)(&mut swr_ctx);
            (api.avio_closep)(&mut io);
            (api.avcodec_free_context)(&mut codec_ctx);
            (api.avformat_free_context)(out_ctx);
            (api.av_channel_layout_uninit)(&mut in_layout);
            (api.av_channel_layout_uninit)(&mut out_layout);
        }
        return Err("FFmpeg: failed to allocate frame".to_string());
    }
    let mut out_capacity = if encoder_has_fixed_frame_size {
        encoder_frame_size
    } else {
        crate::ffmpeg_source::swr_get_out_samples_safe(api, swr_ctx, in_frame_size as i32)
    };
    if out_capacity <= 0 {
        out_capacity = in_frame_size as i32;
    }
    unsafe {
        (*frame).nb_samples = out_capacity;
        (*frame).format = enc_fmt;
        (*frame).sample_rate = out_sample_rate as i32;
        let mut frame_layout: AVChannelLayout = std::mem::zeroed();
        (api.av_channel_layout_default)(&mut frame_layout, out_channels as i32);
        (*frame).ch_layout = frame_layout;
        if (api.av_frame_get_buffer)(frame, 0) < 0 {
            (api.av_frame_free)(&mut frame);
            (api.swr_free)(&mut swr_ctx);
            (api.avio_closep)(&mut io);
            (api.avcodec_free_context)(&mut codec_ctx);
            (api.avformat_free_context)(out_ctx);
            (api.av_channel_layout_uninit)(&mut in_layout);
            (api.av_channel_layout_uninit)(&mut out_layout);
            return Err("FFmpeg: failed to allocate frame buffer".to_string());
        }
    }

    let in_channel_count = in_channels as usize;
    let mut next_pts: i64 = 0;
    let mut canceled = false;
    let mut processed_frames: u64 = 0;
    let mut last_pct: u32 = 0;
    let mut last_pts_us: i64 = 0;
    if let Some(cb) = progress.as_mut() {
        cb(0);
    }
    let is_canceled = |flag: &Option<Arc<AtomicBool>>| -> bool {
        flag.as_ref()
            .map(|cancel| cancel.load(Ordering::Relaxed))
            .unwrap_or(false)
    };
    let drain_packets = || -> Result<(), String> {
        loop {
            let mut pkt = crate::ffmpeg_source::av_packet_alloc_safe(api);
            if pkt.is_null() {
                return Err("FFmpeg: packet alloc failed".to_string());
            }
            let recv = crate::ffmpeg_source::avcodec_receive_packet_safe(api, codec_ctx, pkt);
            if recv == 0 {
                unsafe {
                    (*pkt).stream_index = (*stream).index;
                    crate::ffmpeg_source::av_packet_rescale_ts_safe(
                        api,
                        pkt,
                        (*codec_ctx).time_base,
                        (*stream).time_base,
                    );
                    let wret =
                        crate::ffmpeg_source::av_interleaved_write_frame_safe(api, out_ctx, pkt);
                    crate::ffmpeg_source::av_packet_unref_safe(api, pkt);
                    crate::ffmpeg_source::av_packet_free_safe(api, &mut pkt);
                    if wret < 0 {
                        return Err("FFmpeg: write audio packet failed".to_string());
                    }
                }
            } else {
                crate::ffmpeg_source::av_packet_free_safe(api, &mut pkt);
                break;
            }
        }
        Ok(())
    };

    loop {
        if is_canceled(&cancel) {
            canceled = true;
            break;
        }
        let mut input_frame = Vec::with_capacity(in_frame_size * in_channel_count);
        while input_frame.len() < in_frame_size * in_channel_count {
            if is_canceled(&cancel) {
                canceled = true;
                break;
            }
            match source.next() {
                Some(sample) => input_frame.push(sample),
                None => break,
            }
        }
        if canceled {
            break;
        }
        if input_frame.is_empty() {
            break;
        }
        let input_samples = (input_frame.len() / in_channel_count) as i32;
        processed_frames =
            processed_frames.saturating_add((input_frame.len() / in_channel_count) as u64);
        if let (Some(total_us), Some(cb)) = (total_duration_us, progress.as_mut())
            && total_us > 0
        {
            let mut pts_us = pts_clock.load(Ordering::Acquire);
            if pts_us < 0 {
                pts_us = 0;
            }
            if pts_us < last_pts_us {
                pts_us = last_pts_us;
            } else {
                last_pts_us = pts_us;
            }
            let pct = ((pts_us as u128 * 10000) / total_us as u128).min(10000) as u32;
            if pct > last_pct {
                last_pct = pct;
                cb(pct);
            }
        } else if let (Some(total), Some(cb)) = (total_frames, progress.as_mut())
            && total > 0
        {
            let pct = ((processed_frames.saturating_mul(10000)) / total).min(10000) as u32;
            if pct > last_pct {
                last_pct = pct;
                cb(pct);
            }
        }

        let needed_out =
            crate::ffmpeg_source::swr_get_out_samples_safe(api, swr_ctx, input_samples);
        if !encoder_has_fixed_frame_size && needed_out > 0 {
            let current_cap = crate::ffmpeg_source::av_frame_nb_samples_safe(frame);
            if needed_out > current_cap {
                unsafe {
                    (*frame).nb_samples = needed_out;
                    if (api.av_frame_get_buffer)(frame, 0) < 0 {
                        (api.swr_free)(&mut swr_ctx);
                        (api.av_frame_free)(&mut frame);
                        (api.avio_closep)(&mut io);
                        (api.avcodec_free_context)(&mut codec_ctx);
                        (api.avformat_free_context)(out_ctx);
                        (api.av_channel_layout_uninit)(&mut in_layout);
                        (api.av_channel_layout_uninit)(&mut out_layout);
                        return Err("FFmpeg: failed to grow frame buffer".to_string());
                    }
                }
            }
        }

        unsafe {
            if (api.av_frame_make_writable)(frame) < 0 {
                return Err("FFmpeg: frame not writable".to_string());
            }
            if encoder_has_fixed_frame_size {
                (*frame).nb_samples = encoder_frame_size;
            }
        }
        let mut in_ptr = input_frame.as_ptr() as *const u8;
        let in_ptrs = &mut in_ptr as *mut *const u8;
        let out_count = crate::ffmpeg_source::av_frame_nb_samples_safe(frame);
        let converted = crate::ffmpeg_source::swr_convert_safe(
            api,
            swr_ctx,
            crate::ffmpeg_source::av_frame_data_mut_ptr_safe(frame),
            out_count,
            in_ptrs,
            input_samples,
        );
        if converted < 0 {
            unsafe {
                (api.swr_free)(&mut swr_ctx);
                (api.av_frame_free)(&mut frame);
                (api.avio_closep)(&mut io);
                (api.avcodec_free_context)(&mut codec_ctx);
                (api.avformat_free_context)(out_ctx);
                (api.av_channel_layout_uninit)(&mut in_layout);
                (api.av_channel_layout_uninit)(&mut out_layout);
            }
            return Err("FFmpeg: resample failed".to_string());
        }
        if converted == 0 {
            continue;
        }
        unsafe {
            (*frame).nb_samples = converted;
            (*frame).pts = next_pts;
        }
        next_pts += converted as i64;

        let send_ret = crate::ffmpeg_source::avcodec_send_frame_safe(api, codec_ctx, frame);
        if send_ret < 0 {
            unsafe {
                (api.swr_free)(&mut swr_ctx);
                (api.av_frame_free)(&mut frame);
                (api.avio_closep)(&mut io);
                (api.avcodec_free_context)(&mut codec_ctx);
                (api.avformat_free_context)(out_ctx);
                (api.av_channel_layout_uninit)(&mut in_layout);
                (api.av_channel_layout_uninit)(&mut out_layout);
            }
            return Err(format!(
                "FFmpeg: send_frame failed: {} ({})",
                ffmpeg_error_text(api, send_ret),
                send_ret
            ));
        }
        if let Err(err) = drain_packets() {
            unsafe {
                (api.swr_free)(&mut swr_ctx);
                (api.av_frame_free)(&mut frame);
                (api.avio_closep)(&mut io);
                (api.avcodec_free_context)(&mut codec_ctx);
                (api.avformat_free_context)(out_ctx);
                (api.av_channel_layout_uninit)(&mut in_layout);
                (api.av_channel_layout_uninit)(&mut out_layout);
            }
            return Err(err);
        }
    }

    if !canceled {
        loop {
            if is_canceled(&cancel) {
                canceled = true;
                break;
            }
            let pending = crate::ffmpeg_source::swr_get_out_samples_safe(api, swr_ctx, 0);
            if pending <= 0 {
                break;
            }
            let current_cap = crate::ffmpeg_source::av_frame_nb_samples_safe(frame);
            if !encoder_has_fixed_frame_size && pending > current_cap {
                unsafe {
                    (*frame).nb_samples = pending;
                    if (api.av_frame_get_buffer)(frame, 0) < 0 {
                        (api.swr_free)(&mut swr_ctx);
                        (api.av_frame_free)(&mut frame);
                        (api.avio_closep)(&mut io);
                        (api.avcodec_free_context)(&mut codec_ctx);
                        (api.avformat_free_context)(out_ctx);
                        (api.av_channel_layout_uninit)(&mut in_layout);
                        (api.av_channel_layout_uninit)(&mut out_layout);
                        return Err("FFmpeg: failed to grow frame buffer".to_string());
                    }
                }
            }
            unsafe {
                if (api.av_frame_make_writable)(frame) < 0 {
                    return Err("FFmpeg: frame not writable".to_string());
                }
                if encoder_has_fixed_frame_size {
                    (*frame).nb_samples = encoder_frame_size;
                }
            }
            let out_count = crate::ffmpeg_source::av_frame_nb_samples_safe(frame);
            let converted = crate::ffmpeg_source::swr_convert_safe(
                api,
                swr_ctx,
                crate::ffmpeg_source::av_frame_data_mut_ptr_safe(frame),
                out_count,
                ptr::null(),
                0,
            );
            if converted <= 0 {
                break;
            }
            unsafe {
                (*frame).nb_samples = converted;
                (*frame).pts = next_pts;
            }
            next_pts += converted as i64;
            let send_ret = crate::ffmpeg_source::avcodec_send_frame_safe(api, codec_ctx, frame);
            if send_ret < 0 {
                unsafe {
                    (api.swr_free)(&mut swr_ctx);
                    (api.av_frame_free)(&mut frame);
                    (api.avio_closep)(&mut io);
                    (api.avcodec_free_context)(&mut codec_ctx);
                    (api.avformat_free_context)(out_ctx);
                    (api.av_channel_layout_uninit)(&mut in_layout);
                    (api.av_channel_layout_uninit)(&mut out_layout);
                }
                return Err(format!(
                    "FFmpeg: send_frame failed: {} ({})",
                    ffmpeg_error_text(api, send_ret),
                    send_ret
                ));
            }
            if let Err(err) = drain_packets() {
                unsafe {
                    (api.swr_free)(&mut swr_ctx);
                    (api.av_frame_free)(&mut frame);
                    (api.avio_closep)(&mut io);
                    (api.avcodec_free_context)(&mut codec_ctx);
                    (api.avformat_free_context)(out_ctx);
                    (api.av_channel_layout_uninit)(&mut in_layout);
                    (api.av_channel_layout_uninit)(&mut out_layout);
                }
                return Err(err);
            }
        }
    }

    if canceled && !graceful_stop {
        unsafe {
            (api.swr_free)(&mut swr_ctx);
            (api.av_frame_free)(&mut frame);
            (api.avio_closep)(&mut io);
            (api.avcodec_free_context)(&mut codec_ctx);
            (api.avformat_free_context)(out_ctx);
            (api.av_channel_layout_uninit)(&mut in_layout);
            (api.av_channel_layout_uninit)(&mut out_layout);
        }
        return Err("Conversion canceled.".to_string());
    }

    unsafe {
        (api.avcodec_send_frame)(codec_ctx, ptr::null());
        loop {
            let mut pkt = (api.av_packet_alloc)();
            if pkt.is_null() {
                break;
            }
            let recv = (api.avcodec_receive_packet)(codec_ctx, pkt);
            if recv == 0 {
                (*pkt).stream_index = (*stream).index;
                crate::ffmpeg_source::av_packet_rescale_ts_safe(
                    api,
                    pkt,
                    (*codec_ctx).time_base,
                    (*stream).time_base,
                );
                crate::ffmpeg_source::av_interleaved_write_frame_safe(api, out_ctx, pkt);
                crate::ffmpeg_source::av_packet_unref_safe(api, pkt);
                crate::ffmpeg_source::av_packet_free_safe(api, &mut pkt);
            } else {
                crate::ffmpeg_source::av_packet_free_safe(api, &mut pkt);
                break;
            }
        }
        let trailer_ret = crate::ffmpeg_source::av_write_trailer_safe(api, out_ctx);
        if trailer_ret < 0 {
            log_debug(&format!("FFmpeg: av_write_trailer failed: {}", trailer_ret));
        }
        (api.swr_free)(&mut swr_ctx);
        (api.av_frame_free)(&mut frame);
        (api.avio_closep)(&mut io);
        (api.avcodec_free_context)(&mut codec_ctx);
        (api.avformat_free_context)(out_ctx);
        (api.av_channel_layout_uninit)(&mut in_layout);
        (api.av_channel_layout_uninit)(&mut out_layout);
    }

    if let Some(cb) = progress.as_mut() {
        cb(10000);
    }

    Ok(())
}

#[cfg(test)]
mod convert_tests {
    use super::*;
    use std::path::PathBuf;
    use std::process::Command;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn test_audio_description_wav_padding_is_noop_for_complete_frames() {
        assert_eq!(audio_description_wav_padding_samples(0, 6), 0);
        assert_eq!(audio_description_wav_padding_samples(6, 6), 0);
        assert_eq!(audio_description_wav_padding_samples(12_000, 6), 0);
        assert_eq!(audio_description_wav_padding_samples(4_000, 2), 0);
    }

    #[test]
    fn test_audio_description_wav_padding_only_completes_final_frame() {
        assert_eq!(audio_description_wav_padding_samples(12_001, 6), 5);
        assert_eq!(audio_description_wav_padding_samples(12_002, 6), 4);
        assert_eq!(audio_description_wav_padding_samples(12_005, 6), 1);
        assert_eq!(audio_description_wav_padding_samples(4_001, 2), 1);
        assert_eq!(audio_description_wav_padding_samples(99, 1), 0);
    }

    #[test]
    fn test_validate_mp3_bitrate() {
        assert!(validate_mp3_bitrate(63).is_err());
        assert_eq!(validate_mp3_bitrate(64).unwrap(), 64);
        assert_eq!(validate_mp3_bitrate(320).unwrap(), 320);
        assert!(validate_mp3_bitrate(321).is_err());
    }

    #[test]
    fn test_mp3_channel_policy_keeps_mono_stereo_and_downmixes_multichannel() {
        assert_eq!(encoder_output_channels(ConvertAudioFormat::Mp3, 1), 1);
        assert_eq!(encoder_output_channels(ConvertAudioFormat::Mp3, 2), 2);
        assert_eq!(encoder_output_channels(ConvertAudioFormat::Mp3, 6), 2);
        assert_eq!(encoder_output_channels(ConvertAudioFormat::Mp3, 8), 2);
        assert_eq!(encoder_output_channels(ConvertAudioFormat::Aac, 6), 6);
    }

    #[test]
    fn test_build_ffmpeg_args_mp3() {
        let settings = ConvertAudioSettings {
            format: ConvertAudioFormat::Mp3,
            quality: ConvertAudioQuality::BitrateKbps(192),
        };
        let args = build_ffmpeg_args(&settings);
        assert_eq!(
            args,
            vec![
                "-c:a".to_string(),
                "libmp3lame".to_string(),
                "-b:a".to_string(),
                "192k".to_string(),
            ]
        );
    }

    fn ffprobe_duration(path: &std::path::Path) -> Result<f64, String> {
        let output = Command::new("ffprobe")
            .args([
                "-v",
                "error",
                "-show_entries",
                "format=duration",
                "-of",
                "default=nw=1:nk=1",
            ])
            .arg(path)
            .output()
            .map_err(|e| format!("ffprobe launch failed: {}", e))?;
        if !output.status.success() {
            return Err(format!(
                "ffprobe failed (code {:?}): {}",
                output.status.code(),
                String::from_utf8_lossy(&output.stderr)
            ));
        }
        let s = String::from_utf8_lossy(&output.stdout).trim().to_string();
        s.parse::<f64>()
            .map_err(|e| format!("ffprobe parse duration failed ({}): {}", s, e))
    }

    #[test]
    fn test_mp3_to_wav_to_mp3_duration_guard() {
        let input = std::env::var("SONARPAD_FFMPEG_DURATION_INPUT").unwrap_or_else(|_| {
            r"C:\Users\ambro\Documents\Sonarpad Audiobooks\ravagliani.edge_source.tmp.mp3"
                .to_string()
        });
        let input_path = PathBuf::from(&input);
        if !input_path.exists() {
            eprintln!("Skipping duration guard test: input not found: {}", input);
            return;
        }

        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0);
        let out_wav = std::env::temp_dir().join(format!("sonarpad_dur_guard_{}.wav", stamp));
        let out_mp3 = std::env::temp_dir().join(format!("sonarpad_dur_guard_{}.mp3", stamp));

        let wav_settings = ConvertAudioSettings {
            format: ConvertAudioFormat::Wav,
            quality: ConvertAudioQuality::None,
        };
        convert_audio_file_with_channels(&input_path, &out_wav, &wav_settings, None, None, Some(2))
            .map_err(|e| {
                if let Err(remove_err) = std::fs::remove_file(&out_wav) {
                    eprintln!("Cleanup failed (wav): {}", remove_err);
                }
                if let Err(remove_err) = std::fs::remove_file(&out_mp3) {
                    eprintln!("Cleanup failed (mp3): {}", remove_err);
                }
                e
            })
            .unwrap();

        let mp3_settings = ConvertAudioSettings {
            format: ConvertAudioFormat::Mp3,
            quality: ConvertAudioQuality::BitrateKbps(128),
        };
        convert_audio_file(&out_wav, &out_mp3, &mp3_settings, None, None)
            .map_err(|e| {
                if let Err(remove_err) = std::fs::remove_file(&out_wav) {
                    eprintln!("Cleanup failed (wav): {}", remove_err);
                }
                if let Err(remove_err) = std::fs::remove_file(&out_mp3) {
                    eprintln!("Cleanup failed (mp3): {}", remove_err);
                }
                e
            })
            .unwrap();

        let wav_dur = ffprobe_duration(&out_wav).unwrap();
        let mp3_dur = ffprobe_duration(&out_mp3).unwrap();

        if let Err(remove_err) = std::fs::remove_file(&out_wav) {
            eprintln!("Cleanup failed (wav): {}", remove_err);
        }
        if let Err(remove_err) = std::fs::remove_file(&out_mp3) {
            eprintln!("Cleanup failed (mp3): {}", remove_err);
        }

        // Guard: final MP3 must preserve WAV duration (allow small mux/encoder drift).
        let drift = (wav_dur - mp3_dur).abs();
        assert!(
            drift <= 2.0,
            "Duration loss too high: wav={}s mp3={}s drift={}s",
            wav_dur,
            mp3_dur,
            drift
        );
    }
}
