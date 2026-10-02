//! Audio capture module using WASAPI (Windows Audio Session API)
//!
//! This module provides audio capture functionality for screen recording

use crate::com_guard::ComGuard;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;
use windows::Win32::Media::Audio::{
    AUDCLNT_SHAREMODE_SHARED, AUDCLNT_STREAMFLAGS_LOOPBACK, IAudioCaptureClient, IAudioClient,
    IMMDevice, IMMDeviceEnumerator, MMDeviceEnumerator, eConsole, eRender,
};
use windows::Win32::System::Com::CLSCTX_ALL;

/// Audio sample without timestamp (will be calculated by encoder)
#[derive(Clone)]
pub struct AudioSample {
    pub data: Vec<i16>, // 16-bit PCM stereo samples
    pub sample_rate: u32,
    pub channels: u16,
}

/// Thread-safe queue for audio samples
pub struct AudioQueue {
    inner: Mutex<Vec<AudioSample>>,
    condvar: Condvar,
    max_samples: usize,
}

impl AudioQueue {
    pub fn new(max_samples: usize) -> Self {
        AudioQueue {
            inner: Mutex::new(Vec::with_capacity(max_samples)),
            condvar: Condvar::new(),
            max_samples,
        }
    }

    pub fn push(&self, sample: AudioSample) {
        let mut queue = self.inner.lock().unwrap_or_else(|e| e.into_inner());

        if queue.len() >= self.max_samples {
            queue.remove(0); // Drop oldest
            crate::log_debug(&format!(
                "WARNING: Audio queue overflow! Dropped oldest sample. Queue size: {}",
                self.max_samples
            ));
        }

        queue.push(sample);
        self.condvar.notify_one();
    }

    pub fn pop(&self, timeout: Duration) -> Option<AudioSample> {
        let mut queue = self.inner.lock().unwrap_or_else(|e| e.into_inner());

        if queue.is_empty() {
            let result = self
                .condvar
                .wait_timeout(queue, timeout)
                .unwrap_or_else(|e| e.into_inner());
            queue = result.0;
        }

        if !queue.is_empty() {
            Some(queue.remove(0))
        } else {
            None
        }
    }

    pub fn len(&self) -> usize {
        self.inner.lock().unwrap_or_else(|e| e.into_inner()).len()
    }

    pub fn is_empty(&self) -> bool {
        self
            .inner
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .is_empty()
    }
}

/// Handle to control audio recording
pub struct AudioRecorderHandle {
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<Result<(), String>>>,
    pub audio_queue: Arc<AudioQueue>,
    pub sample_rate: u32,
    pub channels: u16,
}

impl AudioRecorderHandle {
    /// Signal audio recording to stop (without waiting)
    pub fn signal_stop(&self) {
        self.stop.store(true, Ordering::SeqCst);
    }

    /// Stop audio recording and wait for thread to finish
    pub fn stop(mut self) -> Result<(), String> {
        self.stop.store(true, Ordering::SeqCst);

        if let Some(thread) = self.thread.take() {
            thread
                .join()
                .map_err(|_| "Audio capture thread panicked".to_string())??;
        }

        Ok(())
    }

    /// Wait for thread to finish (call after signal_stop)
    pub fn join(mut self) -> Result<(), String> {
        if let Some(thread) = self.thread.take() {
            thread
                .join()
                .map_err(|_| "Audio capture thread panicked".to_string())??;
        }
        Ok(())
    }
}

/// Create an AudioRecorderHandle from components (for internal use)
pub fn create_audio_recorder_handle(
    stop: Arc<AtomicBool>,
    thread: JoinHandle<Result<(), String>>,
    audio_queue: Arc<AudioQueue>,
    sample_rate: u32,
    channels: u16,
) -> AudioRecorderHandle {
    AudioRecorderHandle {
        stop,
        thread: Some(thread),
        audio_queue,
        sample_rate,
        channels,
    }
}

/// Start audio recording using WASAPI loopback
pub fn start_audio_recording() -> Result<AudioRecorderHandle, String> {
    let audio_queue = Arc::new(AudioQueue::new(3000)); // Large buffer to prevent overflow
    let stop = Arc::new(AtomicBool::new(false));

    // Get audio format info before starting thread
    let (sample_rate, channels) = get_audio_format()?;
    crate::log_debug(&format!(
        "Audio capture will use: {} Hz, {} channels",
        sample_rate, channels
    ));

    let audio_queue_clone = Arc::clone(&audio_queue);
    let stop_clone = Arc::clone(&stop);

    let thread = thread::spawn(move || audio_capture_loop(audio_queue_clone, stop_clone));

    Ok(AudioRecorderHandle {
        stop,
        thread: Some(thread),
        audio_queue,
        sample_rate,
        channels,
    })
}

/// Get the audio format that will be used for capture
fn get_audio_format() -> Result<(u32, u16), String> {
    let _com = ComGuard::new_sta().map_err(|e| format!("CoInitializeEx failed: {:?}", e))?;

    let enumerator: IMMDeviceEnumerator =
        crate::co_create_instance_safe(&MMDeviceEnumerator, None, CLSCTX_ALL)
            .map_err(|e| format!("Failed to create device enumerator: {}", e))?;

    let device: IMMDevice = unsafe { enumerator.GetDefaultAudioEndpoint(eRender, eConsole) }
        .map_err(|e| format!("Failed to get default audio endpoint: {}", e))?;

    let audio_client: IAudioClient = unsafe { device.Activate(CLSCTX_ALL, None) }
        .map_err(|e| format!("Failed to activate audio client: {}", e))?;

    let format_ptr = unsafe { audio_client.GetMixFormat() }
        .map_err(|e| format!("GetMixFormat failed: {}", e))?;
    let format = unsafe { &*format_ptr };

    let sample_rate = format.nSamplesPerSec;
    let channels = format.nChannels;

    Ok((sample_rate, channels))
}

fn audio_capture_loop(audio_queue: Arc<AudioQueue>, stop: Arc<AtomicBool>) -> Result<(), String> {
    let _power_awake = crate::power_awake::acquire("audio-recording");
    // Initialize COM for this thread - will be cleaned up when _com goes out of scope
    let _com = ComGuard::new_sta().map_err(|e| format!("CoInitializeEx failed: {:?}", e))?;

    audio_capture_loop_impl(audio_queue, stop)
}

fn audio_capture_loop_impl(
    audio_queue: Arc<AudioQueue>,
    stop: Arc<AtomicBool>,
) -> Result<(), String> {
    crate::log_debug("Audio capture loop started");

    // Get default audio endpoint (speakers/headphones for loopback)
    let enumerator: IMMDeviceEnumerator =
        crate::co_create_instance_safe(&MMDeviceEnumerator, None, CLSCTX_ALL)
            .map_err(|e| format!("Failed to create device enumerator: {}", e))?;

    let device: IMMDevice = unsafe { enumerator.GetDefaultAudioEndpoint(eRender, eConsole) }
        .map_err(|e| format!("Failed to get default audio endpoint: {}", e))?;

    let audio_client: IAudioClient = unsafe { device.Activate(CLSCTX_ALL, None) }
        .map_err(|e| format!("Failed to activate audio client: {}", e))?;

    // Get mix format
    let format_ptr = unsafe { audio_client.GetMixFormat() }
        .map_err(|e| format!("GetMixFormat failed: {}", e))?;
    let format = unsafe { &*format_ptr };

    // Initialize audio client for loopback capture
    let buffer_duration = 10_000_000; // 1 second in 100ns units
    let init_res = unsafe {
        audio_client.Initialize(
            AUDCLNT_SHAREMODE_SHARED,
            AUDCLNT_STREAMFLAGS_LOOPBACK,
            buffer_duration,
            0,
            format,
            None,
        )
    };
    init_res.map_err(|e| format!("Initialize failed: {}", e))?;

    let capture_client: IAudioCaptureClient = unsafe { audio_client.GetService() }
        .map_err(|e| format!("GetService failed: {}", e))?;

    // Start capture
    unsafe { audio_client.Start() }.map_err(|e| format!("Start failed: {}", e))?;

    // Copy packed struct fields to avoid unaligned reference errors
    let sample_rate = format.nSamplesPerSec;
    let channels = format.nChannels;
    let bits_per_sample = format.wBitsPerSample;
    let bytes_per_sample = (bits_per_sample / 8) as usize;

    crate::log_debug(&format!(
        "Audio format: {} Hz, {} channels, {} bits",
        sample_rate, channels, bits_per_sample
    ));

    let mut qpc_freq = 0i64;
    unsafe { windows::Win32::System::Performance::QueryPerformanceFrequency(&mut qpc_freq) }
        .ok()
        .ok_or("QueryPerformanceFrequency failed")?;

    let mut start_qpc = 0i64;
    unsafe { windows::Win32::System::Performance::QueryPerformanceCounter(&mut start_qpc) }
        .ok()
        .ok_or("QueryPerformanceCounter failed")?;

    // Capture loop
    while !stop.load(Ordering::SeqCst) {
        // Reduced sleep for faster audio capture - was 10ms, now 5ms
        thread::sleep(Duration::from_millis(5));

        let packet_length = match unsafe { capture_client.GetNextPacketSize() } {
            Ok(len) => len,
            Err(_) => continue,
        };

        let mut current_packet_length = packet_length;
        while current_packet_length > 0 {
            let mut buffer_ptr: *mut u8 = std::ptr::null_mut();
            let mut num_frames = 0u32;
            let mut flags = 0u32;

            let get_buffer_res = unsafe {
                capture_client.GetBuffer(&mut buffer_ptr, &mut num_frames, &mut flags, None, None)
            };
            if get_buffer_res.is_err() {
                break;
            }

            if num_frames > 0 && !buffer_ptr.is_null() {
                // Convert to 16-bit PCM stereo
                let frame_size = (channels as usize) * bytes_per_sample;
                let total_bytes = (num_frames as usize) * frame_size;
                let buffer_slice = unsafe { std::slice::from_raw_parts(buffer_ptr, total_bytes) };

                let mut samples = Vec::with_capacity(num_frames as usize * 2); // stereo

                if bits_per_sample == 16 {
                    // Already 16-bit PCM
                    for chunk in buffer_slice.chunks_exact(2) {
                        let sample = i16::from_le_bytes([chunk[0], chunk[1]]);
                        samples.push(sample);
                    }
                } else if bits_per_sample == 32 {
                    // Convert 32-bit float to 16-bit PCM
                    for chunk in buffer_slice.chunks_exact(4) {
                        let float_val =
                            f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
                        let sample = (float_val.clamp(-1.0, 1.0) * 32767.0) as i16;
                        samples.push(sample);
                    }
                }

                // If mono, duplicate to stereo
                if channels == 1 {
                    let mut stereo_samples = Vec::with_capacity(samples.len() * 2);
                    for sample in samples {
                        stereo_samples.push(sample);
                        stereo_samples.push(sample);
                    }
                    samples = stereo_samples;
                }

                let audio_sample = AudioSample {
                    data: samples,
                    sample_rate,
                    channels: 2, // Always output stereo
                };

                audio_queue.push(audio_sample);
            }

            crate::log_if_err!(unsafe { capture_client.ReleaseBuffer(num_frames) });

            current_packet_length = match unsafe { capture_client.GetNextPacketSize() } {
                Ok(len) => len,
                Err(_) => break,
            };
        }
    }

    unsafe { audio_client.Stop() }.map_err(|e| format!("Stop failed: {}", e))?;

    crate::log_debug("Audio capture loop stopped");
    Ok(())
}
