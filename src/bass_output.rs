use crate::accessibility::to_wide;
use crate::bass_ffmpeg_stream::FfmpegBassStream;
use crate::bass_sys::{
    BASS_ATTRIB_TEMPO, BASS_ATTRIB_TEMPO_PITCH, BASS_ATTRIB_VOL, BASS_FX_FREESOURCE, BASS_POS_BYTE,
    BASS_SAMPLE_FLOAT, BASS_STREAM_DECODE, BASS_STREAM_PRESCAN, BASS_UNICODE, BassApi, Dword,
    Hplugin, Hstream, Qword, bass_api, bass_error, bass_fx_api, log_bass_error,
};
use crate::embedded_deps;
use crate::log_debug;
use std::ffi::c_void;
use std::path::Path;
use std::ptr;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

pub struct BassOutput {
    api: &'static BassApi,
    handle: Mutex<Hstream>,
    /// Keep FFmpeg stream alive while playing
    _ffmpeg_stream: Option<FfmpegBassStream>,
    /// Offset in seconds for FFmpeg streams (seek position)
    start_offset_secs: f64,
    diagnostic_started: Instant,
    diagnostic_last: Mutex<Option<(Instant, Dword)>>,
}

static BASS_INIT: OnceLock<Result<(), String>> = OnceLock::new();
static BASS_PLUGINS: OnceLock<Result<Vec<Hplugin>, String>> = OnceLock::new();

fn bass_stream_free_safe(api: &BassApi, handle: Hstream) -> i32 {
    unsafe { (api.stream_free)(handle) }
}

fn bass_channel_set_attribute_safe(
    api: &BassApi,
    handle: Hstream,
    attrib: Dword,
    value: f32,
) -> i32 {
    unsafe { (api.channel_set_attribute)(handle, attrib, value) }
}

fn bass_channel_play_safe(api: &BassApi, handle: Hstream, restart: i32) -> i32 {
    unsafe { (api.channel_play)(handle, restart) }
}

fn bass_start_safe(api: &BassApi) -> i32 {
    unsafe { (api.start)() }
}

fn bass_channel_pause_safe(api: &BassApi, handle: Hstream) -> i32 {
    unsafe { (api.channel_pause)(handle) }
}

fn bass_channel_stop_safe(api: &BassApi, handle: Hstream) -> i32 {
    unsafe { (api.channel_stop)(handle) }
}

fn bass_channel_get_position_safe(api: &BassApi, handle: Hstream, mode: Dword) -> Qword {
    unsafe { (api.channel_get_position)(handle, mode) }
}

fn bass_channel_get_length_safe(api: &BassApi, handle: Hstream, mode: Dword) -> Qword {
    unsafe { (api.channel_get_length)(handle, mode) }
}

fn bass_channel_set_position_safe(api: &BassApi, handle: Hstream, pos: Qword, mode: Dword) -> i32 {
    unsafe { (api.channel_set_position)(handle, pos, mode) }
}

fn bass_channel_bytes2seconds_safe(api: &BassApi, handle: Hstream, pos: Qword) -> f64 {
    unsafe { (api.channel_bytes2seconds)(handle, pos) }
}

fn bass_channel_seconds2bytes_safe(api: &BassApi, handle: Hstream, pos: f64) -> Qword {
    unsafe { (api.channel_seconds2bytes)(handle, pos) }
}

fn bass_channel_is_active_safe(api: &BassApi, handle: Hstream) -> Dword {
    unsafe { (api.channel_is_active)(handle) }
}

fn init_bass_once() -> Result<(), String> {
    BASS_INIT
        .get_or_init(|| {
            let api = bass_api()?;
            let init_ok = unsafe { (api.init)(-1, 44_100, 0, ptr::null_mut(), ptr::null()) };
            if init_ok == 0 {
                return Err(format!("BASS_Init failed (error {})", bass_error(api)));
            }
            Ok(())
        })
        .clone()
}

fn play_channel(api: &BassApi, handle: Hstream, restart: i32, context: &str) -> Result<(), String> {
    const BASS_ERROR_START: i32 = 9;

    let play_ok = bass_channel_play_safe(api, handle, restart);
    if play_ok != 0 {
        return Ok(());
    }

    let err = bass_error(api);
    log_debug(&format!("BASS: {} failed (error {})", context, err));
    if err != BASS_ERROR_START {
        return Err(format!("{context} failed (error {err})"));
    }

    let start_ok = bass_start_safe(api);
    if start_ok == 0 {
        log_bass_error(api, "BASS_Start");
        return Err(format!("{context} failed (error {err})"));
    }

    let retry_ok = bass_channel_play_safe(api, handle, restart);
    if retry_ok == 0 {
        log_bass_error(api, &format!("{context} retry"));
        return Err(format!("{context} failed after BASS_Start retry"));
    }

    log_debug(&format!("BASS: {} recovered after BASS_Start", context));
    Ok(())
}

fn load_plugins_once(api: &BassApi) -> Result<Vec<Hplugin>, String> {
    BASS_PLUGINS
        .get_or_init(|| {
            let plugin_names = [
                "bass_aac.dll",
                "bass_alac.dll",
                "bassflac.dll",
                "bassopus.dll",
                "basswebm.dll",
                "basswma.dll",
            ];
            let mut handles = Vec::new();
            for name in plugin_names {
                let path = embedded_deps::get_dep_path(name);
                if !path.exists() {
                    log_debug(&format!("BASS: plugin missing: {}", path.display()));
                    continue;
                }
                let wide = to_wide(path.to_string_lossy().as_ref());
                let handle =
                    unsafe { (api.plugin_load)(wide.as_ptr() as *const c_void, BASS_UNICODE) };
                if handle == 0 {
                    log_bass_error(api, &format!("PluginLoad {}", name));
                } else {
                    handles.push(handle);
                }
            }
            Ok(handles)
        })
        .clone()
}

fn create_stream_from_path(api: &BassApi, path: &Path, flags: Dword) -> Result<Hstream, String> {
    let wide = to_wide(path.to_string_lossy().as_ref());
    let handle = unsafe {
        (api.stream_create_file)(
            0,
            wide.as_ptr() as *const c_void,
            0,
            0,
            flags | BASS_UNICODE,
        )
    };
    if handle == 0 {
        return Err(format!(
            "BASS_StreamCreateFile failed (error {})",
            bass_error(api)
        ));
    }
    Ok(handle)
}

impl BassOutput {
    pub fn start(
        path: &Path,
        start_seconds: u64,
        speed: f32,
        pitch: f32,
        volume: f32,
        paused: bool,
    ) -> Result<Arc<Self>, String> {
        Self::start_at(path, start_seconds as f64, speed, pitch, volume, paused)
    }

    pub fn start_at(
        path: &Path,
        start_seconds: f64,
        speed: f32,
        pitch: f32,
        volume: f32,
        paused: bool,
    ) -> Result<Arc<Self>, String> {
        init_bass_once()?;
        let api = bass_api()?;
        let fx_api = bass_fx_api().ok();
        if let Err(err) = load_plugins_once(api) {
            log_debug(&format!("BASS: plugin load failed: {}", err));
        }

        log_debug(&format!(
            "BASS diagnostic: open path={} start={start_seconds:.3}s speed={speed} pitch={pitch} volume={volume} paused={paused}",
            path.display()
        ));
        let want_tempo = (speed != 1.0 || pitch != 0.0) && fx_api.is_some();
        let mut flags = BASS_STREAM_PRESCAN | BASS_SAMPLE_FLOAT;
        if want_tempo {
            flags |= BASS_STREAM_DECODE;
        }

        let mut source = create_stream_from_path(api, path, flags)?;
        let handle = if want_tempo {
            if let Some(fx_api) = fx_api {
                let tempo_handle = unsafe {
                    (fx_api.tempo_create)(source, BASS_FX_FREESOURCE | BASS_SAMPLE_FLOAT)
                };
                if tempo_handle == 0 {
                    log_bass_error(api, "BASS_FX_TempoCreate");
                    let free_ok = bass_stream_free_safe(api, source);
                    if free_ok == 0 {
                        log_bass_error(api, "BASS_StreamFree");
                    }
                    source = create_stream_from_path(api, path, BASS_STREAM_PRESCAN)?;
                    source
                } else {
                    let tempo = ((speed as f64 - 1.0) * 100.0) as f32;
                    let tempo = tempo.clamp(-95.0, 5000.0);
                    let set_ok = bass_channel_set_attribute_safe(
                        api,
                        tempo_handle,
                        BASS_ATTRIB_TEMPO,
                        tempo,
                    );
                    if set_ok == 0 {
                        log_bass_error(api, "BASS_ChannelSetAttribute tempo");
                    }
                    let pitch_clamped = pitch.clamp(-60.0, 60.0);
                    let set_pitch_ok = unsafe {
                        (api.channel_set_attribute)(
                            tempo_handle,
                            BASS_ATTRIB_TEMPO_PITCH,
                            pitch_clamped,
                        )
                    };
                    if set_pitch_ok == 0 {
                        log_bass_error(api, "BASS_ChannelSetAttribute pitch");
                    }
                    tempo_handle
                }
            } else {
                source
            }
        } else {
            if speed != 1.0 {
                log_debug("BASS: bass_fx unavailable, forcing speed=1.0.");
            }
            source
        };

        log_debug(&format!(
            "BASS diagnostic: opened handle={handle} path={}",
            path.display()
        ));
        let volume = volume.clamp(0.0, 3.0);
        let set_ok = bass_channel_set_attribute_safe(api, handle, BASS_ATTRIB_VOL, volume);
        if set_ok == 0 {
            log_bass_error(api, "BASS_ChannelSetAttribute volume");
        }

        if start_seconds > 0.0 {
            let pos = bass_channel_seconds2bytes_safe(api, handle, start_seconds);
            let seek_ok = bass_channel_set_position_safe(api, handle, pos, BASS_POS_BYTE);
            if seek_ok == 0 {
                log_bass_error(api, "BASS_ChannelSetPosition");
                let free_ok = bass_stream_free_safe(api, handle);
                if free_ok == 0 {
                    log_bass_error(api, "BASS_StreamFree (seek-fail)");
                }
                return Err(format!(
                    "BASS initial seek failed (error {})",
                    bass_error(api)
                ));
            }
        }

        if !paused && let Err(err) = play_channel(api, handle, 0, "BASS_ChannelPlay") {
            let free_ok = bass_stream_free_safe(api, handle);
            if free_ok == 0 {
                log_bass_error(api, "BASS_StreamFree (play-fail)");
            }
            return Err(err);
        }

        Ok(Arc::new(Self {
            api,
            handle: Mutex::new(handle),
            _ffmpeg_stream: None,
            start_offset_secs: 0.0,
            diagnostic_started: Instant::now(),
            diagnostic_last: Mutex::new(None),
        }))
    }

    /// Start FFmpeg-backed playback at a precise fractional-second position.
    pub fn start_with_ffmpeg_at(
        path: &Path,
        start_seconds: f64,
        speed: f32,
        pitch: f32,
        volume: f32,
        paused: bool,
        stream_index: Option<i32>,
    ) -> Result<Arc<Self>, String> {
        init_bass_once()?;
        let api = bass_api()?;
        let fx_api = bass_fx_api().ok();

        let want_tempo = (speed != 1.0 || pitch != 0.0) && fx_api.is_some();
        // Create FFmpeg stream with BASS callback.
        // Use decode-only source only when we are going to wrap with BASS_FX tempo.
        let (ffmpeg_stream, source_handle) =
            FfmpegBassStream::new(path, start_seconds, stream_index, want_tempo)?;
        let handle = if want_tempo {
            if let Some(fx_api) = fx_api {
                let tempo_handle = unsafe {
                    (fx_api.tempo_create)(source_handle, BASS_FX_FREESOURCE | BASS_SAMPLE_FLOAT)
                };
                if tempo_handle == 0 {
                    log_bass_error(api, "BASS_FX_TempoCreate (ffmpeg)");
                    source_handle
                } else {
                    let tempo = ((speed as f64 - 1.0) * 100.0) as f32;
                    let tempo = tempo.clamp(-95.0, 5000.0);
                    let set_ok = bass_channel_set_attribute_safe(
                        api,
                        tempo_handle,
                        BASS_ATTRIB_TEMPO,
                        tempo,
                    );
                    if set_ok == 0 {
                        log_bass_error(api, "BASS_ChannelSetAttribute tempo (ffmpeg)");
                    }
                    let pitch_clamped = pitch.clamp(-60.0, 60.0);
                    let set_pitch_ok = unsafe {
                        (api.channel_set_attribute)(
                            tempo_handle,
                            BASS_ATTRIB_TEMPO_PITCH,
                            pitch_clamped,
                        )
                    };
                    if set_pitch_ok == 0 {
                        log_bass_error(api, "BASS_ChannelSetAttribute pitch (ffmpeg)");
                    }
                    tempo_handle
                }
            } else {
                source_handle
            }
        } else {
            if speed != 1.0 {
                log_debug("BASS: bass_fx unavailable, forcing speed=1.0.");
            }
            source_handle
        };

        log_debug(&format!(
            "BASS diagnostic: opened handle={handle} path={}",
            path.display()
        ));
        let volume = volume.clamp(0.0, 3.0);
        let set_ok = bass_channel_set_attribute_safe(api, handle, BASS_ATTRIB_VOL, volume);
        if set_ok == 0 {
            log_bass_error(api, "BASS_ChannelSetAttribute volume (ffmpeg)");
        }

        if !paused && let Err(err) = play_channel(api, handle, 0, "BASS_ChannelPlay (ffmpeg)") {
            let free_ok = bass_stream_free_safe(api, handle);
            if free_ok == 0 {
                log_bass_error(api, "BASS_StreamFree (ffmpeg play-fail)");
            }
            return Err(err);
        }

        log_debug("BASS: FFmpeg streaming started");
        Ok(Arc::new(Self {
            api,
            handle: Mutex::new(handle),
            _ffmpeg_stream: Some(ffmpeg_stream),
            start_offset_secs: start_seconds,
            diagnostic_started: Instant::now(),
            diagnostic_last: Mutex::new(None),
        }))
    }

    // Read-only snapshots, throttled to five seconds unless the channel state changes.
    fn log_playback_snapshot(&self, reason: &str, force: bool) {
        let handle = *self.handle.lock().unwrap_or_else(|e| e.into_inner());
        let state = bass_channel_is_active_safe(self.api, handle);
        let state_error = bass_error(self.api);
        let now = Instant::now();
        let mut last = self
            .diagnostic_last
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        if !force
            && last.is_some_and(|(time, previous)| {
                previous == state && now.duration_since(time) < Duration::from_secs(5)
            })
        {
            return;
        }
        *last = Some((now, state));
        let pos = bass_channel_get_position_safe(self.api, handle, BASS_POS_BYTE);
        let pos_error = bass_error(self.api);
        let len = bass_channel_get_length_safe(self.api, handle, BASS_POS_BYTE);
        let len_error = bass_error(self.api);
        let seconds = if pos == u64::MAX {
            None
        } else {
            Some(bass_channel_bytes2seconds_safe(self.api, handle, pos))
        };
        log_debug(&format!(
            "BASS diagnostic: {reason} handle={handle} backend={} elapsed={:.3}s state={state} state_error={state_error} pos_bytes={pos} pos_error={pos_error} pos_secs={seconds:?} length_bytes={len} length_error={len_error} offset={:.3}s",
            if self._ffmpeg_stream.is_some() {
                "ffmpeg"
            } else {
                "direct"
            },
            self.diagnostic_started.elapsed().as_secs_f64(),
            self.start_offset_secs
        ));
    }

    pub fn play(&self) -> bool {
        let handle = *self.handle.lock().unwrap_or_else(|e| e.into_inner());
        let result = play_channel(self.api, handle, 0, "BASS_ChannelPlay").is_ok();
        self.log_playback_snapshot("play", true);
        result
    }

    pub fn pause(&self) -> bool {
        let handle = *self.handle.lock().unwrap_or_else(|e| e.into_inner());
        let ok = bass_channel_pause_safe(self.api, handle);
        if ok == 0 {
            log_bass_error(self.api, "BASS_ChannelPause");
            return false;
        }
        true
    }

    pub fn stop(&self) {
        self.log_playback_snapshot("stop requested", true);
        let handle = *self.handle.lock().unwrap_or_else(|e| e.into_inner());
        let ok = bass_channel_stop_safe(self.api, handle);
        if ok == 0 {
            log_bass_error(self.api, "BASS_ChannelStop");
        }
        let free_ok = bass_stream_free_safe(self.api, handle);
        if free_ok == 0 {
            log_bass_error(self.api, "BASS_StreamFree");
        }
    }

    pub fn set_volume(&self, volume: f32) {
        let handle = *self.handle.lock().unwrap_or_else(|e| e.into_inner());
        let volume = volume.clamp(0.0, 3.0);
        let ok = bass_channel_set_attribute_safe(self.api, handle, BASS_ATTRIB_VOL, volume);
        if ok == 0 {
            log_bass_error(self.api, "BASS_ChannelSetAttribute volume");
        }
    }

    pub fn position_secs(&self) -> Option<f64> {
        let handle = *self.handle.lock().unwrap_or_else(|e| e.into_inner());
        let pos = bass_channel_get_position_safe(self.api, handle, BASS_POS_BYTE);
        if pos == 0 {
            let err = bass_error(self.api);
            if err != 0 {
                log_debug(&format!("BASS: position failed (error {})", err));
                return None;
            }
        }
        let bass_pos = bass_channel_bytes2seconds_safe(self.api, handle, pos).max(0.0);
        self.log_playback_snapshot("position poll", false);
        // Add start offset for FFmpeg streams that were seeked
        Some(bass_pos + self.start_offset_secs)
    }

    pub fn duration_secs(&self) -> Option<f64> {
        if let Some(ffmpeg_stream) = self._ffmpeg_stream.as_ref()
            && let Some(total_secs) = ffmpeg_stream.total_duration_secs()
        {
            return Some(total_secs.max(0.0));
        }

        let handle = *self.handle.lock().unwrap_or_else(|e| e.into_inner());
        let len = bass_channel_get_length_safe(self.api, handle, BASS_POS_BYTE);
        if len == u64::MAX {
            return None;
        }
        if len == 0 {
            let err = bass_error(self.api);
            if err != 0 {
                log_debug(&format!("BASS: duration failed (error {})", err));
                return None;
            }
        }
        let bass_len = bass_channel_bytes2seconds_safe(self.api, handle, len).max(0.0);
        // Add start offset for FFmpeg streams started from a non-zero position.
        Some(bass_len + self.start_offset_secs)
    }

    pub fn seek_to_seconds(&self, absolute_seconds: f64) -> bool {
        log_debug(&format!(
            "BASS diagnostic: seek requested target={absolute_seconds:.3}s"
        ));
        self.log_playback_snapshot("before seek", true);
        // FFmpeg streaming starts from `start_offset_secs`. If caller asks to seek
        // before this offset, we must fail here so the caller can reopen at the
        // real absolute position.
        if self.start_offset_secs > 0.0 && absolute_seconds < self.start_offset_secs {
            return false;
        }
        let handle = *self.handle.lock().unwrap_or_else(|e| e.into_inner());
        if self._ffmpeg_stream.is_some() {
            // For FFmpeg-backed streams (not directly seekable file handles),
            // backward seeks via BASS can report success but not really rewind.
            // Force reopen path for backward seeks to guarantee accurate behavior.
            let pos_now = bass_channel_get_position_safe(self.api, handle, BASS_POS_BYTE);
            let now_rel = bass_channel_bytes2seconds_safe(self.api, handle, pos_now).max(0.0);
            let now_abs = now_rel + self.start_offset_secs;
            if absolute_seconds + 0.05 < now_abs {
                return false;
            }
        }
        let relative = (absolute_seconds - self.start_offset_secs).max(0.0);
        let pos = bass_channel_seconds2bytes_safe(self.api, handle, relative);
        let ok = bass_channel_set_position_safe(self.api, handle, pos, BASS_POS_BYTE);
        if ok == 0 {
            log_bass_error(self.api, "BASS_ChannelSetPosition");
            self.log_playback_snapshot("seek failed", true);
            return false;
        }
        true
    }

    pub fn is_stopped(&self) -> bool {
        const BASS_ACTIVE_STOPPED: Dword = 0;
        let handle = *self.handle.lock().unwrap_or_else(|e| e.into_inner());
        let state = bass_channel_is_active_safe(self.api, handle);
        self.log_playback_snapshot("state poll", false);
        state == BASS_ACTIVE_STOPPED
    }

    pub fn clear_subtitles(&self) {
        // Edge subtitle audio is played through the TTS engine path (rodio).
    }
}
