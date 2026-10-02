use crate::settings::Language;
use encoding_rs::WINDOWS_1252;
use reqwest::blocking::Client;
use reqwest::header::USER_AGENT;
use serde::Deserialize;
use std::io::{BufRead, BufReader, Read};
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};
use std::{fs, io::Write};
use windows::Win32::System::SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX};

const CREATE_NO_WINDOW: u32 = 0x0800_0000;
#[cfg(debug_assertions)]
const BRIDGE_FILE_NAME: &str = "faster_whisper_bridge.exe";
const BRIDGE_CACHE_FILE_NAME: &str = "faster_whisper_bridge_v2.exe";
const BRIDGE_MIN_VALID_SIZE_BYTES: u64 = 1_000_000;
const CUDA_PACKAGE_FILE_NAME: &str = "whisper-cuda-runtime-win64-cu12.zip";
const DICTATION_WORKER_IDLE_TIMEOUT: Duration = Duration::from_secs(5 * 60);
const DICTATION_WORKER_STARTUP_TIMEOUT: Duration = Duration::from_secs(60);
const MIN_SHARED_WORKER_RAM_BYTES: u64 = 4 * 1024 * 1024 * 1024;
const BRIDGE_DOWNLOAD_URLS: [&str; 2] = [
    "https://github.com/Ambro86/Sonarpad-Tools/releases/download/0.7/faster_whisper_bridge.exe",
    "https://github.com/Ambro86/Sonarpad-Tools/releases/download/v0.7/faster_whisper_bridge.exe",
];
const CUDA_PACKAGE_URLS: [&str; 2] = [
    "https://github.com/Ambro86/Sonarpad-Tools/releases/download/0.7/whisper-cuda-runtime-win64-cu12.zip",
    "https://github.com/Ambro86/Sonarpad-Tools/releases/download/v0.7/whisper-cuda-runtime-win64-cu12.zip",
];
const CUDA_REQUIRED_DLLS: [&str; 3] = ["cublas64_12.dll", "cudart64_12.dll", "cudnn64_9.dll"];

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum BridgeModel {
    Small,
    Medium,
    LargeV3,
}

impl BridgeModel {
    fn as_name(self) -> &'static str {
        match self {
            BridgeModel::Small => "small",
            BridgeModel::Medium => "medium",
            BridgeModel::LargeV3 => "large-v3",
        }
    }
}

#[derive(Deserialize)]
struct BridgeOutput {
    #[serde(default)]
    ok: bool,
    #[serde(default)]
    ready: bool,
    #[serde(default)]
    text: String,
    #[serde(default)]
    error: String,
}

pub struct BridgeProgressCallbacks {
    pub download: Option<Box<dyn FnMut(i32) + Send>>,
    pub transcription: Option<Box<dyn FnMut(i32) + Send>>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct DictationWorkerConfig {
    model: BridgeModel,
    use_cuda_runtime: bool,
}

struct DictationBridgeWorker {
    child: Child,
    stdin: ChildStdin,
    line_rx: mpsc::Receiver<String>,
    stderr_log: Arc<Mutex<String>>,
    config: DictationWorkerConfig,
    last_used_at: Instant,
}

fn handle_bridge_line(
    line: &str,
    progress: &mut Option<Box<dyn FnMut(i32) + Send>>,
    bridge_result: &mut Option<BridgeOutput>,
) {
    if let Some(raw_pct) = line.strip_prefix("PROGRESS:")
        && let Ok(pct) = raw_pct.trim().parse::<i32>()
    {
        if let Some(cb) = progress.as_mut() {
            cb(pct.clamp(0, 100));
        }
        return;
    }
    if line.starts_with('{')
        && let Ok(parsed) = serde_json::from_str::<BridgeOutput>(line)
    {
        *bridge_result = Some(parsed);
    }
}

fn json_line_ascii(value: &serde_json::Value) -> String {
    let json = value.to_string();
    if json.is_ascii() {
        return json;
    }

    let mut escaped = String::with_capacity(json.len());
    for ch in json.chars() {
        if ch.is_ascii() {
            escaped.push(ch);
        } else {
            let mut units = [0u16; 2];
            for unit in ch.encode_utf16(&mut units).iter() {
                escaped.push_str(&format!(r#"\u{:04x}"#, *unit));
            }
        }
    }
    escaped
}

fn worker_stderr_message(stderr_log: &Arc<Mutex<String>>) -> String {
    match stderr_log.lock() {
        Ok(text) => text.trim().to_string(),
        Err(_) => String::new(),
    }
}

fn bridge_start_io_threads(
    stdout: impl Read + Send + 'static,
    stderr: impl Read + Send + 'static,
) -> (mpsc::Receiver<String>, Arc<Mutex<String>>) {
    let (line_tx, line_rx) = mpsc::channel::<String>();
    let stderr_log = Arc::new(Mutex::new(String::new()));

    std::thread::spawn(move || {
        let mut reader = BufReader::new(stdout);
        let mut line = Vec::new();
        loop {
            line.clear();
            match reader.read_until(b'\n', &mut line) {
                Ok(0) => break,
                Ok(_) => {
                    let trimmed = decode_bridge_text(&line).trim().to_string();
                    crate::log_debug(&format!("Bridge: stdout line '{}'", trimmed));
                    if !trimmed.is_empty() && line_tx.send(trimmed).is_err() {
                        break;
                    }
                }
                Err(err) => {
                    if line_tx.send(format!("ERROR:stdout_read:{err}")).is_err() {
                        crate::log_debug("bridge stdout error channel send failed");
                    }
                    break;
                }
            }
        }
    });

    let stderr_log_copy = stderr_log.clone();
    std::thread::spawn(move || {
        let mut reader = BufReader::new(stderr);
        let mut raw = Vec::new();
        match reader.read_to_end(&mut raw) {
            Ok(_) => {
                let text = decode_bridge_text(&raw);
                if !text.trim().is_empty() {
                    crate::log_debug(&format!("Bridge stderr: {}", text.trim()));
                    if let Ok(mut log) = stderr_log_copy.lock() {
                        *log = text;
                    }
                }
            }
            Err(err) => {
                crate::log_debug(&format!("bridge stderr read failed: {err}"));
            }
        }
    });

    (line_rx, stderr_log)
}

fn close_dictation_worker(worker: &mut DictationBridgeWorker, reason: &str) {
    crate::log_debug(&format!("Dictation bridge worker: closing ({reason})"));
    let shutdown = serde_json::json!({ "command": "shutdown" });
    let shutdown_line = json_line_ascii(&shutdown);
    if let Err(err) = writeln!(worker.stdin, "{shutdown_line}") {
        crate::log_debug(&format!(
            "Dictation bridge worker shutdown write failed: {err}"
        ));
    } else if let Err(err) = worker.stdin.flush() {
        crate::log_debug(&format!(
            "Dictation bridge worker shutdown flush failed: {err}"
        ));
    }
    let shutdown_deadline = Instant::now() + Duration::from_secs(2);
    loop {
        match worker.child.try_wait() {
            Ok(Some(status)) => {
                crate::log_debug(&format!(
                    "Dictation bridge worker: exited gracefully with status {}",
                    status
                ));
                break;
            }
            Ok(None) => {
                if Instant::now() >= shutdown_deadline {
                    if let Err(err) = worker.child.kill() {
                        crate::log_debug(&format!(
                            "Dictation bridge worker forced kill failed: {err}"
                        ));
                    }
                    break;
                }
                std::thread::sleep(Duration::from_millis(50));
            }
            Err(err) => {
                crate::log_debug(&format!("Dictation bridge worker wait failed: {err}"));
                if let Err(kill_err) = worker.child.kill() {
                    crate::log_debug(&format!(
                        "Dictation bridge worker forced kill failed: {kill_err}"
                    ));
                }
                break;
            }
        }
    }
}

fn ensure_dictation_worker_watcher() {
    static WATCHER: OnceLock<()> = OnceLock::new();
    WATCHER.get_or_init(|| {
        std::thread::spawn(|| {
            loop {
                std::thread::sleep(Duration::from_secs(30));
                let slot = dictation_worker_slot();
                let mut guard = match slot.lock() {
                    Ok(guard) => guard,
                    Err(_) => {
                        crate::log_debug("Dictation bridge worker watcher: lock poisoned");
                        continue;
                    }
                };
                let should_close = guard.as_mut().is_some_and(|worker| {
                    worker.last_used_at.elapsed() >= DICTATION_WORKER_IDLE_TIMEOUT
                });
                if should_close && let Some(worker) = guard.as_mut() {
                    close_dictation_worker(worker, "idle_timeout");
                    *guard = None;
                }
            }
        });
    });
}

fn dictation_worker_slot() -> &'static Mutex<Option<DictationBridgeWorker>> {
    static SLOT: OnceLock<Mutex<Option<DictationBridgeWorker>>> = OnceLock::new();
    SLOT.get_or_init(|| Mutex::new(None))
}

fn total_physical_memory_bytes() -> Option<u64> {
    let mut status = MEMORYSTATUSEX {
        dwLength: std::mem::size_of::<MEMORYSTATUSEX>() as u32,
        ..Default::default()
    };
    // SAFETY: `status` points to a properly initialized MEMORYSTATUSEX with correct length.
    unsafe {
        GlobalMemoryStatusEx(&mut status)
            .ok()
            .map(|_| status.ullTotalPhys)
    }
}

fn shared_worker_allowed() -> bool {
    static ALLOWED: OnceLock<bool> = OnceLock::new();
    *ALLOWED.get_or_init(|| {
        let allowed = total_physical_memory_bytes()
            .map(|bytes| bytes >= MIN_SHARED_WORKER_RAM_BYTES)
            .unwrap_or(true);
        if !allowed {
            crate::log_debug("Dictation bridge worker disabled: system RAM below 4 GB");
        }
        allowed
    })
}

pub fn shutdown_shared_worker() {
    let slot = dictation_worker_slot();
    let mut guard = match slot.lock() {
        Ok(guard) => guard,
        Err(_) => {
            crate::log_debug("Dictation bridge worker shutdown: lock poisoned");
            return;
        }
    };
    if let Some(worker) = guard.as_mut() {
        close_dictation_worker(worker, "app_shutdown");
    }
    *guard = None;
}

pub fn prewarm_shared_worker(model: BridgeModel, use_cuda_runtime: bool) {
    if !shared_worker_allowed() {
        return;
    }
    let cancel = Arc::new(AtomicBool::new(false));
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_secs(1));
        let worker_config = DictationWorkerConfig {
            model,
            use_cuda_runtime,
        };
        let slot = dictation_worker_slot();

        let should_start = {
            let mut guard = match slot.lock() {
                Ok(guard) => guard,
                Err(_) => {
                    crate::log_debug("Dictation bridge worker prewarm: lock poisoned");
                    return;
                }
            };

            let mut stale_worker = false;
            if let Some(worker) = guard.as_mut() {
                let wrong_config = worker.config != worker_config;
                let exited = match worker.child.try_wait() {
                    Ok(Some(_)) => true,
                    Ok(None) => false,
                    Err(_) => true,
                };
                stale_worker = wrong_config || exited;
            }
            if stale_worker
                && let Some(mut worker) = guard.take()
                && let Err(err) = worker.child.kill()
            {
                crate::log_debug(&format!(
                    "Dictation bridge worker prewarm stale kill failed: {err}"
                ));
            }

            guard.is_none()
        };

        if !should_start {
            return;
        }

        let mut no_download_progress = None;
        match start_dictation_bridge_worker(
            model,
            use_cuda_runtime,
            &cancel,
            &mut no_download_progress,
        ) {
            Ok(mut worker) => {
                let mut guard = match slot.lock() {
                    Ok(guard) => guard,
                    Err(_) => {
                        crate::log_debug(
                            "Dictation bridge worker prewarm: lock poisoned while storing worker",
                        );
                        close_dictation_worker(&mut worker, "prewarm_lock_poisoned");
                        return;
                    }
                };

                let replace_existing = match guard.as_mut() {
                    Some(existing) => {
                        let wrong_config = existing.config != worker_config;
                        let exited = match existing.child.try_wait() {
                            Ok(Some(_)) => true,
                            Ok(None) => false,
                            Err(_) => true,
                        };
                        wrong_config || exited
                    }
                    None => true,
                };

                if replace_existing {
                    if let Some(mut existing) = guard.take() {
                        close_dictation_worker(&mut existing, "prewarm_replace");
                    }
                    *guard = Some(worker);
                    crate::log_debug("Dictation bridge worker prewarm completed");
                } else {
                    close_dictation_worker(&mut worker, "prewarm_unused");
                }
            }
            Err(err) => {
                crate::log_debug(&format!("Dictation bridge worker prewarm failed: {err}"));
            }
        }
    });
}

fn language_code(language: Language) -> &'static str {
    match language {
        Language::Italian => "it",
        Language::German => "de",
        Language::English => "en",
        Language::Spanish => "es",
        Language::Portuguese | Language::PortugueseBrazilian => "pt",
        Language::Swedish => "sv",
        Language::Vietnamese => "vi",
        Language::Czech => "cs",
        Language::Polish => "pl",
        Language::French => "fr",
        Language::Serbian => "sr",
        Language::Ukrainian => "uk",
        Language::Lithuanian => "lt",
        Language::Russian => "ru",
        Language::Chinese => "zh",
        Language::Hindi => "hi",
    }
}

fn decode_bridge_text(bytes: &[u8]) -> String {
    match std::str::from_utf8(bytes) {
        Ok(text) => text.to_string(),
        Err(_) => {
            let (decoded, _, _) = WINDOWS_1252.decode(bytes);
            decoded.into_owned()
        }
    }
}

fn remove_case_insensitive_all(input: &str, needle: &str) -> String {
    if needle.is_empty() {
        return input.to_string();
    }
    let mut output = input.to_string();
    loop {
        let lower = output.to_ascii_lowercase();
        let Some(pos) = lower.find(needle) else {
            break;
        };
        let end = pos + needle.len();
        output.replace_range(pos..end, "");
    }
    output
}

fn remove_spaced_dot_runs(line: &str) -> String {
    let tokens = line.split_whitespace().collect::<Vec<_>>();
    let mut output = Vec::new();
    let mut index = 0usize;
    while index < tokens.len() {
        if tokens[index] == "." {
            let start = index;
            while index < tokens.len() && tokens[index] == "." {
                index += 1;
            }
            if index - start >= 3 {
                continue;
            }
            output.extend_from_slice(&tokens[start..index]);
        } else {
            output.push(tokens[index]);
            index += 1;
        }
    }
    output.join(" ")
}

fn strip_hallucinations(text: &str) -> String {
    let mut cleaned = text.to_string();

    for needle in [
        "sottotitoli e revisione a cura di qtss",
        "sottotitoli a cura di qtss",
        "sottotitoli di qtss",
        "traduzione a cura di qtss",
        "revisione a cura di qtss",
        "subtitles by qtss",
        "translated by qtss",
        "sottotitoli creati dalla comunità amara.org",
        "sottotitoli creati dalla comunita amara.org",
        "sottotitoli creati dalla comunità amara org",
        "sottotitoli creati dalla comunita amara org",
        "subtitles created by the amara.org community",
        "subtitles created by the amara org community",
        "subtitles by the amara.org community",
        "subtitles by the amara org community",
        "sottotitoli di amara.org",
        "sottotitoli di amara org",
        "subtitles by amara.org",
        "subtitles by amara org",
    ] {
        cleaned = remove_case_insensitive_all(&cleaned, needle);
    }

    let lower = cleaned.to_ascii_lowercase();
    for tail in ["amara.org", "amara org", "qtss.", "qtss"] {
        if let Some(pos) = lower.rfind(tail)
            && lower.len().saturating_sub(pos) <= 96
        {
            cleaned.truncate(pos);
            break;
        }
    }

    cleaned
        .lines()
        .map(remove_spaced_dot_runs)
        .filter(|line| !line.is_empty() && !line.replace('.', "").trim().is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

fn bridge_install_path() -> PathBuf {
    crate::settings::settings_dir()
        .join("tools")
        .join(BRIDGE_CACHE_FILE_NAME)
}

fn cuda_runtime_dir() -> PathBuf {
    crate::settings::settings_dir().join("tools").join("cuda")
}

fn cuda_package_path() -> PathBuf {
    crate::settings::settings_dir()
        .join("tools")
        .join(CUDA_PACKAGE_FILE_NAME)
}

#[cfg(debug_assertions)]
fn repo_dll_dir_candidates() -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    if let Ok(exe_path) = std::env::current_exe()
        && let Some(repo_root) = exe_path
            .parent()
            .and_then(|p| p.parent())
            .and_then(|p| p.parent())
    {
        candidates.push(repo_root.join("dll"));
    }
    if let Ok(cwd) = std::env::current_dir() {
        candidates.push(cwd.join("dll"));
    }
    candidates
}

fn has_required_cuda_runtime(cuda_dir: &Path) -> bool {
    CUDA_REQUIRED_DLLS
        .iter()
        .all(|name| cuda_dir.join(name).is_file())
}

fn is_valid_bridge_exe(path: &Path) -> Result<bool, String> {
    let meta = fs::metadata(path).map_err(|e| format!("bridge stat failed: {e}"))?;
    if meta.len() < BRIDGE_MIN_VALID_SIZE_BYTES {
        return Ok(false);
    }
    let mut file = fs::File::open(path).map_err(|e| format!("bridge open failed: {e}"))?;
    let mut header = [0u8; 2];
    let read = file
        .read(&mut header)
        .map_err(|e| format!("bridge read failed: {e}"))?;
    if read < 2 {
        return Ok(false);
    }
    Ok(header == *b"MZ")
}

fn download_file_from_url(
    url: &str,
    target_path: &Path,
    cancel: &Arc<AtomicBool>,
    user_agent: &str,
    download_progress: &mut Option<Box<dyn FnMut(i32) + Send>>,
) -> Result<(), String> {
    let _power_awake = crate::power_awake::acquire("download");
    let parent = target_path
        .parent()
        .ok_or_else(|| "invalid download target path".to_string())?;
    fs::create_dir_all(parent).map_err(|e| format!("create tools dir failed: {e}"))?;

    let client = Client::builder()
        .connect_timeout(Duration::from_secs(30))
        .timeout(Duration::from_secs(60 * 15))
        .build()
        .map_err(|e| format!("http client build failed: {e}"))?;

    let mut response = client
        .get(url)
        .header(USER_AGENT, user_agent)
        .send()
        .map_err(|e| format!("download request failed: {e}"))?;
    if !response.status().is_success() {
        return Err(format!("download failed: HTTP {}", response.status()));
    }

    let part_path = target_path.with_extension("download.part");
    let mut out =
        fs::File::create(&part_path).map_err(|e| format!("create download .part failed: {e}"))?;
    let mut buf = vec![0u8; 128 * 1024];
    let total_bytes = response.content_length();
    let mut downloaded_bytes: u64 = 0;
    let mut last_pct = -1;
    loop {
        if cancel.load(Ordering::Relaxed) {
            if let Err(err) = fs::remove_file(&part_path) {
                crate::log_debug(&format!("download .part cleanup failed: {err}"));
            }
            return Err("cancelled".to_string());
        }
        let read = response
            .read(&mut buf)
            .map_err(|e| format!("download read failed: {e}"))?;
        if read == 0 {
            break;
        }
        out.write_all(&buf[..read])
            .map_err(|e| format!("download write failed: {e}"))?;
        downloaded_bytes = downloaded_bytes.saturating_add(read as u64);
        if let Some(total) = total_bytes
            && total > 0
        {
            let pct = ((downloaded_bytes.saturating_mul(100)) / total).clamp(0, 100) as i32;
            if pct > last_pct {
                last_pct = pct;
                if let Some(cb) = download_progress.as_mut() {
                    cb(pct);
                }
            }
        }
    }
    out.flush()
        .map_err(|e| format!("download flush failed: {e}"))?;
    fs::rename(&part_path, target_path).map_err(|e| format!("download finalize failed: {e}"))?;
    if let Some(cb) = download_progress.as_mut() {
        cb(100);
    }
    Ok(())
}

fn download_bridge_binary_from_url(
    url: &str,
    target_path: &Path,
    cancel: &Arc<AtomicBool>,
    download_progress: &mut Option<Box<dyn FnMut(i32) + Send>>,
) -> Result<(), String> {
    download_file_from_url(
        url,
        target_path,
        cancel,
        "Sonarpad/faster-whisper-bridge",
        download_progress,
    )
}

fn download_bridge_binary(
    target_path: &Path,
    cancel: &Arc<AtomicBool>,
    download_progress: &mut Option<Box<dyn FnMut(i32) + Send>>,
) -> Result<(), String> {
    let mut last_err = String::new();
    for url in BRIDGE_DOWNLOAD_URLS {
        crate::log_debug(&format!("Transcription: downloading bridge from {}", url));
        match download_bridge_binary_from_url(url, target_path, cancel, download_progress) {
            Ok(()) => match is_valid_bridge_exe(target_path) {
                Ok(true) => return Ok(()),
                Ok(false) => {
                    if let Err(err) = fs::remove_file(target_path) {
                        crate::log_debug(&format!("invalid bridge cleanup failed: {err}"));
                    }
                    last_err =
                        format!("downloaded bridge is not a valid Windows executable from {url}");
                }
                Err(err) => {
                    if let Err(remove_err) = fs::remove_file(target_path) {
                        crate::log_debug(&format!("invalid bridge cleanup failed: {remove_err}"));
                    }
                    last_err = err;
                }
            },
            Err(err) => {
                last_err = err;
            }
        }
    }
    if last_err.is_empty() {
        Err("bridge download failed".to_string())
    } else {
        Err(last_err)
    }
}

fn ensure_bridge_binary(
    cancel: &Arc<AtomicBool>,
    download_progress: &mut Option<Box<dyn FnMut(i32) + Send>>,
) -> Result<PathBuf, String> {
    #[cfg(debug_assertions)]
    {
        for candidate in repo_dll_dir_candidates()
            .into_iter()
            .map(|dir| dir.join(BRIDGE_FILE_NAME))
        {
            if candidate.is_file() {
                match is_valid_bridge_exe(&candidate) {
                    Ok(true) => {
                        crate::log_debug(&format!(
                            "Transcription: using local debug bridge {}",
                            candidate.display()
                        ));
                        return Ok(candidate);
                    }
                    Ok(false) => {
                        crate::log_debug(&format!(
                            "Transcription: local debug bridge invalid {}",
                            candidate.display()
                        ));
                    }
                    Err(err) => {
                        crate::log_debug(&format!(
                            "Transcription: local debug bridge check failed {}: {}",
                            candidate.display(),
                            err
                        ));
                    }
                }
            }
        }
    }

    let bridge_path = bridge_install_path();
    if bridge_path.exists() {
        match is_valid_bridge_exe(&bridge_path) {
            Ok(true) => {
                crate::log_debug(&format!(
                    "Transcription: using cached bridge {}",
                    bridge_path.display()
                ));
                return Ok(bridge_path);
            }
            Ok(false) => {
                crate::log_debug("Transcription: existing bridge invalid, re-downloading");
                if let Err(err) = fs::remove_file(&bridge_path) {
                    crate::log_debug(&format!("invalid bridge removal failed: {err}"));
                }
            }
            Err(err) => {
                crate::log_debug(&format!(
                    "Transcription: existing bridge validation failed: {}",
                    err
                ));
                if let Err(remove_err) = fs::remove_file(&bridge_path) {
                    crate::log_debug(&format!("invalid bridge removal failed: {remove_err}"));
                }
            }
        }
    }
    download_bridge_binary(&bridge_path, cancel, download_progress)?;
    crate::log_debug(&format!(
        "Transcription: bridge ready at {}",
        bridge_path.display()
    ));
    Ok(bridge_path)
}

fn download_cuda_package_from_url(
    url: &str,
    target_path: &Path,
    cancel: &Arc<AtomicBool>,
) -> Result<(), String> {
    let mut progress = None;
    download_file_from_url(
        url,
        target_path,
        cancel,
        "Sonarpad/whisper-cuda-runtime",
        &mut progress,
    )
}

fn download_cuda_package(target_path: &Path, cancel: &Arc<AtomicBool>) -> Result<(), String> {
    let mut last_err = String::new();
    for url in CUDA_PACKAGE_URLS {
        crate::log_debug(&format!(
            "Transcription: downloading CUDA runtime from {}",
            url
        ));
        match download_cuda_package_from_url(url, target_path, cancel) {
            Ok(()) => return Ok(()),
            Err(err) => {
                last_err = err;
            }
        }
    }
    if last_err.is_empty() {
        Err("CUDA package download failed".to_string())
    } else {
        Err(last_err)
    }
}

fn extract_cuda_runtime(zip_path: &Path, target_dir: &Path) -> Result<(), String> {
    fs::create_dir_all(target_dir).map_err(|e| format!("create CUDA dir failed: {e}"))?;
    let file = fs::File::open(zip_path).map_err(|e| format!("open CUDA package failed: {e}"))?;
    let mut archive =
        zip::ZipArchive::new(file).map_err(|e| format!("read CUDA zip archive failed: {e}"))?;

    for i in 0..archive.len() {
        let mut entry = archive
            .by_index(i)
            .map_err(|e| format!("read CUDA zip entry failed: {e}"))?;
        let name = entry.name().to_ascii_lowercase();
        if !name.ends_with(".dll") {
            continue;
        }
        let Some(file_name) = Path::new(entry.name()).file_name() else {
            continue;
        };
        let out_path = target_dir.join(file_name);
        let mut out_file =
            fs::File::create(&out_path).map_err(|e| format!("write CUDA DLL failed: {e}"))?;
        std::io::copy(&mut entry, &mut out_file)
            .map_err(|e| format!("extract CUDA DLL failed: {e}"))?;
    }
    Ok(())
}

fn ensure_cuda_runtime(cancel: &Arc<AtomicBool>) -> Result<PathBuf, String> {
    let cuda_dir = cuda_runtime_dir();
    if has_required_cuda_runtime(&cuda_dir) {
        return Ok(cuda_dir);
    }

    if cuda_dir.exists()
        && let Err(err) = fs::remove_dir_all(&cuda_dir)
    {
        crate::log_debug(&format!(
            "Transcription: failed to clear incomplete CUDA dir {}: {err}",
            cuda_dir.display()
        ));
    }
    fs::create_dir_all(&cuda_dir).map_err(|e| format!("create CUDA dir failed: {e}"))?;

    let package_path = cuda_package_path();
    if !package_path.is_file() {
        download_cuda_package(&package_path, cancel)?;
    }

    extract_cuda_runtime(&package_path, &cuda_dir)?;
    if !has_required_cuda_runtime(&cuda_dir) {
        return Err("CUDA runtime package missing required DLLs".to_string());
    }
    if let Err(err) = fs::remove_file(&package_path) {
        crate::log_debug(&format!(
            "Transcription: failed to remove CUDA package {}: {err}",
            package_path.display()
        ));
    }
    crate::log_debug(&format!(
        "Transcription: CUDA runtime ready at {}",
        cuda_dir.display()
    ));
    Ok(cuda_dir)
}

fn spawn_bridge_process(
    bridge_path: &Path,
    wav_path: &Path,
    model: BridgeModel,
    language: Option<Language>,
    include_timestamps: bool,
    work_dir: &Path,
    cuda_dir: Option<&Path>,
) -> Result<std::process::Child, String> {
    let model_name = model.as_name();
    let download_root = crate::settings::settings_dir()
        .join("models")
        .join("faster-whisper");
    let mut args = vec![
        "--input".to_string(),
        wav_path.display().to_string(),
        "--model".to_string(),
        model_name.to_string(),
        "--download-root".to_string(),
        download_root.display().to_string(),
    ];
    if let Some(language) = language {
        args.push("--language".to_string());
        args.push(language_code(language).to_string());
    }
    if include_timestamps {
        args.push("--timestamps".to_string());
    }

    let mut command = Command::new(bridge_path);
    command.args(&args);

    if let Some(cuda_dir) = cuda_dir {
        let mut path_entries = vec![cuda_dir.display().to_string()];
        if let Some(existing) = std::env::var_os("PATH") {
            path_entries.push(existing.to_string_lossy().to_string());
        }
        command
            .env("PATH", path_entries.join(";"))
            .env("SONARPAD_FORCE_CUDA", "1");
        crate::log_debug(&format!(
            "Transcription: CUDA runtime detected at {}",
            cuda_dir.display()
        ));
    }

    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .env("PYTHONUTF8", "1")
        .env("PYTHONIOENCODING", "utf-8")
        .creation_flags(CREATE_NO_WINDOW)
        .current_dir(work_dir)
        .spawn()
        .map_err(|e| format!("start bridge failed: {e}"))
}

fn spawn_bridge_worker_process(
    bridge_path: &Path,
    model: BridgeModel,
    work_dir: &Path,
    cuda_dir: Option<&Path>,
) -> Result<Child, String> {
    let model_name = model.as_name();
    let download_root = crate::settings::settings_dir()
        .join("models")
        .join("faster-whisper");
    let args = vec![
        "--worker".to_string(),
        "--model".to_string(),
        model_name.to_string(),
        "--download-root".to_string(),
        download_root.display().to_string(),
    ];

    let mut command = Command::new(bridge_path);
    command.args(&args);

    if let Some(cuda_dir) = cuda_dir {
        let mut path_entries = vec![cuda_dir.display().to_string()];
        if let Some(existing) = std::env::var_os("PATH") {
            path_entries.push(existing.to_string_lossy().to_string());
        }
        command
            .env("PATH", path_entries.join(";"))
            .env("SONARPAD_FORCE_CUDA", "1");
        crate::log_debug(&format!(
            "Dictation bridge worker: CUDA runtime detected at {}",
            cuda_dir.display()
        ));
    }

    command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .env("PYTHONUTF8", "1")
        .env("PYTHONIOENCODING", "utf-8")
        .creation_flags(CREATE_NO_WINDOW)
        .current_dir(work_dir)
        .spawn()
        .map_err(|e| format!("start bridge worker failed: {e}"))
}

fn select_and_spawn_bridge(
    wav_path: &Path,
    model: BridgeModel,
    language: Option<Language>,
    include_timestamps: bool,
    cancel: &Arc<AtomicBool>,
    bridge_download_progress: &mut Option<Box<dyn FnMut(i32) + Send>>,
    use_cuda_runtime: bool,
) -> Result<std::process::Child, String> {
    let work_dir = crate::settings::settings_dir();
    let bridge_path = ensure_bridge_binary(cancel, bridge_download_progress)?;
    let cuda_dir = if use_cuda_runtime {
        Some(ensure_cuda_runtime(cancel)?)
    } else {
        None
    };
    spawn_bridge_process(
        &bridge_path,
        wav_path,
        model,
        language,
        include_timestamps,
        &work_dir,
        cuda_dir.as_deref(),
    )
}

fn start_dictation_bridge_worker(
    model: BridgeModel,
    use_cuda_runtime: bool,
    cancel: &Arc<AtomicBool>,
    bridge_download_progress: &mut Option<Box<dyn FnMut(i32) + Send>>,
) -> Result<DictationBridgeWorker, String> {
    ensure_dictation_worker_watcher();
    let work_dir = crate::settings::settings_dir();
    let bridge_path = ensure_bridge_binary(cancel, bridge_download_progress)?;
    let cuda_dir = if use_cuda_runtime {
        Some(ensure_cuda_runtime(cancel)?)
    } else {
        None
    };
    let mut child =
        spawn_bridge_worker_process(&bridge_path, model, &work_dir, cuda_dir.as_deref())?;
    let stdin = child
        .stdin
        .take()
        .ok_or_else(|| "bridge worker stdin unavailable".to_string())?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "bridge worker stdout unavailable".to_string())?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| "bridge worker stderr unavailable".to_string())?;
    let (line_rx, stderr_log) = bridge_start_io_threads(stdout, stderr);

    let start_deadline = Instant::now() + DICTATION_WORKER_STARTUP_TIMEOUT;
    loop {
        if cancel.load(Ordering::Relaxed) {
            if let Err(err) = child.kill() {
                crate::log_debug(&format!("bridge worker kill failed during startup: {err}"));
            }
            return Err("cancelled".to_string());
        }
        match line_rx.recv_timeout(Duration::from_millis(150)) {
            Ok(line) => {
                if line.starts_with('{')
                    && let Ok(parsed) = serde_json::from_str::<BridgeOutput>(&line)
                    && parsed.ready
                {
                    crate::log_debug("Dictation bridge worker: ready");
                    return Ok(DictationBridgeWorker {
                        child,
                        stdin,
                        line_rx,
                        stderr_log,
                        config: DictationWorkerConfig {
                            model,
                            use_cuda_runtime,
                        },
                        last_used_at: Instant::now(),
                    });
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                let stderr_text = worker_stderr_message(&stderr_log);
                let status = match child.try_wait() {
                    Ok(Some(status)) => status.to_string(),
                    Ok(None) => "running".to_string(),
                    Err(err) => format!("status unavailable: {err}"),
                };
                return Err(format!(
                    "bridge worker startup failed (status={status}){}",
                    if stderr_text.is_empty() {
                        String::new()
                    } else {
                        format!(": {}", stderr_text)
                    }
                ));
            }
        }

        if Instant::now() >= start_deadline {
            if let Err(err) = child.kill() {
                crate::log_debug(&format!("bridge worker timeout kill failed: {err}"));
            }
            let stderr_text = worker_stderr_message(&stderr_log);
            return Err(if stderr_text.is_empty() {
                "bridge worker startup timed out".to_string()
            } else {
                format!("bridge worker startup timed out: {}", stderr_text)
            });
        }
    }
}

pub fn transcribe_wav(
    wav_path: &Path,
    model: BridgeModel,
    language: Option<Language>,
    include_timestamps: bool,
    use_cuda_runtime: bool,
    cancel: &Arc<AtomicBool>,
    mut progress_callbacks: BridgeProgressCallbacks,
) -> Result<String, String> {
    let _power_awake = crate::power_awake::acquire("media-transcription");
    crate::log_debug(&format!(
        "Bridge: transcribe_wav start wav={} model={} cuda={} timestamps={} language={}",
        wav_path.display(),
        model.as_name(),
        use_cuda_runtime,
        include_timestamps,
        language.map(language_code).unwrap_or("auto")
    ));
    let mut child = select_and_spawn_bridge(
        wav_path,
        model,
        language,
        include_timestamps,
        cancel,
        &mut progress_callbacks.download,
        use_cuda_runtime,
    )?;
    crate::log_debug("Bridge: process spawned");

    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "bridge stdout unavailable".to_string())?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| "bridge stderr unavailable".to_string())?;

    let (line_tx, line_rx) = mpsc::channel::<String>();
    let stdout_thread = std::thread::spawn(move || {
        let mut reader = BufReader::new(stdout);
        let mut line = Vec::new();
        loop {
            line.clear();
            match reader.read_until(b'\n', &mut line) {
                Ok(0) => break,
                Ok(_) => {
                    let trimmed = decode_bridge_text(&line).trim().to_string();
                    crate::log_debug(&format!("Bridge: stdout line '{}'", trimmed));
                    if !trimmed.is_empty() && line_tx.send(trimmed).is_err() {
                        break;
                    }
                }
                Err(err) => {
                    let _ignore = line_tx.send(format!("ERROR:stdout_read:{err}"));
                    break;
                }
            }
        }
    });

    let (err_tx, err_rx) = mpsc::channel::<String>();
    let stderr_thread = std::thread::spawn(move || {
        let mut reader = BufReader::new(stderr);
        let mut raw = Vec::new();
        let text = if let Err(err) = reader.read_to_end(&mut raw) {
            format!("stderr read failed: {err}")
        } else {
            decode_bridge_text(&raw)
        };
        if err_tx.send(text).is_err() {
            crate::log_debug("bridge stderr channel send failed");
        }
    });

    let mut bridge_result: Option<BridgeOutput> = None;
    let mut cancelled = false;

    loop {
        if cancel.load(Ordering::Relaxed) {
            cancelled = true;
            crate::log_debug("Bridge: cancellation requested, killing process");
            if let Err(err) = child.kill() {
                crate::log_debug(&format!("bridge kill failed: {err}"));
            }
        }

        match line_rx.recv_timeout(Duration::from_millis(150)) {
            Ok(line) => {
                crate::log_debug("Bridge: received stdout payload");
                handle_bridge_line(
                    &line,
                    &mut progress_callbacks.transcription,
                    &mut bridge_result,
                );
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                crate::log_debug("Bridge: stdout channel disconnected");
            }
        }

        match child.try_wait() {
            Ok(Some(status)) => {
                crate::log_debug(&format!("Bridge: process exited with status {}", status));
                break;
            }
            Ok(None) => {}
            Err(err) => return Err(format!("bridge wait failed: {err}")),
        }
    }

    crate::log_debug("Bridge: joining stdout thread");
    if let Err(_panic) = stdout_thread.join() {
        return Err("bridge stdout thread panicked".to_string());
    }
    while let Ok(line) = line_rx.try_recv() {
        crate::log_debug("Bridge: draining pending stdout payload");
        handle_bridge_line(
            &line,
            &mut progress_callbacks.transcription,
            &mut bridge_result,
        );
    }
    crate::log_debug("Bridge: joining stderr thread");
    if let Err(_panic) = stderr_thread.join() {
        return Err("bridge stderr thread panicked".to_string());
    }
    let stderr_text = err_rx.recv().unwrap_or_default();
    if !stderr_text.trim().is_empty() {
        crate::log_debug(&format!("Bridge: stderr '{}'", stderr_text.trim()));
    }

    if cancelled || cancel.load(Ordering::Relaxed) {
        crate::log_debug("Bridge: returning cancelled");
        return Err("cancelled".to_string());
    }

    if let Some(result) = bridge_result {
        if result.ok {
            crate::log_debug("Bridge: returning successful transcript");
            return Ok(strip_hallucinations(&result.text));
        }
        if !result.error.trim().is_empty() {
            crate::log_debug(&format!("Bridge: returning error '{}'", result.error));
            return Err(result.error);
        }
    }

    if stderr_text.trim().is_empty() {
        crate::log_debug("Bridge: returning no transcript error");
        Err("bridge returned no transcript".to_string())
    } else {
        crate::log_debug("Bridge: returning stderr as error");
        Err(stderr_text)
    }
}

pub fn transcribe_wav_with_shared_worker(
    wav_path: &Path,
    model: BridgeModel,
    language: Option<Language>,
    include_timestamps: bool,
    use_cuda_runtime: bool,
    cancel: &Arc<AtomicBool>,
    mut progress_callbacks: BridgeProgressCallbacks,
) -> Result<String, String> {
    let _power_awake = crate::power_awake::acquire("media-transcription");
    if !shared_worker_allowed() {
        return transcribe_wav(
            wav_path,
            model,
            language,
            include_timestamps,
            use_cuda_runtime,
            cancel,
            progress_callbacks,
        );
    }
    let worker_config = DictationWorkerConfig {
        model,
        use_cuda_runtime,
    };
    let slot = dictation_worker_slot();
    let mut guard = slot
        .lock()
        .map_err(|_| "bridge worker lock poisoned".to_string())?;

    let mut stale_worker = false;
    if let Some(worker) = guard.as_mut() {
        let wrong_config = worker.config != worker_config;
        let exited = match worker.child.try_wait() {
            Ok(Some(status)) => {
                crate::log_debug(&format!(
                    "Dictation bridge worker: existing process exited with status {}",
                    status
                ));
                true
            }
            Ok(None) => false,
            Err(err) => {
                crate::log_debug(&format!("Dictation bridge worker: try_wait failed: {err}"));
                true
            }
        };
        stale_worker = wrong_config || exited;
    }

    if stale_worker
        && let Some(mut worker) = guard.take()
        && let Err(err) = worker.child.kill()
    {
        crate::log_debug(&format!(
            "Dictation bridge worker: stale kill failed: {err}"
        ));
    }

    if guard.is_none() {
        match start_dictation_bridge_worker(
            model,
            use_cuda_runtime,
            cancel,
            &mut progress_callbacks.download,
        ) {
            Ok(worker) => {
                *guard = Some(worker);
            }
            Err(err) => {
                crate::log_debug(&format!(
                    "Dictation bridge worker unavailable, falling back to one-shot bridge: {}",
                    err
                ));
                return transcribe_wav(
                    wav_path,
                    model,
                    language,
                    include_timestamps,
                    use_cuda_runtime,
                    cancel,
                    progress_callbacks,
                );
            }
        }
    }

    let worker = match guard.as_mut() {
        Some(worker) => worker,
        None => return Err("bridge worker unavailable".to_string()),
    };
    worker.last_used_at = Instant::now();

    let request = serde_json::json!({
        "command": "transcribe",
        "input": wav_path.display().to_string(),
        "language": language.map(language_code).unwrap_or(""),
        "timestamps": include_timestamps,
    });
    let request_line = json_line_ascii(&request);
    crate::log_debug(&format!(
        "Dictation bridge worker request: {}",
        request_line
    ));
    if let Err(err) = writeln!(worker.stdin, "{request_line}") {
        crate::log_debug(&format!(
            "Dictation bridge worker write failed, falling back to one-shot bridge: {}",
            err
        ));
        if let Err(kill_err) = worker.child.kill() {
            crate::log_debug(&format!("Dictation bridge worker kill failed: {kill_err}"));
        }
        *guard = None;
        return transcribe_wav(
            wav_path,
            model,
            language,
            include_timestamps,
            use_cuda_runtime,
            cancel,
            progress_callbacks,
        );
    }
    if let Err(err) = worker.stdin.flush() {
        crate::log_debug(&format!(
            "Dictation bridge worker flush failed, falling back to one-shot bridge: {}",
            err
        ));
        if let Err(kill_err) = worker.child.kill() {
            crate::log_debug(&format!("Dictation bridge worker kill failed: {kill_err}"));
        }
        *guard = None;
        return transcribe_wav(
            wav_path,
            model,
            language,
            include_timestamps,
            use_cuda_runtime,
            cancel,
            progress_callbacks,
        );
    }

    let mut bridge_result: Option<BridgeOutput> = None;
    loop {
        if cancel.load(Ordering::Relaxed) {
            crate::log_debug("Dictation bridge worker: cancellation requested, killing worker");
            if let Err(err) = worker.child.kill() {
                crate::log_debug(&format!("Dictation bridge worker kill failed: {err}"));
            }
            *guard = None;
            return Err("cancelled".to_string());
        }

        match worker.line_rx.recv_timeout(Duration::from_millis(150)) {
            Ok(line) => {
                handle_bridge_line(
                    &line,
                    &mut progress_callbacks.transcription,
                    &mut bridge_result,
                );
                if let Some(result) = bridge_result.take() {
                    if result.ready {
                        continue;
                    }
                    if result.ok {
                        worker.last_used_at = Instant::now();
                        return Ok(strip_hallucinations(&result.text));
                    }
                    let err = if result.error.trim().is_empty() {
                        "bridge worker returned no transcript".to_string()
                    } else {
                        result.error
                    };
                    return Err(err);
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                let stderr_text = worker_stderr_message(&worker.stderr_log);
                *guard = None;
                return Err(if stderr_text.is_empty() {
                    "bridge worker disconnected".to_string()
                } else {
                    stderr_text
                });
            }
        }

        match worker.child.try_wait() {
            Ok(Some(status)) => {
                let stderr_text = worker_stderr_message(&worker.stderr_log);
                *guard = None;
                return Err(if stderr_text.is_empty() {
                    format!("bridge worker exited with status {}", status)
                } else {
                    stderr_text
                });
            }
            Ok(None) => {}
            Err(err) => {
                *guard = None;
                return Err(format!("bridge worker wait failed: {err}"));
            }
        }
    }
}
