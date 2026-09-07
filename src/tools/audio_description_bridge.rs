use encoding_rs::WINDOWS_1252;
use reqwest::blocking::Client;
use reqwest::header::USER_AGENT;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, mpsc};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const CREATE_NO_WINDOW: u32 = 0x0800_0000;
#[cfg(debug_assertions)]
const BRIDGE_DEBUG_FILE_NAME: &str = "audio_description_bridge.exe";
const BRIDGE_CACHE_FILE_NAME: &str = "audio_description_bridge_v6.exe";
const BRIDGE_MIN_VALID_SIZE_BYTES: u64 = 5_000_000;
const BRIDGE_DOWNLOAD_URLS: [&str; 2] = [
    "https://github.com/Ambro86/Sonarpad-Tools/releases/download/0.7/audio_description_bridge.exe",
    "https://github.com/Ambro86/Sonarpad-Tools/releases/download/v0.7/audio_description_bridge.exe",
];

#[derive(Debug, Clone, Serialize)]
pub struct AudioDescriptionPreparedChunk {
    pub path: String,
    pub start_sec: f64,
    pub end_sec: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BridgeCharacter {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub description: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct AudioDescriptionBridgeRequest {
    pub input_path: String,
    pub audio_wav_path: Option<String>,
    pub duration_sec: f64,
    pub chunks: Vec<AudioDescriptionPreparedChunk>,
    pub language: String,
    pub verbosity: String,
    pub allow_extended_pauses: bool,
    pub recognize_characters: bool,
    pub recognize_screen_text: bool,
    pub initial_character_glossary: Vec<BridgeCharacter>,
    /// Explicit AI access mode. Never infer/fallback between personal and Sonarpad AI.
    pub ai_access_mode: String,
    pub gemini_api_key: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub sonarpad_ai_service_url: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub sonarpad_ai_access_code: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub sonarpad_ai_device_id: String,
    pub gemini_model: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resume: Option<AudioDescriptionBridgeResume>,
}

#[derive(Debug, Clone, Serialize)]
pub struct AudioDescriptionBridgeResume {
    pub completed_chunks: usize,
    pub descriptions: Vec<BridgeDescription>,
    pub character_glossary: Vec<BridgeCharacter>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BridgeInterval {
    pub start_sec: f64,
    pub end_sec: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BridgeDescription {
    pub start_sec: f64,
    #[serde(default)]
    pub visual_start_sec: Option<f64>,
    #[serde(default)]
    pub visual_evidence_time_sec: Option<f64>,
    #[serde(default)]
    pub end_sec: f64,
    pub text: String,
    #[serde(default)]
    pub mandatory: bool,
    #[serde(default)]
    pub slot_start_sec: Option<f64>,
    #[serde(default)]
    pub slot_end_sec: Option<f64>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AudioDescriptionBridgeResult {
    #[serde(default)]
    pub ok: bool,
    #[serde(default)]
    pub cancelled: bool,
    #[serde(default)]
    pub error: String,
    #[serde(default)]
    pub log_path: String,
    #[serde(default)]
    pub duration_sec: f64,
    #[serde(default)]
    pub chunk_duration_sec: u32,
    #[serde(default)]
    pub analysis_engine: String,
    #[serde(default)]
    pub protected_intervals: Vec<BridgeInterval>,
    #[serde(default)]
    pub descriptions: Vec<BridgeDescription>,
    #[serde(default)]
    pub character_glossary: Vec<BridgeCharacter>,
    #[serde(default)]
    pub gemini_model: String,
}

#[derive(Debug, Clone, Deserialize)]
struct BridgeStatus {
    #[serde(default)]
    stage: String,
    #[serde(default)]
    message: String,
}

#[derive(Debug, Clone, Deserialize)]
struct BridgeQuota {
    #[serde(default)]
    model: String,
    #[serde(default)]
    error: String,
}

#[derive(Debug, Clone, Deserialize)]
struct BridgeOverload {
    #[serde(default)]
    model: String,
    #[serde(default)]
    error: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AudioDescriptionBridgeCheckpoint {
    #[serde(default)]
    pub completed_chunks: usize,
    #[serde(default)]
    pub total_chunks: usize,
    #[serde(default)]
    pub descriptions: Vec<BridgeDescription>,
    #[serde(default)]
    pub character_glossary: Vec<BridgeCharacter>,
    #[serde(default)]
    pub gemini_model: String,
}

#[derive(Debug, Clone)]
pub enum AudioDescriptionQuotaDecision {
    SwitchModel(String),
    Wait,
    Stop,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioDescriptionOverloadDecision {
    Wait,
    Stop,
}

pub type AudioDescriptionBridgePercentCallback = Box<dyn FnMut(i32) + Send>;
pub type AudioDescriptionBridgeStatusCallback = Box<dyn FnMut(&str, &str) + Send>;
pub type AudioDescriptionBridgeQuotaCallback =
    Box<dyn FnMut(&str, &str) -> AudioDescriptionQuotaDecision + Send>;
pub type AudioDescriptionBridgeOverloadCallback =
    Box<dyn FnMut(&str, &str) -> AudioDescriptionOverloadDecision + Send>;
pub type AudioDescriptionBridgeCheckpointCallback =
    Box<dyn FnMut(&AudioDescriptionBridgeCheckpoint) + Send>;

pub struct AudioDescriptionBridgeCallbacks {
    pub download: Option<AudioDescriptionBridgePercentCallback>,
    pub progress: Option<AudioDescriptionBridgePercentCallback>,
    pub status: Option<AudioDescriptionBridgeStatusCallback>,
    pub quota: Option<AudioDescriptionBridgeQuotaCallback>,
    pub overload: Option<AudioDescriptionBridgeOverloadCallback>,
    pub checkpoint: Option<AudioDescriptionBridgeCheckpointCallback>,
}

fn bridge_install_path() -> PathBuf {
    crate::settings::settings_dir()
        .join("tools")
        .join(BRIDGE_CACHE_FILE_NAME)
}

#[cfg(debug_assertions)]
fn repo_dll_dir_candidates() -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    if let Ok(exe_path) = std::env::current_exe()
        && let Some(repo_root) = exe_path
            .parent()
            .and_then(|path| path.parent())
            .and_then(|path| path.parent())
    {
        candidates.push(repo_root.join("dll"));
    }
    if let Ok(cwd) = std::env::current_dir() {
        candidates.push(cwd.join("dll"));
    }
    candidates
}

fn is_valid_bridge_exe(path: &Path) -> Result<bool, String> {
    let meta = fs::metadata(path).map_err(|error| format!("bridge stat failed: {error}"))?;
    if meta.len() < BRIDGE_MIN_VALID_SIZE_BYTES {
        return Ok(false);
    }
    let mut file = fs::File::open(path).map_err(|error| format!("bridge open failed: {error}"))?;
    let mut header = [0_u8; 2];
    if file
        .read(&mut header)
        .map_err(|error| format!("bridge read failed: {error}"))?
        < 2
    {
        return Ok(false);
    }
    Ok(header == *b"MZ")
}

fn download_bridge(
    target_path: &Path,
    cancel: &Arc<AtomicBool>,
    progress: &mut Option<Box<dyn FnMut(i32) + Send>>,
) -> Result<(), String> {
    let parent = target_path
        .parent()
        .ok_or_else(|| "invalid audio-description bridge path".to_string())?;
    fs::create_dir_all(parent)
        .map_err(|error| format!("create audio-description app-data dir failed: {error}"))?;
    let client = Client::builder()
        .connect_timeout(Duration::from_secs(30))
        .timeout(Duration::from_secs(30 * 60))
        .build()
        .map_err(|error| format!("audio-description HTTP client failed: {error}"))?;
    let part_path = target_path.with_extension("download.part");
    let mut last_error = String::new();

    for url in BRIDGE_DOWNLOAD_URLS {
        if cancel.load(Ordering::Relaxed) {
            return Err("cancelled".to_string());
        }
        crate::log_debug(&format!("Audio description: downloading worker from {url}"));
        let attempt = (|| -> Result<(), String> {
            let mut response = client
                .get(url)
                .header(USER_AGENT, "Sonarpad/audio-description-bridge")
                .send()
                .map_err(|error| format!("worker download request failed: {error}"))?;
            if !response.status().is_success() {
                return Err(format!(
                    "worker download failed: HTTP {}",
                    response.status()
                ));
            }
            let total = response.content_length();
            let mut output = fs::File::create(&part_path)
                .map_err(|error| format!("create worker download failed: {error}"))?;
            let mut buffer = vec![0_u8; 128 * 1024];
            let mut downloaded = 0_u64;
            let mut last_pct = -1;
            loop {
                if cancel.load(Ordering::Relaxed) {
                    {
                        let _closed_output = output;
                    }
                    crate::log_if_err!(
                        fs::remove_file(&part_path),
                        "Audio description: remove partial worker after cancellation failed"
                    );
                    return Err("cancelled".to_string());
                }
                let read = response
                    .read(&mut buffer)
                    .map_err(|error| format!("worker download read failed: {error}"))?;
                if read == 0 {
                    break;
                }
                output
                    .write_all(&buffer[..read])
                    .map_err(|error| format!("worker download write failed: {error}"))?;
                downloaded = downloaded.saturating_add(read as u64);
                if let Some(total) = total
                    && total > 0
                {
                    let pct = ((downloaded.saturating_mul(100)) / total).min(100) as i32;
                    if pct > last_pct {
                        last_pct = pct;
                        if let Some(callback) = progress.as_mut() {
                            callback(pct);
                        }
                    }
                }
            }
            output
                .flush()
                .map_err(|error| format!("worker download flush failed: {error}"))?;
            {
                let _closed_output = output;
            }
            fs::rename(&part_path, target_path)
                .map_err(|error| format!("worker download finalize failed: {error}"))?;
            if !is_valid_bridge_exe(target_path)? {
                crate::log_if_err!(
                    fs::remove_file(target_path),
                    "Audio description: remove invalid downloaded worker failed"
                );
                return Err("downloaded worker is not a valid Windows executable".to_string());
            }
            if let Some(callback) = progress.as_mut() {
                callback(100);
            }
            Ok(())
        })();
        match attempt {
            Ok(()) => return Ok(()),
            Err(error) if error == "cancelled" => return Err(error),
            Err(error) => {
                last_error = error;
                crate::log_if_err!(
                    fs::remove_file(&part_path),
                    "Audio description: remove failed partial worker download failed"
                );
            }
        }
    }

    if last_error.is_empty() {
        Err("audio-description worker download failed".to_string())
    } else {
        Err(last_error)
    }
}

fn ensure_bridge(
    cancel: &Arc<AtomicBool>,
    download_progress: &mut Option<Box<dyn FnMut(i32) + Send>>,
) -> Result<PathBuf, String> {
    #[cfg(debug_assertions)]
    for candidate in repo_dll_dir_candidates()
        .into_iter()
        .map(|dir| dir.join(BRIDGE_DEBUG_FILE_NAME))
    {
        if candidate.is_file() && is_valid_bridge_exe(&candidate).unwrap_or(false) {
            crate::log_debug(&format!(
                "Audio description: using local debug worker {}",
                candidate.display()
            ));
            return Ok(candidate);
        }
    }

    let cached = bridge_install_path();
    if cached.exists() {
        match is_valid_bridge_exe(&cached) {
            Ok(true) => {
                crate::log_debug(&format!(
                    "Audio description: using cached worker {}",
                    cached.display()
                ));
                return Ok(cached);
            }
            Ok(false) => {
                crate::log_debug(
                    "Audio description: existing cached worker invalid, re-downloading",
                );
            }
            Err(error) => {
                crate::log_debug(&format!(
                    "Audio description: cached worker validation failed: {error}"
                ));
            }
        }
        if let Err(error) = fs::remove_file(&cached) {
            crate::log_debug(&format!(
                "Audio description: invalid cached worker removal failed: {error}"
            ));
        }
    }
    download_bridge(&cached, cancel, download_progress)?;
    crate::log_debug(&format!(
        "Audio description: worker ready at {}",
        cached.display()
    ));
    Ok(cached)
}

fn decode_bridge_text(raw: &[u8]) -> String {
    if let Ok(text) = std::str::from_utf8(raw) {
        return text.to_string();
    }
    let (decoded, _, _) = WINDOWS_1252.decode(raw);
    decoded.into_owned()
}

fn terminate_bridge_process_tree(child: &mut Child) {
    let pid = child.id();
    crate::log_debug(&format!(
        "Audio description: force-stopping worker process tree pid={pid}"
    ));

    // Kill the direct worker immediately. Do not block on wait()/join() here:
    // a stuck API call or inherited pipe must never leave the dialog forever
    // on "Cancelling...". taskkill is also launched for descendants that
    // may have inherited handles from the packaged Python worker.
    if let Err(error) = child.kill() {
        crate::log_debug(&format!(
            "Audio description: direct worker kill returned {error}; forcing process tree"
        ));
    }
    if let Err(error) = Command::new("taskkill")
        .args(["/PID", &pid.to_string(), "/T", "/F"])
        .creation_flags(CREATE_NO_WINDOW)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
    {
        crate::log_debug(&format!(
            "Audio description: failed to launch taskkill for worker tree pid={pid}: {error}"
        ));
    }
}

fn temporary_request_path() -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    std::env::temp_dir().join(format!(
        "sonarpad_audio_description_{}_{}.json",
        std::process::id(),
        stamp
    ))
}

pub fn run_audio_description_bridge(
    request: &AudioDescriptionBridgeRequest,
    cancel: Arc<AtomicBool>,
    mut callbacks: AudioDescriptionBridgeCallbacks,
) -> Result<AudioDescriptionBridgeResult, String> {
    let bridge_path = ensure_bridge(&cancel, &mut callbacks.download)?;
    if cancel.load(Ordering::Relaxed) {
        return Err("cancelled".to_string());
    }

    let request_path = temporary_request_path();
    let request_json = serde_json::to_vec(request)
        .map_err(|error| format!("serialize audio-description request failed: {error}"))?;
    fs::write(&request_path, request_json)
        .map_err(|error| format!("write audio-description request failed: {error}"))?;

    let run_result = (|| -> Result<AudioDescriptionBridgeResult, String> {
        let mut child = Command::new(&bridge_path)
            .arg("--request")
            .arg(&request_path)
            .creation_flags(CREATE_NO_WINDOW)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|error| format!("start audio-description worker failed: {error}"))?;

        let mut child_stdin = child
            .stdin
            .take()
            .ok_or_else(|| "audio-description worker stdin unavailable".to_string())?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| "audio-description worker stdout unavailable".to_string())?;
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| "audio-description worker stderr unavailable".to_string())?;
        let stderr_thread = std::thread::spawn(move || {
            const STDERR_TAIL_LINES: usize = 40;
            let mut reader = BufReader::new(stderr);
            let mut raw_line = Vec::new();
            let mut tail = VecDeque::<String>::with_capacity(STDERR_TAIL_LINES);
            loop {
                raw_line.clear();
                match reader.read_until(b'\n', &mut raw_line) {
                    Ok(0) => break,
                    Ok(_) => {
                        let line = decode_bridge_text(&raw_line).trim().to_string();
                        if line.is_empty() {
                            continue;
                        }
                        crate::log_debug(&format!("audio_description.worker {line}"));
                        if tail.len() == STDERR_TAIL_LINES {
                            tail.pop_front();
                        }
                        tail.push_back(line);
                    }
                    Err(error) => {
                        crate::log_debug(&format!(
                            "Audio description: reading worker stderr failed: {error}"
                        ));
                        break;
                    }
                }
            }
            tail.into_iter().collect::<Vec<_>>().join("\n")
        });

        let (line_tx, line_rx) = mpsc::channel::<Result<String, String>>();
        let stdout_thread = std::thread::spawn(move || {
            let mut reader = BufReader::new(stdout);
            let mut raw_line = Vec::new();
            loop {
                raw_line.clear();
                match reader.read_until(b'\n', &mut raw_line) {
                    Ok(0) => break,
                    Ok(_) => {
                        let line = decode_bridge_text(&raw_line).trim().to_string();
                        if line_tx.send(Ok(line)).is_err() {
                            break;
                        }
                    }
                    Err(error) => {
                        if line_tx
                            .send(Err(format!(
                                "read audio-description worker failed: {error}"
                            )))
                            .is_err()
                        {
                            crate::log_debug(
                                "Audio description: worker output receiver closed while reporting a read error",
                            );
                        }
                        break;
                    }
                }
            }
        });

        let mut result: Option<AudioDescriptionBridgeResult> = None;
        loop {
            if cancel.load(Ordering::SeqCst) {
                terminate_bridge_process_tree(&mut child);
                // Dropping the JoinHandles detaches the pipe-reader threads. They
                // will finish when Windows closes the killed worker's pipe handles,
                // while this job thread can notify the UI immediately.
                let _detached_stdout_thread = stdout_thread;
                let _detached_stderr_thread = stderr_thread;
                return Err("cancelled".to_string());
            }
            match line_rx.recv_timeout(Duration::from_millis(100)) {
                Ok(Ok(line)) => {
                    if let Some(raw_pct) = line.strip_prefix("PROGRESS:") {
                        if let Ok(pct) = raw_pct.trim().parse::<i32>()
                            && let Some(callback) = callbacks.progress.as_mut()
                        {
                            callback(pct.clamp(0, 100));
                        }
                    } else if let Some(raw_status) = line.strip_prefix("STATUS:") {
                        if let Ok(status) = serde_json::from_str::<BridgeStatus>(raw_status)
                            && let Some(callback) = callbacks.status.as_mut()
                        {
                            callback(&status.stage, &status.message);
                        }
                    } else if let Some(raw_checkpoint) = line.strip_prefix("CHECKPOINT:") {
                        let checkpoint = serde_json::from_str::<AudioDescriptionBridgeCheckpoint>(
                            raw_checkpoint,
                        )
                        .map_err(|error| {
                            format!(
                                "invalid checkpoint event from audio-description worker: {error}"
                            )
                        })?;
                        if let Some(callback) = callbacks.checkpoint.as_mut() {
                            callback(&checkpoint);
                        }
                    } else if let Some(raw_quota) = line.strip_prefix("QUOTA:") {
                        let quota =
                            serde_json::from_str::<BridgeQuota>(raw_quota).map_err(|error| {
                                format!(
                                    "invalid quota event from audio-description worker: {error}"
                                )
                            })?;
                        let decision = callbacks
                            .quota
                            .as_mut()
                            .map(|callback| callback(&quota.model, &quota.error))
                            .unwrap_or(AudioDescriptionQuotaDecision::Wait);
                        let reply = match decision {
                            AudioDescriptionQuotaDecision::SwitchModel(model) => {
                                serde_json::json!({"action": "switch", "model": model})
                            }
                            AudioDescriptionQuotaDecision::Wait => {
                                serde_json::json!({"action": "wait"})
                            }
                            AudioDescriptionQuotaDecision::Stop => {
                                serde_json::json!({"action": "stop"})
                            }
                        };
                        writeln!(child_stdin, "{reply}").map_err(|error| {
                            format!(
                                "write quota decision to audio-description worker failed: {error}"
                            )
                        })?;
                        child_stdin.flush().map_err(|error| {
                            format!(
                                "flush quota decision to audio-description worker failed: {error}"
                            )
                        })?;
                    } else if let Some(raw_overload) = line.strip_prefix("OVERLOAD:") {
                        let overload = serde_json::from_str::<BridgeOverload>(raw_overload)
                            .map_err(|error| {
                                format!(
                                    "invalid overload event from audio-description worker: {error}"
                                )
                            })?;
                        crate::log_debug(&format!(
                            "audio_description.overload model={} error={}",
                            overload.model, overload.error
                        ));
                        let decision = callbacks
                            .overload
                            .as_mut()
                            .map(|callback| callback(&overload.model, &overload.error))
                            .unwrap_or(AudioDescriptionOverloadDecision::Wait);
                        let reply = match decision {
                            AudioDescriptionOverloadDecision::Wait => {
                                crate::log_debug(
                                    "audio_description.overload decision=wait_forever",
                                );
                                serde_json::json!({"action": "wait"})
                            }
                            AudioDescriptionOverloadDecision::Stop => {
                                crate::log_debug("audio_description.overload decision=stop");
                                serde_json::json!({"action": "stop"})
                            }
                        };
                        writeln!(child_stdin, "{reply}").map_err(|error| {
                            format!(
                                "write overload decision to audio-description worker failed: {error}"
                            )
                        })?;
                        child_stdin.flush().map_err(|error| {
                            format!(
                                "flush overload decision to audio-description worker failed: {error}"
                            )
                        })?;
                    } else if let Some(raw_result) = line.strip_prefix("RESULT:") {
                        result = serde_json::from_str(raw_result).ok();
                    } else if !line.is_empty() {
                        crate::log_debug(&format!("Audio description worker: {line}"));
                    }
                }
                Ok(Err(error)) => {
                    crate::log_if_err!(
                        child.kill(),
                        "Audio description: worker kill after stdout read error failed"
                    );
                    crate::log_if_err!(
                        child.wait(),
                        "Audio description: worker wait after stdout read error failed"
                    );
                    if stdout_thread.join().is_err() {
                        crate::log_debug(
                            "Audio description: stdout reader thread panicked after worker read error",
                        );
                    }
                    if stderr_thread.join().is_err() {
                        crate::log_debug(
                            "Audio description: stderr reader thread panicked after worker read error",
                        );
                    }
                    return Err(error);
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
            }
        }

        if stdout_thread.join().is_err() {
            crate::log_debug("Audio description: stdout reader thread panicked");
        }
        let status = child
            .wait()
            .map_err(|error| format!("wait audio-description worker failed: {error}"))?;
        let stderr_text = stderr_thread.join().unwrap_or_default();
        let output = result.ok_or_else(|| {
            if stderr_text.trim().is_empty() {
                format!("audio-description worker returned no result ({status})")
            } else {
                format!(
                    "audio-description worker returned no result ({status}): {}",
                    stderr_text.trim()
                )
            }
        })?;
        if output.cancelled {
            return Err("cancelled".to_string());
        }
        if !output.ok {
            let mut error = if output.error.trim().is_empty() {
                format!("audio-description analysis failed ({status})")
            } else {
                output.error.clone()
            };
            if !output.log_path.trim().is_empty() {
                error.push_str(&format!("\nLog: {}", output.log_path));
            }
            return Err(error);
        }
        if output.chunk_duration_sec != 180 {
            return Err(format!(
                "audio-description worker returned unsupported chunk duration: {}",
                output.chunk_duration_sec
            ));
        }
        if output.analysis_engine != "pyannote-segmentation-onnx" {
            return Err(format!(
                "audio-description worker returned unexpected analysis engine: {}",
                output.analysis_engine
            ));
        }
        Ok(output)
    })();

    crate::log_if_err!(
        fs::remove_file(&request_path),
        "Audio description: remove temporary worker request failed"
    );
    run_result
}
