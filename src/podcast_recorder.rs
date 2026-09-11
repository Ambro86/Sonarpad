use crate::audio_utils;
use crate::com_guard::ComGuard;
use crate::mf_encoder;
use crate::settings;
use crate::settings::{PODCAST_DEVICE_DEFAULT, PodcastFormat};
use chrono::Local;
#[path = "podcast_timeline.rs"]
mod timeline;
use std::ffi::OsString;
use std::mem::ManuallyDrop;
use std::mem::size_of;
use std::os::windows::ffi::OsStringExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};
use timeline::{TICKS_PER_SECOND, TimedQueue, Timeline, frame_ticks};
use windows::Win32::Devices::FunctionDiscovery::PKEY_Device_FriendlyName;
use windows::Win32::Foundation::CloseHandle;
use windows::Win32::Media::Audio::{
    AUDCLNT_BUFFERFLAGS_DATA_DISCONTINUITY, AUDCLNT_BUFFERFLAGS_SILENT,
    AUDCLNT_BUFFERFLAGS_TIMESTAMP_ERROR, AUDCLNT_SHAREMODE_SHARED,
    AUDCLNT_STREAMFLAGS_AUTOCONVERTPCM, AUDCLNT_STREAMFLAGS_LOOPBACK,
    AUDIOCLIENT_ACTIVATION_PARAMS, AUDIOCLIENT_ACTIVATION_PARAMS_0,
    AUDIOCLIENT_ACTIVATION_TYPE_PROCESS_LOOPBACK, AUDIOCLIENT_PROCESS_LOOPBACK_PARAMS,
    ActivateAudioInterfaceAsync, AudioSessionStateActive, DEVICE_STATE_ACTIVE, EDataFlow,
    IActivateAudioInterfaceAsyncOperation, IActivateAudioInterfaceCompletionHandler,
    IAudioCaptureClient, IAudioClient, IAudioSessionControl, IAudioSessionControl2,
    IAudioSessionEnumerator, IAudioSessionManager2, IMMDevice, IMMDeviceCollection,
    IMMDeviceEnumerator, MMDeviceEnumerator, PROCESS_LOOPBACK_MODE_INCLUDE_TARGET_PROCESS_TREE,
    VIRTUAL_AUDIO_DEVICE_PROCESS_LOOPBACK, WAVEFORMATEX, eCapture, eConsole, eRender,
};
use windows::Win32::Media::KernelStreaming::WAVE_FORMAT_EXTENSIBLE;
use windows::Win32::Media::Multimedia::{KSDATAFORMAT_SUBTYPE_IEEE_FLOAT, WAVE_FORMAT_IEEE_FLOAT};
use windows::Win32::System::Com::StructuredStorage::PropVariantToStringAlloc;
use windows::Win32::System::Com::{CLSCTX_ALL, CoCreateInstance, CoTaskMemFree, STGM_READ};
use windows::Win32::System::Power::{ES_CONTINUOUS, ES_SYSTEM_REQUIRED, SetThreadExecutionState};
use windows::Win32::System::Threading::{
    OpenProcess, PROCESS_NAME_FORMAT, PROCESS_QUERY_LIMITED_INFORMATION, QueryFullProcessImageNameW,
};
use windows::Win32::System::Variant::VT_BLOB;
use windows::Win32::UI::Shell::PropertiesSystem::IPropertyStore;
use windows::core::{GUID, HRESULT, Interface, PCWSTR, PROPVARIANT, PWSTR, implement};

const TARGET_SAMPLE_RATE: u32 = 44100;
const TARGET_CHANNELS: u16 = 2;
const TARGET_BITS: u16 = 16;
const MIX_CHUNK_FRAMES: usize = 512;
const WAVE_FORMAT_PCM_TAG: u32 = 0x0001;
const KSDATAFORMAT_SUBTYPE_PCM: GUID = GUID::from_u128(0x00000001_0000_0010_8000_00AA00389B71);

#[derive(Clone)]
pub struct AudioDevice {
    pub id: String,
    pub name: String,
}

#[derive(Clone)]
pub struct AudioApp {
    pub pid: u32,
    pub display_name: String,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum SampleFormat {
    I16,
    I24,
    I32,
    F32,
}

impl SampleFormat {
    fn name(self) -> &'static str {
        match self {
            Self::I16 => "i16",
            Self::I24 => "i24",
            Self::I32 => "i32",
            Self::F32 => "f32",
        }
    }
}

struct DeviceEnumerator {
    _init: ComGuard,
    inner: IMMDeviceEnumerator,
}

impl DeviceEnumerator {
    fn new() -> Result<Self, String> {
        let init = ComGuard::new_mta().map_err(|e| format!("CoInitializeEx failed: {e}"))?;
        let inner: IMMDeviceEnumerator = unsafe {
            CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)
                .map_err(|e| format!("MMDeviceEnumerator failed: {e}"))?
        };
        Ok(Self { _init: init, inner })
    }
}

pub fn list_input_devices() -> Result<Vec<AudioDevice>, String> {
    list_devices(eCapture)
}

pub fn list_output_devices() -> Result<Vec<AudioDevice>, String> {
    list_devices(eRender)
}

pub fn list_audio_apps(include_inactive: bool) -> Result<Vec<AudioApp>, String> {
    let _com = ComGuard::new_mta().map_err(|e| format!("CoInitializeEx failed: {e}"))?;
    let enumerator: IMMDeviceEnumerator = unsafe {
        CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)
            .map_err(|e| format!("MMDeviceEnumerator failed: {e}"))?
    };
    let device = unsafe {
        enumerator
            .GetDefaultAudioEndpoint(eRender, eConsole)
            .map_err(|e| format!("GetDefaultAudioEndpoint(render) failed: {e}"))?
    };
    let session_manager: IAudioSessionManager2 = unsafe {
        device
            .Activate(CLSCTX_ALL, None)
            .map_err(|e| format!("IAudioSessionManager2 activate failed: {e}"))?
    };
    let sessions: IAudioSessionEnumerator = unsafe {
        session_manager
            .GetSessionEnumerator()
            .map_err(|e| format!("GetSessionEnumerator failed: {e}"))?
    };
    let count = unsafe {
        sessions
            .GetCount()
            .map_err(|e| format!("GetCount failed: {e}"))?
    };

    let mut apps = Vec::new();
    for index in 0..count {
        let control: IAudioSessionControl = unsafe {
            sessions
                .GetSession(index)
                .map_err(|e| format!("GetSession({index}) failed: {e}"))?
        };
        let state = unsafe {
            control
                .GetState()
                .map_err(|e| format!("GetState({index}) failed: {e}"))?
        };
        if !include_inactive && state != AudioSessionStateActive {
            continue;
        }
        let session_control2: IAudioSessionControl2 = control
            .cast()
            .map_err(|e| format!("IAudioSessionControl2 cast failed: {e}"))?;
        let pid = unsafe {
            session_control2
                .GetProcessId()
                .map_err(|e| format!("GetProcessId({index}) failed: {e}"))?
        };
        if pid == 0 || pid == std::process::id() {
            continue;
        }
        let display_name = process_display_name(pid);
        if !display_name.is_empty() {
            apps.push(AudioApp { pid, display_name });
        }
    }
    apps.sort_by(|a, b| {
        a.display_name
            .to_lowercase()
            .cmp(&b.display_name.to_lowercase())
    });
    apps.dedup_by(|a, b| a.pid == b.pid);
    Ok(apps)
}

pub fn probe_device_with_name(
    device_id: &str,
    device_name: &str,
    loopback: bool,
) -> Result<(), String> {
    let _com = ComGuard::new_mta().map_err(|e| format!("CoInitializeEx failed: {e}"))?;
    let device = resolve_device_with_name(device_id, device_name, loopback)?;
    let client: IAudioClient = unsafe {
        device
            .Activate(CLSCTX_ALL, None)
            .map_err(|e| format!("AudioClient activate failed: {e}"))?
    };
    let mix_format = unsafe {
        client
            .GetMixFormat()
            .map_err(|e| format!("GetMixFormat failed: {e}"))?
    };
    let mut stream_flags = 0;
    if loopback {
        stream_flags |= AUDCLNT_STREAMFLAGS_LOOPBACK;
    }
    unsafe {
        client
            .Initialize(
                AUDCLNT_SHAREMODE_SHARED,
                stream_flags | AUDCLNT_STREAMFLAGS_AUTOCONVERTPCM,
                10_000_000,
                0,
                mix_format,
                None,
            )
            .map_err(|e| format!("AudioClient initialize failed: {e}"))?;
        CoTaskMemFree(Some(mix_format as *const _));
    }
    Ok(())
}

pub fn probe_process_loopback(process_id: u32) -> Result<(), String> {
    if process_id == 0 {
        return Err("Invalid target process id.".to_string());
    }
    crate::log_debug(&format!(
        "Process loopback probe: preparing audio client for PID {}",
        process_id
    ));
    let _com = ComGuard::new_mta().map_err(|e| format!("CoInitializeEx failed: {e}"))?;
    let client = activate_process_loopback_client(process_id)?;
    let wave_format = process_loopback_wave_format();
    crate::log_debug(&format!(
        "Process loopback probe: initializing shared client for PID {}",
        process_id
    ));
    unsafe {
        client
            .Initialize(
                AUDCLNT_SHAREMODE_SHARED,
                AUDCLNT_STREAMFLAGS_LOOPBACK | AUDCLNT_STREAMFLAGS_AUTOCONVERTPCM,
                0,
                0,
                &wave_format,
                None,
            )
            .map_err(|e| format!("AudioClient initialize failed: {e}"))?;
    }
    Ok(())
}

fn list_devices(flow: EDataFlow) -> Result<Vec<AudioDevice>, String> {
    let enumerator = DeviceEnumerator::new()?;
    let collection: IMMDeviceCollection = unsafe {
        enumerator
            .inner
            .EnumAudioEndpoints(flow, DEVICE_STATE_ACTIVE)
            .map_err(|e| format!("EnumAudioEndpoints failed: {e}"))?
    };
    let count = unsafe {
        collection
            .GetCount()
            .map_err(|e| format!("GetCount failed: {e}"))?
    };
    let mut devices = Vec::new();
    for index in 0..count {
        let device: IMMDevice = unsafe {
            collection
                .Item(index)
                .map_err(|e| format!("Device Item failed: {e}"))?
        };
        if let Some(info) = device_info(&device) {
            devices.push(info);
        }
    }
    Ok(devices)
}

fn device_id(device: &IMMDevice) -> Option<String> {
    unsafe {
        let id = device.GetId().ok()?;
        if id.is_null() {
            return None;
        }
        let value = id.to_string().unwrap_or_default();
        CoTaskMemFree(Some(id.0 as *const _));
        if value.is_empty() { None } else { Some(value) }
    }
}

fn device_info(device: &IMMDevice) -> Option<AudioDevice> {
    let id = device_id(device)?;
    let name = device_friendly_name(device).unwrap_or_else(|| id.clone());
    Some(AudioDevice { id, name })
}

fn device_friendly_name(device: &IMMDevice) -> Option<String> {
    unsafe {
        let store: IPropertyStore = device.OpenPropertyStore(STGM_READ).ok()?;
        let value = store.GetValue(&PKEY_Device_FriendlyName).ok()?;
        let name_ptr = PropVariantToStringAlloc(&value).ok()?;
        if name_ptr.is_null() {
            return None;
        }
        let name = name_ptr.to_string().unwrap_or_default();
        CoTaskMemFree(Some(name_ptr.0 as *const _));
        if name.is_empty() { None } else { Some(name) }
    }
}

#[derive(Clone)]
pub struct RecorderConfig {
    pub include_mic: bool,
    pub mic_device_id: String,
    pub mic_device_name: String,
    pub mic_gain: f32,
    pub include_system: bool,
    pub split_mic_system: bool,
    pub system_device_id: String,
    pub system_device_name: String,
    pub system_gain: f32,
    pub single_app_process_id: Option<u32>,
    pub selected_app_process_ids: Vec<u32>,
    pub output_format: PodcastFormat,
    pub mp3_bitrate: u32,
    pub save_folder: PathBuf,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum RecorderStatus {
    Idle,
    Recording,
    Paused,
    Saving,
    Error,
}

pub struct RecorderHandle {
    shared: Arc<SharedState>,
    buffer: Arc<MixBuffer>,
    stop: Arc<AtomicBool>,
    paused: Arc<AtomicBool>,
    single_app_process_id: Option<Arc<AtomicU32>>,
    threads: Vec<JoinHandle<Result<(), String>>>,
    output_path: PathBuf,
    secondary_output_path: Option<PathBuf>,
    temp_wav: PathBuf,
    temp_mp3: PathBuf,
    secondary_temp_wav: Option<PathBuf>,
    secondary_temp_mp3: Option<PathBuf>,
    format: PodcastFormat,
}

struct SharedState {
    status: Mutex<RecorderStatus>,
    last_error: Mutex<Option<String>>,
    started_at: Mutex<Option<Instant>>,
    paused_at: Mutex<Option<Instant>>,
    paused_total: Mutex<Duration>,
    mic_peak: AtomicU32,
    system_peak: AtomicU32,
    include_mic: bool,
    include_system: bool,
}

impl SharedState {
    fn new(include_mic: bool, include_system: bool) -> Self {
        SharedState {
            status: Mutex::new(RecorderStatus::Idle),
            last_error: Mutex::new(None),
            started_at: Mutex::new(None),
            paused_at: Mutex::new(None),
            paused_total: Mutex::new(Duration::ZERO),
            mic_peak: AtomicU32::new(0),
            system_peak: AtomicU32::new(0),
            include_mic,
            include_system,
        }
    }
}

pub struct LevelSnapshot {
    pub mic_peak: u32,
    pub system_peak: u32,
}

pub fn start_recording(config: RecorderConfig) -> Result<RecorderHandle, String> {
    if !config.include_mic && !config.include_system {
        return Err("No sources selected.".to_string());
    }

    let output_folder = if config.save_folder.as_os_str().is_empty() {
        PathBuf::from(settings::default_podcast_save_folder())
    } else {
        config.save_folder.clone()
    };
    if let Some(parent) = output_folder.parent() {
        crate::log_if_err!(std::fs::create_dir_all(parent));
    }
    crate::log_if_err!(std::fs::create_dir_all(&output_folder));

    let timestamp = Local::now().format("%Y-%m-%d_%H-%M-%S").to_string();
    let base_name = format!("Podcast_{timestamp}");

    let extension = match config.output_format {
        PodcastFormat::Mp3 => "mp3",
        PodcastFormat::Wav => "wav",
    };
    let split_mic_system = config.split_mic_system && config.include_mic && config.include_system;
    let (
        output_path,
        secondary_output_path,
        temp_wav,
        temp_mp3,
        secondary_temp_wav,
        secondary_temp_mp3,
    ) = if split_mic_system {
        (
            output_folder.join(format!("{base_name}_microphone.{extension}")),
            Some(output_folder.join(format!("{base_name}_system_audio.{extension}"))),
            output_folder.join(format!("{base_name}_microphone.wav.tmp")),
            output_folder.join(format!("{base_name}_microphone_tmp.mp3")),
            Some(output_folder.join(format!("{base_name}_system_audio.wav.tmp"))),
            Some(output_folder.join(format!("{base_name}_system_audio_tmp.mp3"))),
        )
    } else {
        (
            output_folder.join(format!("{base_name}.{extension}")),
            None,
            output_folder.join(format!("{base_name}.wav.tmp")),
            output_folder.join(format!("{base_name}_tmp.mp3")),
            None,
            None,
        )
    };
    crate::log_debug(&format!(
        "Podcast recorder output mode: split_sources={} primary={} secondary={}",
        split_mic_system,
        output_path.display(),
        secondary_output_path
            .as_ref()
            .map(|path| path.display().to_string())
            .unwrap_or_else(|| "none".to_string())
    ));

    // Audio-only path
    let shared = Arc::new(SharedState::new(config.include_mic, config.include_system));
    *shared.status.lock().unwrap_or_else(|e| e.into_inner()) = RecorderStatus::Recording;
    *shared.started_at.lock().unwrap_or_else(|e| e.into_inner()) = Some(Instant::now());

    let stop = Arc::new(AtomicBool::new(false));
    let paused = Arc::new(AtomicBool::new(false));
    let single_app_process_id =
        if config.include_system && config.selected_app_process_ids.is_empty() {
            config
                .single_app_process_id
                .filter(|pid| *pid != 0)
                .map(|pid| Arc::new(AtomicU32::new(pid)))
        } else {
            None
        };

    let system_stream_count = if config.include_system {
        let selected_count = config
            .selected_app_process_ids
            .iter()
            .filter(|pid| **pid != 0)
            .count();
        if selected_count > 0 {
            selected_count
        } else {
            1
        }
    } else {
        0
    };
    let mix_buffer = Arc::new(MixBuffer::new(system_stream_count)?);
    let mut threads = Vec::new();

    if config.include_mic {
        crate::log_debug("Starting microphone capture thread");
        let buffer = mix_buffer.clone();
        let shared_state = shared.clone();
        let stop_flag = stop.clone();
        let paused_flag = paused.clone();
        let device_id = config.mic_device_id.clone();
        let device_name = config.mic_device_name.clone();
        let mic_gain = config.mic_gain;
        let completion = CaptureCompletion::new(buffer.clone());
        threads.push(thread::spawn(move || {
            let _completion = completion;
            crate::log_debug("Microphone capture thread started");
            let result = capture_source(CaptureOptions {
                kind: SourceKind::Microphone,
                device_id,
                device_name,
                loopback: false,
                gain: mic_gain,
                target_process_id: None,
                dynamic_target_process_id: None,
                system_stream_index: 0,
                buffer,
                shared: shared_state.clone(),
                stop: stop_flag.clone(),
                paused: paused_flag,
            });
            if let Err(err) = &result {
                crate::log_debug(&format!("Microphone capture error: {}", err));
                if let Ok(mut error) = shared_state.last_error.lock() {
                    *error = Some(err.clone());
                }
                if let Ok(mut status) = shared_state.status.lock() {
                    *status = RecorderStatus::Error;
                }
                stop_flag.store(true, Ordering::SeqCst);
            } else {
                crate::log_debug("Microphone capture thread stopped normally");
            }
            result
        }));
    }

    if config.include_system {
        crate::log_debug("Starting system audio capture thread");
        let mut target_process_ids = config.selected_app_process_ids.clone();
        if target_process_ids.is_empty() {
            if let Some(pid) = single_app_process_id
                .as_ref()
                .map(|process_id| process_id.load(Ordering::SeqCst))
            {
                target_process_ids.push(pid);
            }
        } else {
            target_process_ids.sort_unstable();
            target_process_ids.dedup();
        }

        let capture_targets = if target_process_ids.is_empty() {
            vec![None]
        } else {
            target_process_ids.into_iter().map(Some).collect()
        };

        for (system_stream_index, target_process_id) in capture_targets.into_iter().enumerate() {
            let buffer = mix_buffer.clone();
            let shared_state = shared.clone();
            let stop_flag = stop.clone();
            let paused_flag = paused.clone();
            let device_id = config.system_device_id.clone();
            let device_name = config.system_device_name.clone();
            let system_gain = config.system_gain;
            let dynamic_target_process_id = if target_process_id.is_some() {
                single_app_process_id.clone()
            } else {
                None
            };
            let completion = CaptureCompletion::new(buffer.clone());
            threads.push(thread::spawn(move || {
                let _completion = completion;
                crate::log_debug("System audio capture thread started");
                let result = capture_source(CaptureOptions {
                    kind: SourceKind::System,
                    device_id,
                    device_name,
                    loopback: true,
                    gain: system_gain,
                    target_process_id,
                    dynamic_target_process_id,
                    system_stream_index,
                    buffer,
                    shared: shared_state.clone(),
                    stop: stop_flag.clone(),
                    paused: paused_flag,
                });
                if let Err(err) = &result {
                    crate::log_debug(&format!("System audio capture error: {}", err));
                    if let Ok(mut error) = shared_state.last_error.lock() {
                        *error = Some(err.clone());
                    }
                    if let Ok(mut status) = shared_state.status.lock() {
                        *status = RecorderStatus::Error;
                    }
                    stop_flag.store(true, Ordering::SeqCst);
                } else {
                    crate::log_debug("System audio capture thread stopped normally");
                }
                result
            }));
        }
    }

    let keep_awake_stop = stop.clone();
    threads.push(thread::spawn(move || keep_awake_loop(keep_awake_stop)));

    let writer_buffer = mix_buffer.clone();
    let writer_shared = shared.clone();
    let writer_stop = stop.clone();
    let writer_paused = paused.clone();
    let writer_bitrate = config.mp3_bitrate;
    let writer_format = config.output_format;
    let primary_writer_path = match writer_format {
        PodcastFormat::Mp3 => temp_mp3.clone(),
        PodcastFormat::Wav => temp_wav.clone(),
    };
    let secondary_writer_path = match writer_format {
        PodcastFormat::Mp3 => secondary_temp_mp3.clone(),
        PodcastFormat::Wav => secondary_temp_wav.clone(),
    };
    threads.push(thread::spawn(move || {
        let result = if let Some(system_path) = secondary_writer_path {
            write_split_audio(
                SplitWriterConfig {
                    mic_path: primary_writer_path,
                    system_path,
                    format: writer_format,
                    mp3_bitrate: writer_bitrate,
                },
                writer_buffer,
                writer_stop.clone(),
                writer_paused,
            )
        } else {
            write_mixed_audio(
                WriterConfig {
                    path: primary_writer_path,
                    format: writer_format,
                    mp3_bitrate: writer_bitrate,
                },
                writer_buffer,
                writer_shared.clone(),
                writer_stop.clone(),
                writer_paused,
            )
        };
        if let Err(err) = &result {
            if let Ok(mut error) = writer_shared.last_error.lock() {
                *error = Some(err.clone());
            }
            if let Ok(mut status) = writer_shared.status.lock() {
                *status = RecorderStatus::Error;
            }
            writer_stop.store(true, Ordering::SeqCst);
        }
        result
    }));

    Ok(RecorderHandle {
        shared,
        buffer: mix_buffer,
        stop,
        paused,
        single_app_process_id,
        threads,
        output_path,
        secondary_output_path,
        temp_wav,
        temp_mp3,
        secondary_temp_wav,
        secondary_temp_mp3,
        format: config.output_format,
    })
}

impl RecorderHandle {
    pub fn pause(&self) {
        if !self.paused.swap(true, Ordering::SeqCst) {
            self.buffer.pause();
            if let Ok(mut paused_at) = self.shared.paused_at.lock() {
                *paused_at = Some(Instant::now());
            }
            if let Ok(mut status) = self.shared.status.lock() {
                *status = RecorderStatus::Paused;
            }
        }
    }

    pub fn resume(&self) {
        if self.paused.swap(false, Ordering::SeqCst) {
            self.buffer.resume();
            let now = Instant::now();
            if let Ok(mut paused_at) = self.shared.paused_at.lock()
                && let Some(start) = paused_at.take()
                && let Ok(mut total) = self.shared.paused_total.lock()
            {
                *total += now.saturating_duration_since(start);
            }
            if let Ok(mut status) = self.shared.status.lock() {
                *status = RecorderStatus::Recording;
            }
        }
    }

    pub fn update_single_app_process(&self, process_id: u32) -> Result<(), String> {
        if process_id == 0 {
            return Err("Invalid target process id.".to_string());
        }
        let Some(target) = self.single_app_process_id.as_ref() else {
            return Err("Current recording is not using single-app capture.".to_string());
        };
        let previous = target.swap(process_id, Ordering::SeqCst);
        if previous != process_id {
            crate::log_debug(&format!(
                "Podcast recorder: switching single-app capture from PID {} to PID {}",
                previous, process_id
            ));
        }
        Ok(())
    }

    pub fn stop(self) -> Result<PathBuf, String> {
        self.stop_with_progress(|_| {}, None)
    }

    pub fn stop_with_progress<F>(
        mut self,
        mut progress: F,
        cancel: Option<Arc<AtomicBool>>,
    ) -> Result<PathBuf, String>
    where
        F: FnMut(u32),
    {
        crate::log_debug("Stopping podcast recording");

        // Freeze the timeline before draining capture and encoder queues.
        self.buffer.finish();
        self.stop.store(true, Ordering::SeqCst);
        crate::log_debug("Signaled encoder to stop");

        if let Ok(mut status) = self.shared.status.lock() {
            *status = RecorderStatus::Saving;
        }

        // Wait for all threads to finish
        let threads = std::mem::take(&mut self.threads);
        for handle in threads {
            match handle.join() {
                Ok(Ok(())) => {}
                Ok(Err(err)) => {
                    crate::log_debug(&format!("Thread error: {}", err));
                    self.set_error(&err);
                    return Err(err);
                }
                Err(_) => {
                    let err = "Recording thread panicked.".to_string();
                    crate::log_debug(&err);
                    self.set_error(&err);
                    return Err(err);
                }
            }
        }
        crate::log_debug("All threads stopped");

        if let Some(cancel) = cancel.as_ref()
            && cancel.load(Ordering::Relaxed)
        {
            self.remove_temporary_files();
            return Err("Saving canceled.".to_string());
        }

        let primary_temp = match self.format {
            PodcastFormat::Mp3 => &self.temp_mp3,
            PodcastFormat::Wav => &self.temp_wav,
        };
        let mut outputs = vec![(primary_temp, &self.output_path)];
        let secondary_temp = match self.format {
            PodcastFormat::Mp3 => self.secondary_temp_mp3.as_ref(),
            PodcastFormat::Wav => self.secondary_temp_wav.as_ref(),
        };
        if let (Some(temp), Some(output)) = (secondary_temp, self.secondary_output_path.as_ref()) {
            outputs.push((temp, output));
        }

        let output_count = outputs.len() as u32;
        for (index, (temp, output)) in outputs.into_iter().enumerate() {
            if let Some(cancel) = cancel.as_ref()
                && cancel.load(Ordering::Relaxed)
            {
                self.remove_temporary_files();
                return Err("Saving canceled.".to_string());
            }
            if let Err(err) = rename_atomic(temp, output) {
                crate::log_debug(&format!(
                    "Podcast final rename failed: source={} destination={} error={}",
                    temp.display(),
                    output.display(),
                    err
                ));
                self.set_error(&err);
                return Err(err);
            }
            progress((((index as u32) + 1) * 100) / output_count.max(1));
        }

        if let Ok(mut status) = self.shared.status.lock() {
            *status = RecorderStatus::Idle;
        }
        Ok(self.output_path.clone())
    }

    pub fn status(&self) -> RecorderStatus {
        self.shared
            .status
            .lock()
            .map(|status| *status)
            .unwrap_or(RecorderStatus::Error)
    }

    pub fn levels(&self) -> LevelSnapshot {
        LevelSnapshot {
            mic_peak: self.shared.mic_peak.load(Ordering::Relaxed),
            system_peak: self.shared.system_peak.load(Ordering::Relaxed),
        }
    }

    pub fn elapsed(&self) -> Duration {
        let start = self.shared.started_at.lock().ok().and_then(|s| *s);
        let start = match start {
            Some(value) => value,
            None => return Duration::ZERO,
        };
        let paused_total = self
            .shared
            .paused_total
            .lock()
            .map(|v| *v)
            .unwrap_or(Duration::ZERO);
        let paused_at = self.shared.paused_at.lock().ok().and_then(|s| *s);
        let now = Instant::now();
        let mut elapsed = now.saturating_duration_since(start);
        if let Some(paused_at) = paused_at {
            elapsed = paused_at.saturating_duration_since(start);
        }
        elapsed.saturating_sub(paused_total)
    }

    pub fn take_error(&self) -> Option<String> {
        self.shared.last_error.lock().ok()?.take()
    }

    fn remove_temporary_files(&self) {
        crate::log_if_err!(std::fs::remove_file(&self.temp_wav));
        crate::log_if_err!(std::fs::remove_file(&self.temp_mp3));
        if let Some(path) = self.secondary_temp_wav.as_ref() {
            crate::log_if_err!(std::fs::remove_file(path));
        }
        if let Some(path) = self.secondary_temp_mp3.as_ref() {
            crate::log_if_err!(std::fs::remove_file(path));
        }
    }

    fn set_error(&self, message: &str) {
        if let Ok(mut err) = self.shared.last_error.lock() {
            *err = Some(message.to_string());
        }
        if let Ok(mut status) = self.shared.status.lock() {
            *status = RecorderStatus::Error;
        }
    }
}

fn rename_atomic(src: &Path, dest: &Path) -> Result<(), String> {
    if dest.exists() {
        crate::log_if_err!(std::fs::remove_file(dest));
    }
    std::fs::rename(src, dest).map_err(|e| e.to_string())
}

fn keep_awake_loop(stop: Arc<AtomicBool>) -> Result<(), String> {
    const KEEP_AWAKE_REFRESH: Duration = Duration::from_secs(30);
    const KEEP_AWAKE_POLL: Duration = Duration::from_millis(200);

    unsafe {
        SetThreadExecutionState(ES_CONTINUOUS | ES_SYSTEM_REQUIRED);
    }
    let mut last_refresh = Instant::now();
    while !stop.load(Ordering::SeqCst) {
        if last_refresh.elapsed() >= KEEP_AWAKE_REFRESH {
            unsafe {
                SetThreadExecutionState(ES_CONTINUOUS | ES_SYSTEM_REQUIRED);
            }
            last_refresh = Instant::now();
        }
        thread::sleep(KEEP_AWAKE_POLL);
    }
    unsafe {
        SetThreadExecutionState(ES_CONTINUOUS);
    }
    Ok(())
}

#[derive(Clone, Copy)]
enum SourceKind {
    Microphone,
    System,
}

struct MixBuffer {
    inner: Mutex<MixQueues>,
    condvar: Condvar,
    origin_qpc: u64,
    origin: Instant,
    captures: AtomicUsize,
}

struct MixQueues {
    mic: TimedQueue,
    system: Vec<TimedQueue>,
    timeline: Timeline,
    cursor: u64,
}

// The writer waits for every capture worker, including one unwinding after a panic.
struct CaptureCompletion(Arc<MixBuffer>);
impl CaptureCompletion {
    fn new(buffer: Arc<MixBuffer>) -> Self {
        buffer.captures.fetch_add(1, Ordering::SeqCst);
        Self(buffer)
    }
}
impl Drop for CaptureCompletion {
    fn drop(&mut self) {
        self.0.captures.fetch_sub(1, Ordering::SeqCst);
        self.0.condvar.notify_one();
    }
}

impl MixBuffer {
    fn new(system_stream_count: usize) -> Result<Self, String> {
        use windows::Win32::System::Performance::{
            QueryPerformanceCounter, QueryPerformanceFrequency,
        };
        let mut frequency = 0;
        let mut counter = 0;
        unsafe { QueryPerformanceFrequency(&mut frequency) }.map_err(|e| e.to_string())?;
        unsafe { QueryPerformanceCounter(&mut counter) }.map_err(|e| e.to_string())?;
        if frequency <= 0 || counter < 0 {
            return Err("Invalid recording clock".to_string());
        }
        let origin = Instant::now();
        let origin_qpc =
            ((counter as u128 * u128::from(TICKS_PER_SECOND)) / frequency as u128) as u64;
        crate::log_debug(
            "Podcast synchronization: shared QPC timeline; delayed packets buffered up to 1000 ms; capture queues drained on stop",
        );
        Ok(Self {
            inner: Mutex::new(MixQueues {
                mic: TimedQueue::default(),
                system: (0..system_stream_count)
                    .map(|_| TimedQueue::default())
                    .collect(),
                timeline: Timeline::new(origin_qpc),
                cursor: 0,
            }),
            condvar: Condvar::new(),
            origin_qpc,
            origin,
            captures: AtomicUsize::new(0),
        })
    }

    fn now(&self) -> u64 {
        self.origin_qpc + (self.origin.elapsed().as_nanos() / 100) as u64
    }

    fn pause(&self) {
        self.inner
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .timeline
            .pause(self.now());
    }

    fn resume(&self) {
        self.inner
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .timeline
            .resume(self.now());
    }

    fn finish(&self) {
        let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        if inner.timeline.end.is_none() {
            inner.timeline.end = Some(self.now());
        }
        self.condvar.notify_one();
    }

    #[cfg(test)]
    fn push(&self, source: SourceKind, system_stream_index: usize, qpc: u64, samples: Vec<f32>) {
        self.push_capture(source, system_stream_index, qpc, samples, false);
    }

    fn push_capture(
        &self,
        source: SourceKind,
        system_stream_index: usize,
        qpc: u64,
        samples: Vec<f32>,
        continuous: bool,
    ) {
        let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        for (offset, count, start) in
            inner
                .timeline
                .segments(qpc, samples.len() / 2, TARGET_SAMPLE_RATE)
        {
            let queue = match source {
                SourceKind::Microphone => &mut inner.mic,
                SourceKind::System => {
                    let Some(queue) = inner.system.get_mut(system_stream_index) else {
                        continue;
                    };
                    queue
                }
            };
            queue.push_clocked(
                start,
                samples[offset * 2..(offset + count) * 2].to_vec(),
                continuous && offset == 0,
            );
        }
        self.condvar.notify_one();
    }

    fn next_chunk(&self, stop: &AtomicBool) -> Option<(Vec<f32>, Vec<f32>)> {
        let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        loop {
            let stopping = stop.load(Ordering::SeqCst);
            if stopping && inner.timeline.end.is_none() {
                inner.timeline.end = Some(self.now());
            }
            // Allow up to one second for delayed device delivery before filling a gap.
            let horizon = if stopping {
                inner.timeline.end.unwrap_or_else(|| self.now())
            } else {
                self.now().saturating_sub(TICKS_PER_SECOND)
            };
            let total = inner.timeline.elapsed_frames(horizon, TARGET_SAMPLE_RATE);
            let remaining = total.saturating_sub(inner.cursor);
            let drained = stopping && self.captures.load(Ordering::SeqCst) == 0;
            if drained && remaining == 0 {
                crate::log_debug(&format!(
                    "Podcast packet continuity: mic_gap_frames={} mic_overlap_frames={} mic_max_gap={} mic_corrected={} system_gap_overlap_max_corrected={:?}",
                    inner.mic.packet_gap_frames,
                    inner.mic.packet_overlap_frames,
                    inner.mic.max_packet_gap,
                    inner.mic.corrected_boundaries,
                    inner
                        .system
                        .iter()
                        .map(|q| (
                            q.packet_gap_frames,
                            q.packet_overlap_frames,
                            q.max_packet_gap,
                            q.corrected_boundaries
                        ))
                        .collect::<Vec<_>>()
                ));
                crate::log_debug(&format!(
                    "Podcast sync: written_frames={} mic_missing_frames={} mic_late_frames={} system_missing_frames={:?} system_late_frames={:?}",
                    inner.cursor,
                    inner.mic.silent_frames,
                    inner.mic.late_frames,
                    inner
                        .system
                        .iter()
                        .map(|q| q.silent_frames)
                        .collect::<Vec<_>>(),
                    inner
                        .system
                        .iter()
                        .map(|q| q.late_frames)
                        .collect::<Vec<_>>()
                ));
                return None;
            }
            if (!stopping && remaining >= MIX_CHUNK_FRAMES as u64) || (drained && remaining > 0) {
                let frames = remaining.min(MIX_CHUNK_FRAMES as u64) as usize;
                let cursor = inner.cursor;
                let mic = inner.mic.read(cursor, frames);
                let mut system = vec![0.0; frames * 2];
                let streams = inner.system.len().max(1) as f32;
                for queue in &mut inner.system {
                    for (mixed, sample) in system.iter_mut().zip(queue.read(cursor, frames)) {
                        *mixed += sample / streams;
                    }
                }
                inner.cursor += frames as u64;
                return Some((mic, system));
            }
            inner = self
                .condvar
                .wait_timeout(inner, Duration::from_millis(20))
                .unwrap_or_else(|e| e.into_inner())
                .0;
        }
    }
}

fn mixed_chunk(mic: Vec<f32>, system: Vec<f32>, shared: &SharedState, streams: usize) -> Vec<f32> {
    let gain = if shared.include_mic && shared.include_system {
        0.5
    } else {
        1.0
    };
    mic.into_iter()
        .zip(system)
        .map(|(mic, system)| {
            // Preserve the existing multiple-application mixing gain.
            let mic = if shared.include_mic {
                mic / streams.max(1) as f32
            } else {
                0.0
            };
            let system = if shared.include_system { system } else { 0.0 };
            ((mic + system) * gain).clamp(-1.0, 1.0)
        })
        .collect()
}

struct WriterConfig {
    path: PathBuf,
    format: PodcastFormat,
    mp3_bitrate: u32,
}

fn write_mixed_audio(
    config: WriterConfig,
    buffer: Arc<MixBuffer>,
    shared: Arc<SharedState>,
    stop: Arc<AtomicBool>,
    paused: Arc<AtomicBool>,
) -> Result<(), String> {
    match config.format {
        PodcastFormat::Mp3 => write_mixed_audio_mp3(
            config.path,
            config.mp3_bitrate,
            buffer,
            shared,
            stop,
            paused,
        ),
        PodcastFormat::Wav => write_mixed_audio_wav(config.path, buffer, shared, stop, paused),
    }
}

fn write_mixed_audio_wav(
    path: PathBuf,
    buffer: Arc<MixBuffer>,
    shared: Arc<SharedState>,
    stop: Arc<AtomicBool>,
    _paused: Arc<AtomicBool>,
) -> Result<(), String> {
    let mut writer =
        audio_utils::WavWriter::create(&path, TARGET_SAMPLE_RATE, TARGET_CHANNELS, TARGET_BITS)
            .map_err(|e| e.to_string())?;

    while let Some((mic, system)) = buffer.next_chunk(&stop) {
        let streams = buffer
            .inner
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .system
            .len();
        let mixed = mixed_chunk(mic, system, &shared, streams);
        writer
            .write_samples_f32(&mixed)
            .map_err(|e| e.to_string())?;
    }
    writer.finalize().map_err(|e| e.to_string())?;
    Ok(())
}

fn write_mixed_audio_mp3(
    path: PathBuf,
    mp3_bitrate: u32,
    buffer: Arc<MixBuffer>,
    shared: Arc<SharedState>,
    stop: Arc<AtomicBool>,
    _paused: Arc<AtomicBool>,
) -> Result<(), String> {
    let mut writer = mf_encoder::Mp3StreamWriter::create(
        &path,
        mp3_bitrate,
        TARGET_SAMPLE_RATE,
        TARGET_CHANNELS,
    )?;
    while let Some((mic, system)) = buffer.next_chunk(&stop) {
        let streams = buffer
            .inner
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .system
            .len();
        let mixed = mixed_chunk(mic, system, &shared, streams);
        let pcm: Vec<i16> = mixed
            .into_iter()
            .map(|sample| (sample * i16::MAX as f32) as i16)
            .collect();
        writer.write_i16(&pcm)?;
    }
    writer.finalize()?;
    Ok(())
}

struct SplitWriterConfig {
    mic_path: PathBuf,
    system_path: PathBuf,
    format: PodcastFormat,
    mp3_bitrate: u32,
}

fn write_split_audio(
    config: SplitWriterConfig,
    buffer: Arc<MixBuffer>,
    stop: Arc<AtomicBool>,
    paused: Arc<AtomicBool>,
) -> Result<(), String> {
    match config.format {
        PodcastFormat::Mp3 => write_split_audio_mp3(
            config.mic_path,
            config.system_path,
            config.mp3_bitrate,
            buffer,
            stop,
            paused,
        ),
        PodcastFormat::Wav => {
            write_split_audio_wav(config.mic_path, config.system_path, buffer, stop, paused)
        }
    }
}

fn write_split_audio_wav(
    mic_path: PathBuf,
    system_path: PathBuf,
    buffer: Arc<MixBuffer>,
    stop: Arc<AtomicBool>,
    _paused: Arc<AtomicBool>,
) -> Result<(), String> {
    let mut mic_writer =
        audio_utils::WavWriter::create(&mic_path, TARGET_SAMPLE_RATE, TARGET_CHANNELS, TARGET_BITS)
            .map_err(|e| e.to_string())?;
    let mut system_writer = audio_utils::WavWriter::create(
        &system_path,
        TARGET_SAMPLE_RATE,
        TARGET_CHANNELS,
        TARGET_BITS,
    )
    .map_err(|e| e.to_string())?;

    while let Some((mic, system)) = buffer.next_chunk(&stop) {
        mic_writer
            .write_samples_f32(&mic)
            .map_err(|e| e.to_string())?;
        system_writer
            .write_samples_f32(&system)
            .map_err(|e| e.to_string())?;
    }
    mic_writer.finalize().map_err(|e| e.to_string())?;
    system_writer.finalize().map_err(|e| e.to_string())?;
    Ok(())
}

fn write_split_audio_mp3(
    mic_path: PathBuf,
    system_path: PathBuf,
    mp3_bitrate: u32,
    buffer: Arc<MixBuffer>,
    stop: Arc<AtomicBool>,
    _paused: Arc<AtomicBool>,
) -> Result<(), String> {
    let mut mic_writer = mf_encoder::Mp3StreamWriter::create(
        &mic_path,
        mp3_bitrate,
        TARGET_SAMPLE_RATE,
        TARGET_CHANNELS,
    )?;
    let mut system_writer = mf_encoder::Mp3StreamWriter::create(
        &system_path,
        mp3_bitrate,
        TARGET_SAMPLE_RATE,
        TARGET_CHANNELS,
    )?;

    while let Some((mic, system)) = buffer.next_chunk(&stop) {
        let mic_pcm: Vec<i16> = mic
            .into_iter()
            .map(|sample| (sample.clamp(-1.0, 1.0) * i16::MAX as f32) as i16)
            .collect();
        let system_pcm: Vec<i16> = system
            .into_iter()
            .map(|sample| (sample.clamp(-1.0, 1.0) * i16::MAX as f32) as i16)
            .collect();
        mic_writer.write_i16(&mic_pcm)?;
        system_writer.write_i16(&system_pcm)?;
    }
    mic_writer.finalize()?;
    system_writer.finalize()?;
    Ok(())
}

#[derive(Clone)]
struct CaptureOptions {
    kind: SourceKind,
    device_id: String,
    device_name: String,
    loopback: bool,
    gain: f32,
    target_process_id: Option<u32>,
    dynamic_target_process_id: Option<Arc<AtomicU32>>,
    system_stream_index: usize,
    buffer: Arc<MixBuffer>,
    shared: Arc<SharedState>,
    stop: Arc<AtomicBool>,
    paused: Arc<AtomicBool>,
}

fn capture_source(options: CaptureOptions) -> Result<(), String> {
    let Some(dynamic_target) = options.dynamic_target_process_id.clone() else {
        return capture_source_once(options);
    };
    loop {
        if options.stop.load(Ordering::SeqCst) {
            return Ok(());
        }
        let target_process_id = dynamic_target.load(Ordering::SeqCst);
        if target_process_id == 0 {
            return Err("Invalid target process id.".to_string());
        }
        let mut current_options = options.clone();
        current_options.target_process_id = Some(target_process_id);
        let result = capture_source_once(current_options);
        if options.stop.load(Ordering::SeqCst) {
            return result;
        }
        if dynamic_target.load(Ordering::SeqCst) != target_process_id {
            crate::log_debug(&format!(
                "Podcast recorder: restarting single-app capture after PID change from {}",
                target_process_id
            ));
            continue;
        }
        return result;
    }
}

fn capture_source_once(options: CaptureOptions) -> Result<(), String> {
    let _com = ComGuard::new_mta().map_err(|e| format!("CoInitializeEx failed: {e}"))?;
    crate::log_debug(&format!(
        "capture_source: kind={:?}, device_id='{}', name='{}', loopback={}",
        match options.kind {
            SourceKind::Microphone => "Microphone",
            SourceKind::System => "System",
        },
        options.device_id,
        options.device_name,
        options.loopback
    ));
    if let Some(target_process_id) = options.target_process_id {
        crate::log_debug(&format!(
            "capture_source: using process loopback for PID {}",
            target_process_id
        ));
    }
    let client: IAudioClient = if let Some(target_process_id) = options.target_process_id {
        activate_process_loopback_client(target_process_id)?
    } else {
        let device =
            resolve_device_with_name(&options.device_id, &options.device_name, options.loopback)?;
        if matches!(options.kind, SourceKind::Microphone) {
            crate::log_debug("Microphone capture: device resolved");
        }
        unsafe {
            device
                .Activate(CLSCTX_ALL, None)
                .map_err(|e| format!("AudioClient activate failed: {e}"))?
        }
    };

    let mut stream_flags = 0;
    if options.loopback {
        stream_flags |= AUDCLNT_STREAMFLAGS_LOOPBACK;
    }
    let (input_rate, input_channels, input_format) = if options.target_process_id.is_some() {
        let wave_format = process_loopback_wave_format();
        let parsed = parse_format(&wave_format)?;
        unsafe {
            client
                .Initialize(
                    AUDCLNT_SHAREMODE_SHARED,
                    stream_flags | AUDCLNT_STREAMFLAGS_AUTOCONVERTPCM,
                    0,
                    0,
                    &wave_format,
                    None,
                )
                .map_err(|e| format!("AudioClient initialize failed: {e}"))?;
        }
        parsed
    } else {
        let mix_format = unsafe {
            client
                .GetMixFormat()
                .map_err(|e| format!("GetMixFormat failed: {e}"))?
        };
        let parsed = match parse_mix_format_ptr(mix_format) {
            Ok(parsed) => parsed,
            Err(err) => {
                unsafe { CoTaskMemFree(Some(mix_format as *const _)) };
                return Err(err);
            }
        };
        let initialize_result = unsafe {
            client.Initialize(
                AUDCLNT_SHAREMODE_SHARED,
                stream_flags | AUDCLNT_STREAMFLAGS_AUTOCONVERTPCM,
                10_000_000,
                0,
                mix_format,
                None,
            )
        };
        unsafe { CoTaskMemFree(Some(mix_format as *const _)) };
        initialize_result.map_err(|e| format!("AudioClient initialize failed: {e}"))?;
        parsed
    };
    if matches!(options.kind, SourceKind::Microphone) {
        crate::log_debug("Microphone capture: client initialized");
    }
    if input_rate == 0 || input_channels == 0 {
        return Err("Invalid capture sample rate or channel count".to_string());
    }

    let capture: IAudioCaptureClient = unsafe {
        client
            .GetService()
            .map_err(|e| format!("GetService capture failed: {e}"))?
    };
    unsafe {
        client.Start().map_err(|e| format!("Start failed: {e}"))?;
    }
    if matches!(options.kind, SourceKind::Microphone) {
        crate::log_debug("Microphone capture: client started");
    }

    let mut resampler =
        LinearResampler::new(input_rate, TARGET_SAMPLE_RATE, input_channels as usize);
    let input_format_name = input_format.name();
    crate::log_debug(&format!(
        "capture_source format: kind={:?} pid={:?} stream_index={} input_rate={} input_channels={} input_format={} target_rate={} target_channels={}",
        match options.kind {
            SourceKind::Microphone => "Microphone",
            SourceKind::System => "System",
        },
        options.target_process_id,
        options.system_stream_index,
        input_rate,
        input_channels,
        input_format_name,
        TARGET_SAMPLE_RATE,
        TARGET_CHANNELS
    ));
    let mut expected_position: Option<u64> = None;
    let mut next_packet_qpc: Option<u64> = None;
    let mut gain_clipped_samples = 0u64;
    let mut discontinuities = 0u64;
    let mut timestamp_errors = 0u64;
    let mut packet_counter: u64 = 0;
    let mut total_input_frames: u64 = 0;
    let mut total_output_frames: u64 = 0;
    let mut last_packet_log = Instant::now();

    loop {
        let stopping = options.stop.load(Ordering::SeqCst);
        if stopping {
            unsafe { client.Stop() }.map_err(|e| format!("Stop capture failed: {e}"))?;
        }
        if let (Some(dynamic_target), Some(target_process_id)) = (
            options.dynamic_target_process_id.as_ref(),
            options.target_process_id,
        ) && dynamic_target.load(Ordering::SeqCst) != target_process_id
        {
            crate::log_debug(&format!(
                "capture_source: PID change detected for stream_index={}, old_pid={}",
                options.system_stream_index, target_process_id
            ));
            break;
        }

        let mut packet_len = unsafe {
            capture
                .GetNextPacketSize()
                .map_err(|e| format!("GetNextPacketSize failed: {e}"))?
        };
        while packet_len > 0 {
            let mut data_ptr: *mut u8 = std::ptr::null_mut();
            let mut frames = 0u32;
            let mut flags = 0u32;
            let mut device_position = 0u64;
            let mut packet_qpc = 0u64;
            unsafe {
                capture
                    .GetBuffer(
                        &mut data_ptr,
                        &mut frames,
                        &mut flags,
                        Some(&mut device_position),
                        Some(&mut packet_qpc),
                    )
                    .map_err(|e| format!("GetBuffer failed: {e}"))?;
            }
            let samples = if flags & (AUDCLNT_BUFFERFLAGS_SILENT.0 as u32) != 0 {
                vec![0f32; frames as usize * input_channels as usize]
            } else {
                read_samples(data_ptr, frames, input_channels, input_format)
            };
            unsafe {
                capture
                    .ReleaseBuffer(frames)
                    .map_err(|e| format!("ReleaseBuffer failed: {e}"))?;
            }

            if !options.paused.load(Ordering::SeqCst) {
                update_peak(&options.shared, &options.kind, &samples);
            }
            let discontinuity = expected_position
                .is_some_and(|expected| expected != device_position)
                || flags & AUDCLNT_BUFFERFLAGS_DATA_DISCONTINUITY.0 as u32 != 0;
            if discontinuity {
                discontinuities += 1;
                // Do not interpolate across an interval of missing input audio.
                resampler =
                    LinearResampler::new(input_rate, TARGET_SAMPLE_RATE, input_channels as usize);
            }
            expected_position = Some(device_position + u64::from(frames));
            let now = options.buffer.now();
            if flags & AUDCLNT_BUFFERFLAGS_TIMESTAMP_ERROR.0 as u32 != 0
                || packet_qpc == 0
                || packet_qpc > now + TICKS_PER_SECOND
            {
                timestamp_errors += 1;
                // A timestamp error also makes the device position untrustworthy.
                // Preserve continuity unless there was a gap; then use arrival time.
                packet_qpc = next_packet_qpc
                    .filter(|_| !discontinuity)
                    .unwrap_or_else(|| {
                        now.saturating_sub(frame_ticks(u64::from(frames), input_rate))
                    });
            }
            next_packet_qpc = Some(packet_qpc + frame_ticks(u64::from(frames), input_rate));
            // The interpolator may retain part of the preceding packet; timestamp its first output.
            let buffered_frames =
                resampler.buffer.len() as f64 / input_channels as f64 - resampler.pos;
            let buffered_ticks = (buffered_frames.max(0.0) * TICKS_PER_SECOND as f64
                / input_rate as f64)
                .round() as u64;
            let output_qpc = packet_qpc.saturating_sub(buffered_ticks);
            let resampled = resampler.push(&samples);
            let mut stereo = to_stereo(&resampled, input_channels as usize);
            packet_counter += 1;
            total_input_frames += frames as u64;
            total_output_frames += (stereo.len() / TARGET_CHANNELS as usize) as u64;
            if options.target_process_id.is_some()
                && (packet_counter == 1
                    || packet_counter.is_multiple_of(200)
                    || last_packet_log.elapsed() >= Duration::from_secs(5))
            {
                let output_frames = stereo.len() / TARGET_CHANNELS as usize;
                crate::log_debug(&format!(
                    "capture_source packet: pid={:?} stream_index={} packet={} frames_in={} samples_in={} frames_out={} flags=0x{:X} total_in_frames={} total_out_frames={} elapsed_ms={}",
                    options.target_process_id,
                    options.system_stream_index,
                    packet_counter,
                    frames,
                    samples.len(),
                    output_frames,
                    flags,
                    total_input_frames,
                    total_output_frames,
                    last_packet_log.elapsed().as_millis()
                ));
                last_packet_log = Instant::now();
            }

            // Apply gain
            if options.gain != 1.0 {
                for sample in stereo.iter_mut() {
                    let amplified = *sample * options.gain;
                    if amplified.abs() > 1.0 {
                        gain_clipped_samples += 1;
                    }
                    *sample = amplified.clamp(-1.0, 1.0);
                }
            }

            options.buffer.push_capture(
                options.kind,
                options.system_stream_index,
                output_qpc,
                stereo,
                !discontinuity,
            );
            packet_len = unsafe {
                capture
                    .GetNextPacketSize()
                    .map_err(|e| format!("GetNextPacketSize failed: {e}"))?
            };
        }
        if stopping {
            break;
        }
        thread::sleep(Duration::from_millis(10));
    }

    unsafe {
        crate::log_if_err!(client.Stop());
    }
    let gain_processed_samples = total_output_frames.saturating_mul(u64::from(TARGET_CHANNELS));
    let gain_clipped_percent = if gain_processed_samples == 0 {
        0.0
    } else {
        gain_clipped_samples as f64 * 100.0 / gain_processed_samples as f64
    };
    crate::log_debug(&format!(
        "Podcast capture sync: source={} packets={} input_frames={} output_frames={} discontinuities={} timestamp_errors={} gain={} gain_clipped_samples={} gain_processed_samples={} gain_clipped_percent={:.4}%",
        if matches!(options.kind, SourceKind::Microphone) {
            "microphone"
        } else {
            "system"
        },
        packet_counter,
        total_input_frames,
        total_output_frames,
        discontinuities,
        timestamp_errors,
        options.gain,
        gain_clipped_samples,
        gain_processed_samples,
        gain_clipped_percent
    ));
    Ok(())
}

fn resolve_device(device_id: &str, loopback: bool) -> Result<IMMDevice, String> {
    // Note: COM must already be initialized by the caller and kept alive
    // for the lifetime of the returned device.
    let enumerator: IMMDeviceEnumerator = unsafe {
        CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)
            .map_err(|e| format!("MMDeviceEnumerator failed: {e}"))?
    };

    if device_id.is_empty() || device_id == PODCAST_DEVICE_DEFAULT {
        let flow = if loopback { eRender } else { eCapture };
        crate::log_debug(&format!(
            "resolve_device: using default device (loopback={})",
            loopback
        ));
        return unsafe {
            enumerator
                .GetDefaultAudioEndpoint(flow, eConsole)
                .map_err(|e| format!("GetDefaultAudioEndpoint failed: {e}"))
        };
    }

    crate::log_debug(&format!(
        "resolve_device: looking for device_id='{}'",
        device_id
    ));
    let wide = crate::accessibility::to_wide(device_id);
    let result = unsafe {
        enumerator
            .GetDevice(PCWSTR(wide.as_ptr()))
            .map_err(|e| format!("GetDevice({}) failed: {e}", device_id))
    };
    if result.is_ok() {
        crate::log_debug("resolve_device: device found successfully");
    }
    result
}

fn resolve_device_with_name(
    device_id: &str,
    device_name: &str,
    loopback: bool,
) -> Result<IMMDevice, String> {
    let name = device_name.trim();

    // Attempt to resolve by ID first
    match resolve_device(device_id, loopback) {
        Ok(device) => Ok(device),
        Err(err) => {
            if name.is_empty() {
                return Err(err);
            }
            let flow = if loopback { eRender } else { eCapture };
            let enumerator: IMMDeviceEnumerator = unsafe {
                CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)
                    .map_err(|e| format!("MMDeviceEnumerator failed: {e}"))?
            };
            let collection: IMMDeviceCollection = unsafe {
                enumerator
                    .EnumAudioEndpoints(flow, DEVICE_STATE_ACTIVE)
                    .map_err(|e| format!("EnumAudioEndpoints failed: {e}"))?
            };
            let count = unsafe {
                collection
                    .GetCount()
                    .map_err(|e| format!("GetCount failed: {e}"))?
            };
            let needle = name.to_lowercase();
            for index in 0..count {
                let device: IMMDevice = unsafe {
                    collection
                        .Item(index)
                        .map_err(|e| format!("Device Item failed: {e}"))?
                };
                if let Some(render_name) = device_friendly_name(&device)
                    && (render_name.to_lowercase().contains(&needle)
                        || needle.contains(&render_name.to_lowercase()))
                {
                    crate::log_debug(&format!(
                        "resolve_device: matched by name '{}'",
                        render_name
                    ));
                    return Ok(device);
                }
            }

            Err(err)
        }
    }
}

fn pcm_sample_format(bits_per_sample: u16) -> Result<SampleFormat, String> {
    match bits_per_sample {
        16 => Ok(SampleFormat::I16),
        24 => Ok(SampleFormat::I24),
        32 => Ok(SampleFormat::I32),
        other => Err(format!(
            "Unsupported PCM capture format: {other} bits per sample"
        )),
    }
}

fn parse_format(fmt: &WAVEFORMATEX) -> Result<(u32, u16, SampleFormat), String> {
    // WAVEFORMATEX is packed on Windows. Copy fields that are later passed to
    // formatting machinery into aligned locals so Rust never creates an
    // unaligned reference to a packed field.
    let channels = fmt.nChannels;
    let rate = fmt.nSamplesPerSec;
    let block_align = fmt.nBlockAlign;
    let avg_bytes_per_sec = fmt.nAvgBytesPerSec;
    let cb_size = fmt.cbSize;
    if channels < 1 {
        return Err(format!(
            "Invalid capture channel count {} in mix format",
            channels
        ));
    }
    if rate == 0 {
        return Err("Invalid capture sample rate 0 in mix format".to_string());
    }

    let tag = fmt.wFormatTag as u32;
    let bits = fmt.wBitsPerSample;
    let (format, subtype_name) = if tag == WAVE_FORMAT_IEEE_FLOAT {
        if bits != 32 {
            return Err(format!(
                "Unsupported IEEE float capture format: {bits} bits per sample"
            ));
        }
        (SampleFormat::F32, "IEEE_FLOAT")
    } else if tag == WAVE_FORMAT_PCM_TAG {
        (pcm_sample_format(bits)?, "PCM")
    } else if tag == WAVE_FORMAT_EXTENSIBLE {
        if cb_size < 22 {
            return Err(format!(
                "Invalid WAVE_FORMAT_EXTENSIBLE capture format: cbSize={} (expected at least 22)",
                cb_size
            ));
        }
        let ext = crate::wave_format_extensible_ref_safe(fmt);
        let subformat = crate::read_unaligned_safe(std::ptr::addr_of!(ext.SubFormat));
        if subformat == KSDATAFORMAT_SUBTYPE_IEEE_FLOAT {
            if bits != 32 {
                return Err(format!(
                    "Unsupported extensible IEEE float capture format: {bits} bits per sample"
                ));
            }
            (SampleFormat::F32, "IEEE_FLOAT")
        } else if subformat == KSDATAFORMAT_SUBTYPE_PCM {
            (pcm_sample_format(bits)?, "PCM")
        } else {
            return Err(format!(
                "Unsupported extensible capture subformat {:?} ({} bits per sample)",
                subformat, bits
            ));
        }
    } else {
        return Err(format!(
            "Unsupported capture format tag 0x{tag:04X} ({} bits per sample)",
            bits
        ));
    };

    crate::log_debug(&format!(
        "Podcast WASAPI format: tag=0x{tag:04X} subtype={} rate={} channels={} bits_per_sample={} block_align={} avg_bytes_per_sec={} cb_size={} decoded_as={}",
        subtype_name,
        rate,
        channels,
        bits,
        block_align,
        avg_bytes_per_sec,
        cb_size,
        format.name()
    ));

    Ok((rate, channels, format))
}

fn parse_mix_format_ptr(mix_format: *mut WAVEFORMATEX) -> Result<(u32, u16, SampleFormat), String> {
    crate::with_raw_mut_ptr_safe(mix_format, |fmt| parse_format(fmt))
        .ok_or_else(|| "GetMixFormat returned null pointer".to_string())?
}

fn read_samples(ptr: *mut u8, frames: u32, channels: u16, format: SampleFormat) -> Vec<f32> {
    let sample_count = frames as usize * channels as usize;
    if ptr.is_null() || sample_count == 0 {
        return Vec::new();
    }
    unsafe {
        match format {
            SampleFormat::F32 => {
                let slice = std::slice::from_raw_parts(ptr as *const f32, sample_count);
                slice.to_vec()
            }
            SampleFormat::I16 => {
                let slice = std::slice::from_raw_parts(ptr as *const i16, sample_count);
                slice.iter().map(|s| *s as f32 / i16::MAX as f32).collect()
            }
            SampleFormat::I24 => {
                let bytes = std::slice::from_raw_parts(ptr as *const u8, sample_count * 3);
                bytes
                    .as_chunks::<3>()
                    .0
                    .iter()
                    .map(|sample| {
                        let raw = (sample[0] as i32)
                            | ((sample[1] as i32) << 8)
                            | ((sample[2] as i32) << 16);
                        let signed = if raw & 0x0080_0000 != 0 {
                            raw | !0x00FF_FFFF
                        } else {
                            raw
                        };
                        signed as f32 / 8_388_608.0
                    })
                    .collect()
            }
            SampleFormat::I32 => {
                let slice = std::slice::from_raw_parts(ptr as *const i32, sample_count);
                slice.iter().map(|s| *s as f32 / 2_147_483_648.0).collect()
            }
        }
    }
}

fn update_peak(shared: &SharedState, kind: &SourceKind, samples: &[f32]) {
    let mut peak = 0f32;
    for sample in samples {
        let abs = sample.abs();
        if abs > peak {
            peak = abs;
        }
    }
    let value = (peak * i16::MAX as f32) as u32;
    match kind {
        SourceKind::Microphone => {
            shared.mic_peak.store(value, Ordering::Relaxed);
            if value > 0 && value.is_multiple_of(5000) {
                crate::log_debug(&format!("Recorder mic peak={}", value));
            }
        }
        SourceKind::System => {
            shared.system_peak.store(value, Ordering::Relaxed);
        }
    }
}

fn to_stereo(samples: &[f32], channels: usize) -> Vec<f32> {
    if channels == TARGET_CHANNELS as usize {
        return samples.to_vec();
    }
    let frames = samples.len() / channels;
    let mut out = Vec::with_capacity(frames * TARGET_CHANNELS as usize);
    for frame in 0..frames {
        let base = frame * channels;
        let left = samples[base];
        let right = if channels > 1 {
            samples[base + 1]
        } else {
            left
        };
        out.push(left);
        out.push(right);
    }
    out
}

struct LinearResampler {
    input_rate: u32,
    output_rate: u32,
    channels: usize,
    pos: f64,
    buffer: Vec<f32>,
}

impl LinearResampler {
    fn new(input_rate: u32, output_rate: u32, channels: usize) -> Self {
        LinearResampler {
            input_rate,
            output_rate,
            channels,
            pos: 0.0,
            buffer: Vec::new(),
        }
    }

    fn push(&mut self, samples: &[f32]) -> Vec<f32> {
        self.buffer.extend_from_slice(samples);
        if self.input_rate == 0 || self.output_rate == 0 || self.channels == 0 {
            return Vec::new();
        }
        let step = self.input_rate as f64 / self.output_rate as f64;
        let frames_available = self.buffer.len() / self.channels;
        let mut out = Vec::new();
        while self.pos + 1.0 < frames_available as f64 {
            let i0 = self.pos.floor() as usize;
            let i1 = i0 + 1;
            let frac = self.pos - i0 as f64;
            for ch in 0..self.channels {
                let s0 = self.buffer[i0 * self.channels + ch];
                let s1 = self.buffer[i1 * self.channels + ch];
                out.push((1.0 - frac as f32) * s0 + (frac as f32) * s1);
            }
            self.pos += step;
        }
        let drop_frames = self.pos.floor() as usize;
        if drop_frames > 0 {
            let drop_samples = drop_frames * self.channels;
            self.buffer.drain(0..drop_samples);
            self.pos -= drop_frames as f64;
        }
        out
    }
}

pub fn default_output_folder() -> PathBuf {
    PathBuf::from(settings::default_podcast_save_folder())
}

pub(crate) fn process_loopback_wave_format() -> WAVEFORMATEX {
    let bits_per_sample = 16u16;
    let block_align = TARGET_CHANNELS * (bits_per_sample / 8);
    WAVEFORMATEX {
        wFormatTag: 1,
        nChannels: TARGET_CHANNELS,
        nSamplesPerSec: TARGET_SAMPLE_RATE,
        nAvgBytesPerSec: TARGET_SAMPLE_RATE * block_align as u32,
        nBlockAlign: block_align,
        wBitsPerSample: bits_per_sample,
        cbSize: 0,
    }
}

pub(crate) fn activate_process_loopback_client(process_id: u32) -> Result<IAudioClient, String> {
    crate::log_debug(&format!(
        "Process loopback activation: requesting async activation for PID {}",
        process_id
    ));
    let params = AUDIOCLIENT_ACTIVATION_PARAMS {
        ActivationType: AUDIOCLIENT_ACTIVATION_TYPE_PROCESS_LOOPBACK,
        Anonymous: AUDIOCLIENT_ACTIVATION_PARAMS_0 {
            ProcessLoopbackParams: AUDIOCLIENT_PROCESS_LOOPBACK_PARAMS {
                TargetProcessId: process_id,
                ProcessLoopbackMode: PROCESS_LOOPBACK_MODE_INCLUDE_TARGET_PROCESS_TREE,
            },
        },
    };

    let mut raw: windows::core::imp::PROPVARIANT = unsafe { std::mem::zeroed() };
    raw.Anonymous.Anonymous.vt = VT_BLOB.0;
    raw.Anonymous.Anonymous.Anonymous.blob = windows::core::imp::BLOB {
        cbSize: size_of::<AUDIOCLIENT_ACTIVATION_PARAMS>() as u32,
        pBlobData: (&params as *const AUDIOCLIENT_ACTIVATION_PARAMS)
            .cast_mut()
            .cast(),
    };
    let prop_variant = ManuallyDrop::new(unsafe { PROPVARIANT::from_raw(raw) });

    let state = Arc::new(ActivationState::default());
    let handler: IActivateAudioInterfaceCompletionHandler =
        ActivateAudioCompletionHandler::new(state.clone()).into();
    let _operation = unsafe {
        ActivateAudioInterfaceAsync(
            VIRTUAL_AUDIO_DEVICE_PROCESS_LOOPBACK,
            &IAudioClient::IID,
            Some((&*prop_variant) as *const PROPVARIANT),
            &handler,
        )
        .map_err(|e| format!("ActivateAudioInterfaceAsync failed: {e}"))?
    };

    let mut guard = state.result.lock().unwrap_or_else(|e| e.into_inner());
    while guard.is_none() {
        guard = state.condvar.wait(guard).unwrap_or_else(|e| e.into_inner());
    }
    let raw = guard
        .take()
        .ok_or_else(|| "Activation completed without result.".to_string())??;
    crate::log_debug(&format!(
        "Process loopback activation: async activation completed for PID {}",
        process_id
    ));
    unsafe {
        Ok(IAudioClient::from_raw(
            (raw as *mut core::ffi::c_void).cast(),
        ))
    }
}

#[derive(Default)]
struct ActivationState {
    result: Mutex<Option<Result<usize, String>>>,
    condvar: Condvar,
}

#[implement(IActivateAudioInterfaceCompletionHandler)]
struct ActivateAudioCompletionHandler {
    state: Arc<ActivationState>,
}

impl ActivateAudioCompletionHandler {
    fn new(state: Arc<ActivationState>) -> Self {
        Self { state }
    }
}

impl windows::Win32::Media::Audio::IActivateAudioInterfaceCompletionHandler_Impl
    for ActivateAudioCompletionHandler
{
    fn ActivateCompleted(
        &self,
        activateoperation: Option<&IActivateAudioInterfaceAsyncOperation>,
    ) -> windows::core::Result<()> {
        crate::log_debug("Process loopback activation: callback entered");
        let result = match activateoperation {
            Some(operation) => {
                let mut activation_hr = HRESULT(0);
                let mut activated = None;
                unsafe {
                    operation.GetActivateResult(&mut activation_hr, &mut activated)?;
                }
                if let Err(err) = activation_hr.ok() {
                    crate::log_debug(&format!(
                        "Process loopback activation: GetActivateResult returned error {}",
                        err
                    ));
                    Err(format!("GetActivateResult failed: {err}"))
                } else if let Some(activated) = activated {
                    match activated.cast::<IAudioClient>() {
                        Ok(client) => {
                            crate::log_debug("Process loopback activation: received IAudioClient");
                            Ok(client.into_raw() as usize)
                        }
                        Err(err) => {
                            crate::log_debug(&format!(
                                "Process loopback activation: IAudioClient cast failed {}",
                                err
                            ));
                            Err(format!("IAudioClient cast failed: {err}"))
                        }
                    }
                } else {
                    crate::log_debug(
                        "Process loopback activation: callback returned no activated interface",
                    );
                    Err("Activation returned no audio client.".to_string())
                }
            }
            None => {
                crate::log_debug("Process loopback activation: callback received no operation");
                Err("Audio activation callback received no operation.".to_string())
            }
        };

        let mut guard = self.state.result.lock().unwrap_or_else(|e| e.into_inner());
        *guard = Some(result);
        self.state.condvar.notify_one();
        Ok(())
    }
}

fn process_display_name(process_id: u32) -> String {
    let base_name = process_image_name(process_id)
        .and_then(|path| {
            PathBuf::from(path)
                .file_stem()
                .map(|stem| stem.to_string_lossy().to_string())
        })
        .filter(|name| !name.trim().is_empty())
        .unwrap_or_else(|| format!("PID {process_id}"));
    if let Some(window_title) = crate::find_process_window_title(process_id)
        && !window_title.is_empty()
    {
        format!("{base_name} - {window_title} (PID {process_id})")
    } else {
        format!("{base_name} (PID {process_id})")
    }
}

fn process_image_name(process_id: u32) -> Option<String> {
    unsafe {
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, process_id).ok()?;
        let mut buffer = vec![0u16; 1024];
        let mut len = buffer.len() as u32;
        let result = QueryFullProcessImageNameW(
            handle,
            PROCESS_NAME_FORMAT(0),
            PWSTR(buffer.as_mut_ptr()),
            &mut len,
        );
        crate::log_if_err!(CloseHandle(handle));
        if result.is_err() || len == 0 {
            return None;
        }
        Some(
            OsString::from_wide(&buffer[..len as usize])
                .to_string_lossy()
                .to_string(),
        )
    }
}

#[cfg(test)]
mod synchronization_tests {
    use super::*;

    fn stopped_buffer(frames: u64, systems: usize) -> Result<Arc<MixBuffer>, String> {
        let buffer = Arc::new(MixBuffer::new(systems)?);
        buffer
            .inner
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .timeline
            .end = Some(buffer.origin_qpc + frame_ticks(frames, TARGET_SAMPLE_RATE));
        Ok(buffer)
    }

    #[test]
    fn pcm24_capture_samples_are_decoded_without_treating_them_as_i16() {
        let bytes = [
            0x00u8, 0x00, 0x00, // 0
            0xFF, 0xFF, 0x7F, // almost +1
            0x00, 0x00, 0x80, // -1
        ];
        let decoded = read_samples(bytes.as_ptr() as *mut u8, 3, 1, SampleFormat::I24);
        assert_eq!(decoded.len(), 3);
        assert_eq!(decoded[0], 0.0);
        assert!((decoded[1] - 0.999_999_9).abs() < 0.000_001);
        assert_eq!(decoded[2], -1.0);
    }

    #[test]
    fn pcm32_capture_samples_are_decoded_without_treating_them_as_i16() {
        let samples = [0i32, i32::MAX, i32::MIN];
        let decoded = read_samples(
            samples.as_ptr() as *mut u8,
            samples.len() as u32,
            1,
            SampleFormat::I32,
        );
        assert_eq!(decoded.len(), 3);
        assert_eq!(decoded[0], 0.0);
        assert!(decoded[1] > 0.999_999);
        assert_eq!(decoded[2], -1.0);
    }

    #[test]
    fn silent_system_does_not_stall_microphone_or_shorten_recording() -> Result<(), String> {
        let buffer = stopped_buffer(1027, 1)?;
        buffer.push(
            SourceKind::Microphone,
            0,
            buffer.origin_qpc,
            vec![0.25; 1027 * 2],
        );
        let stop = AtomicBool::new(true);
        let mut count = 0;
        while let Some((mic, system)) = buffer.next_chunk(&stop) {
            assert!(mic.iter().all(|v| *v == 0.25));
            assert!(system.iter().all(|v| *v == 0.0));
            assert_eq!(mic.len(), system.len());
            count += mic.len() / 2;
        }
        assert_eq!(count, 1027, "partial final chunk must be saved");
        Ok(())
    }

    #[test]
    fn resumed_system_packet_keeps_its_time_in_both_tracks() -> Result<(), String> {
        let buffer = stopped_buffer(2000, 1)?;
        buffer.push(
            SourceKind::Microphone,
            0,
            buffer.origin_qpc,
            vec![0.25; 4000],
        );
        buffer.push(
            SourceKind::System,
            0,
            buffer.origin_qpc + frame_ticks(1500, TARGET_SAMPLE_RATE),
            vec![0.75; 1000],
        );
        let mut system_track = Vec::new();
        while let Some((_, system)) = buffer.next_chunk(&AtomicBool::new(true)) {
            system_track.extend(system);
        }
        assert_eq!(system_track, [vec![0.0; 3000], vec![0.75; 1000]].concat());
        Ok(())
    }

    #[test]
    fn packets_crossing_pause_are_cut_at_the_same_time_for_both_sources() -> Result<(), String> {
        let buffer = stopped_buffer(44100 * 3, 1)?;
        {
            let mut inner = buffer.inner.lock().unwrap_or_else(|e| e.into_inner());
            let end = inner.timeline.end.take();
            inner.timeline.pause(buffer.origin_qpc + TICKS_PER_SECOND);
            inner
                .timeline
                .resume(buffer.origin_qpc + 2 * TICKS_PER_SECOND);
            inner.timeline.end = end;
        }
        for source in [SourceKind::Microphone, SourceKind::System] {
            buffer.push(
                source,
                0,
                buffer.origin_qpc,
                [vec![0.25; 88200], vec![0.5; 88200], vec![0.75; 88200]].concat(),
            );
        }
        let mut recorded = Vec::new();
        while let Some((mic, system)) = buffer.next_chunk(&AtomicBool::new(true)) {
            assert_eq!(mic, system);
            recorded.extend(mic);
        }
        assert_eq!(recorded.len(), 176400);
        assert!(recorded[..88200].iter().all(|value| *value == 0.25));
        assert!(recorded[88200..].iter().all(|value| *value == 0.75));
        Ok(())
    }

    #[test]
    fn resampling_different_packet_sizes_preserves_shared_impulse_time() -> Result<(), String> {
        let buffer = stopped_buffer(44100, 1)?;
        for (kind, channels, chunk) in [
            (SourceKind::Microphone, 1, 480),
            (SourceKind::System, 2, 960),
        ] {
            let mut resampler = LinearResampler::new(48000, TARGET_SAMPLE_RATE, channels);
            for offset in (0..48000).step_by(chunk) {
                let mut samples = vec![0.0; chunk * channels];
                if (offset..offset + chunk).contains(&24000) {
                    for channel in 0..channels {
                        samples[(24000 - offset) * channels + channel] = 1.0;
                    }
                }
                let buffered = resampler.buffer.len() as f64 / channels as f64 - resampler.pos;
                let qpc = buffer.origin_qpc + frame_ticks(offset as u64, 48000)
                    - (buffered.max(0.0) * TICKS_PER_SECOND as f64 / 48000.0).round() as u64;
                let converted = to_stereo(&resampler.push(&samples), channels);
                buffer.push(kind, 0, qpc, converted);
            }
        }
        let mut microphone = Vec::new();
        let mut system_track = Vec::new();
        while let Some((mic, system)) = buffer.next_chunk(&AtomicBool::new(true)) {
            microphone.extend(mic);
            system_track.extend(system);
        }
        let peak = |samples: &[f32]| {
            samples
                .iter()
                .enumerate()
                .max_by(|(_, a), (_, b)| a.total_cmp(b))
                .map(|(index, _)| index / 2)
                .unwrap_or(usize::MAX)
        };
        assert!(peak(&microphone).abs_diff(22050) <= 1);
        assert!(peak(&microphone).abs_diff(peak(&system_track)) <= 1);
        Ok(())
    }

    #[test]
    fn multiple_system_streams_keep_existing_mix_levels() {
        let shared = SharedState::new(true, true);
        assert_eq!(
            mixed_chunk(vec![0.8, 0.8], vec![0.2, 0.2], &shared, 2),
            vec![0.3, 0.3]
        );
    }
}
