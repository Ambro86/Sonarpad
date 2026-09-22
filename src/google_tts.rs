use crate::settings::{Language, VoiceInfo};
use base64::Engine;
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio_tungstenite::tungstenite::{self, Message, WebSocket, connect, stream::MaybeTlsStream};

const SAMPLE_RATE: u32 = 24_000;
const CREATE_NO_WINDOW: u32 = 0x0800_0000;
const BINDING_NAME: &str = "googleTtsForSonarpadBridge";
const STOP_EXPRESSION: &str =
    "window.googleTtsForSonarpadStop && window.googleTtsForSonarpadStop()";
const VOICE_SEPARATOR: char = '\u{001f}';
static PROFILE_SEQUENCE: AtomicU64 = AtomicU64::new(1);
type CdpEventHandler<'a> = Option<&'a mut dyn FnMut(&Value) -> Result<(), String>>;

fn runtime_start_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

fn lock_runtime_start(
    cancel: &Arc<AtomicBool>,
) -> Result<std::sync::MutexGuard<'static, ()>, String> {
    loop {
        match runtime_start_lock().try_lock() {
            Ok(guard) => return Ok(guard),
            Err(std::sync::TryLockError::WouldBlock) => {
                if cancel.load(Ordering::Relaxed) {
                    return Err("cancelled".to_string());
                }
                thread::sleep(Duration::from_millis(50));
            }
            Err(std::sync::TryLockError::Poisoned(error)) => return Ok(error.into_inner()),
        }
    }
}

/// Google/NVDA pitch values are exposed to users as an exact 0..=100 scale.
/// We encode new values outside the legacy -12..=12 semitone range so old
/// settings and document voice tags remain readable without ambiguity.
pub const GOOGLE_PITCH_ENCODE_BASE: i32 = 1000;

pub fn google_pitch_percent_to_internal(percent: i32) -> i32 {
    GOOGLE_PITCH_ENCODE_BASE + percent.clamp(0, 100)
}

fn google_pitch_percent_from_legacy_semitones(semitones: i32) -> i32 {
    let normalized = (semitones.clamp(-12, 12) + 12) as f64 / 24.0;
    (normalized * 100.0).round().clamp(0.0, 100.0) as i32
}

pub fn google_pitch_percent_from_internal(value: i32) -> i32 {
    if (GOOGLE_PITCH_ENCODE_BASE..=GOOGLE_PITCH_ENCODE_BASE + 100).contains(&value) {
        value - GOOGLE_PITCH_ENCODE_BASE
    } else {
        // Legacy Sonarpad values used semitones from -12 to +12.
        google_pitch_percent_from_legacy_semitones(value)
    }
}

pub fn normalize_google_pitch_internal(value: i32) -> i32 {
    google_pitch_percent_to_internal(google_pitch_percent_from_internal(value))
}

pub fn google_pitch_preset_internal(legacy_semitones: i32) -> i32 {
    google_pitch_percent_to_internal(google_pitch_percent_from_legacy_semitones(legacy_semitones))
}

const INDEX_HTML: &[u8] = include_bytes!("../assets/google_tts/web/index.html");
const BRIDGE_HARNESS_JS: &[u8] = include_bytes!("../assets/google_tts/web/bridgeHarness.js");
const BACKGROUND_COMPILED_JS: &[u8] =
    include_bytes!("../assets/google_tts/engine/background_compiled.js");
const BINDINGS_MAIN_JS: &[u8] = include_bytes!("../assets/google_tts/engine/bindings_main.js");
const BINDINGS_MAIN_WASM: &[u8] = include_bytes!("../assets/google_tts/engine/bindings_main.wasm");
const ENGINE_MANIFEST_JSON: &[u8] = include_bytes!("../assets/google_tts/engine/manifest.json");
const OFFSCREEN_HTML: &[u8] = include_bytes!("../assets/google_tts/engine/offscreen.html");
const OFFSCREEN_COMPILED_JS: &[u8] =
    include_bytes!("../assets/google_tts/engine/offscreen_compiled.js");
const STREAMING_WORKLET_JS: &[u8] =
    include_bytes!("../assets/google_tts/engine/streaming_worklet_processor.js");
const CATALOG_JSON: &[u8] = include_bytes!("../assets/google_tts/engine/voices.json");
const WASM_MANIFEST_JSON: &[u8] =
    include_bytes!("../assets/google_tts/engine/wasm_tts_manifest_v3.json");

#[derive(Clone, Deserialize, Serialize)]
pub struct GoogleVoicePackage {
    pub id: String,
    #[serde(rename = "fileId")]
    pub file_id: String,
    pub url: String,
    #[serde(rename = "sha256Checksum")]
    pub sha256_checksum: String,
    #[serde(rename = "compressedSize")]
    pub compressed_size: u64,
    #[serde(default)]
    pub speakers: Vec<GoogleSpeakerRaw>,
    #[serde(default, rename = "dependentVoiceId")]
    pub dependent_voice_id: Option<String>,
}

#[derive(Clone, Deserialize, Serialize)]
pub struct GoogleSpeakerRaw {
    #[serde(default)]
    pub speaker: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub gender: String,
}

#[derive(Clone)]
pub struct GoogleSpeaker {
    pub id: String,
    pub name: String,
    pub language: String,
}

#[derive(Clone)]
pub struct GoogleVoicePackageStatus {
    pub package: GoogleVoicePackage,
    pub language: String,
    pub installed: bool,
}

fn catalog_cache() -> &'static Vec<GoogleVoicePackage> {
    static CATALOG: OnceLock<Vec<GoogleVoicePackage>> = OnceLock::new();
    CATALOG.get_or_init(|| match serde_json::from_slice(CATALOG_JSON) {
        Ok(packages) => packages,
        Err(err) => {
            crate::log_debug(&format!("Google TTS catalog parse failed: {err}"));
            Vec::new()
        }
    })
}

pub fn catalog_packages() -> Vec<GoogleVoicePackageStatus> {
    catalog_cache()
        .iter()
        .cloned()
        .map(|package| GoogleVoicePackageStatus {
            language: package_language(&package.id),
            installed: is_package_ready(&package),
            package,
        })
        .collect()
}

fn package_language(package_id: &str) -> String {
    let mut parts = package_id.split('-');
    let language = parts.next().unwrap_or(package_id).to_ascii_lowercase();
    let region = parts.next().unwrap_or_default().to_ascii_uppercase();
    if region.len() == 2 {
        format!("{language}-{region}")
    } else {
        language
    }
}

fn google_data_dir() -> PathBuf {
    crate::settings::settings_dir().join("google_tts")
}

pub fn voice_dir() -> PathBuf {
    google_data_dir().join("voices")
}

fn runtime_dir() -> PathBuf {
    google_data_dir().join("runtime")
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum BrowserRuntime {
    Chrome,
    Edge,
}

impl BrowserRuntime {
    fn display_name(self) -> &'static str {
        match self {
            Self::Chrome => "Google Chrome",
            Self::Edge => "Microsoft Edge",
        }
    }

    fn executable_name(self) -> &'static str {
        match self {
            Self::Chrome => "chrome.exe",
            Self::Edge => "msedge.exe",
        }
    }

    fn environment_variable(self) -> &'static str {
        match self {
            Self::Chrome => "CHROME_PATH",
            Self::Edge => "EDGE_PATH",
        }
    }

    fn profile_directory_name(self) -> &'static str {
        match self {
            Self::Chrome => "chrome_profiles",
            Self::Edge => "edge_profiles",
        }
    }
}

struct BrowserExecutable {
    runtime: BrowserRuntime,
    path: PathBuf,
}

fn browser_profiles_dir(runtime: BrowserRuntime) -> PathBuf {
    google_data_dir().join(runtime.profile_directory_name())
}

fn package_path(package: &GoogleVoicePackage) -> PathBuf {
    voice_dir().join(format!("{}.zvoice", package.id))
}

fn verification_cache() -> &'static Mutex<HashMap<String, (u64, SystemTime)>> {
    static CACHE: OnceLock<Mutex<HashMap<String, (u64, SystemTime)>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn clear_verification_cache(package_id: &str) {
    match verification_cache().lock() {
        Ok(mut cache) => {
            cache.retain(|id, _| id != package_id);
        }
        Err(_) => crate::log_debug("Google TTS verification cache lock poisoned"),
    }
}

fn file_sha256(path: &Path) -> Result<String, String> {
    let mut file = fs::File::open(path).map_err(|err| err.to_string())?;
    let mut digest = Sha256::new();
    let mut buffer = vec![0u8; 1024 * 1024];
    loop {
        let read = file.read(&mut buffer).map_err(|err| err.to_string())?;
        if read == 0 {
            break;
        }
        digest.update(&buffer[..read]);
    }
    Ok(hex::encode(digest.finalize()))
}

pub fn is_package_installed(package: &GoogleVoicePackage) -> bool {
    let path = package_path(package);
    let Ok(metadata) = fs::metadata(&path) else {
        clear_verification_cache(&package.id);
        return false;
    };
    let size = metadata.len();
    if package.compressed_size > 0 && size != package.compressed_size {
        clear_verification_cache(&package.id);
        return false;
    }
    if package.sha256_checksum.trim().is_empty() {
        return true;
    }
    let modified = metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH);
    let cache_key = (size, modified);
    match verification_cache().lock() {
        Ok(cache) if cache.get(&package.id) == Some(&cache_key) => return true,
        Ok(_) => {}
        Err(_) => crate::log_debug("Google TTS verification cache lock poisoned"),
    }
    match file_sha256(&path) {
        Ok(actual) if actual.eq_ignore_ascii_case(&package.sha256_checksum) => {
            match verification_cache().lock() {
                Ok(mut cache) => {
                    cache.extend(std::iter::once((package.id.clone(), cache_key)));
                }
                Err(_) => crate::log_debug("Google TTS verification cache lock poisoned"),
            }
            true
        }
        Ok(_) => {
            clear_verification_cache(&package.id);
            false
        }
        Err(err) => {
            clear_verification_cache(&package.id);
            crate::log_debug(&format!(
                "Google TTS voice verification failed for {}: {err}",
                package.id
            ));
            false
        }
    }
}

fn package_by_id(package_id: &str) -> Option<&'static GoogleVoicePackage> {
    catalog_cache()
        .iter()
        .find(|package| package.id == package_id)
}

fn is_package_ready(package: &GoogleVoicePackage) -> bool {
    if !is_package_installed(package) {
        return false;
    }
    package
        .dependent_voice_id
        .as_deref()
        .and_then(package_by_id)
        .is_none_or(is_package_installed)
}

fn collect_download_plan(
    package_id: &str,
    visited: &mut HashSet<String>,
    plan: &mut Vec<GoogleVoicePackage>,
) -> Result<(), String> {
    if !visited.insert(package_id.to_string()) {
        return Ok(());
    }
    let package = package_by_id(package_id)
        .cloned()
        .ok_or_else(|| format!("Unknown Google TTS voice package: {package_id}"))?;
    if let Some(dependency_id) = package.dependent_voice_id.as_deref() {
        collect_download_plan(dependency_id, visited, plan)?;
    }
    plan.push(package);
    Ok(())
}

fn speaker_from_package(package: &GoogleVoicePackage) -> Vec<GoogleSpeaker> {
    let language = package_language(&package.id);
    package
        .speakers
        .iter()
        .map(|speaker| {
            let speaker_name = if speaker.name.trim().is_empty() {
                if speaker.speaker.trim().is_empty() {
                    package.id.clone()
                } else {
                    speaker.speaker.clone()
                }
            } else {
                speaker.name.clone()
            };
            GoogleSpeaker {
                id: format!("{}:{}", package.id, speaker.speaker),
                name: speaker_name,
                language: language.clone(),
            }
        })
        .collect()
}

fn all_installed_speakers() -> Vec<GoogleSpeaker> {
    catalog_cache()
        .iter()
        .filter(|package| is_package_ready(package))
        .flat_map(speaker_from_package)
        .collect()
}

pub fn stored_voice_value(speaker: &GoogleSpeaker) -> String {
    format!("{}{}{}", speaker.id, VOICE_SEPARATOR, speaker.name)
}

pub fn voice_display_name(stored: &str) -> String {
    stored
        .split_once(VOICE_SEPARATOR)
        .map(|(_, name)| name.to_string())
        .unwrap_or_else(|| stored.to_string())
}

pub fn voice_id_from_stored(stored: &str) -> &str {
    stored
        .split_once(VOICE_SEPARATOR)
        .map(|(id, _)| id)
        .unwrap_or(stored)
}

pub fn installed_voices() -> Vec<VoiceInfo> {
    all_installed_speakers()
        .into_iter()
        .map(|speaker| VoiceInfo {
            short_name: stored_voice_value(&speaker),
            locale: speaker.language.clone(),
            is_multilingual: false,
        })
        .collect()
}

pub fn has_installed_voices() -> bool {
    catalog_cache().iter().any(is_package_ready)
}

pub fn remove_package(package_id: &str, language: Language) -> Result<(), String> {
    let package = package_by_id(package_id)
        .ok_or_else(|| format!("Unknown Google TTS voice package: {package_id}"))?;
    let required_by: Vec<&str> = catalog_cache()
        .iter()
        .filter(|candidate| {
            candidate.dependent_voice_id.as_deref() == Some(package_id)
                && is_package_installed(candidate)
        })
        .map(|candidate| candidate.id.as_str())
        .collect();
    if !required_by.is_empty() {
        let packages = required_by.join(", ");
        return Err(crate::i18n::tr_f(
            language,
            "google_tts.voices.required_by",
            &[("packages", &packages)],
        ));
    }
    let path = package_path(package);
    match fs::remove_file(&path) {
        Ok(()) => {
            clear_verification_cache(package_id);
            restart_runtime();
            Ok(())
        }
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(err) => Err(format!("Failed to remove {}: {err}", path.display())),
    }
}

fn write_download_body(
    response: &mut reqwest::blocking::Response,
    temporary: &Path,
    total: Option<u64>,
    cancel: &Arc<AtomicBool>,
    progress: &mut impl FnMut(i32),
) -> Result<(), String> {
    let mut output = fs::File::create(temporary).map_err(|err| err.to_string())?;
    let mut downloaded = 0u64;
    let mut last_progress = -1;
    let mut buffer = vec![0u8; 256 * 1024];
    loop {
        if cancel.load(Ordering::Relaxed) {
            return Err("cancelled".to_string());
        }
        let read = response.read(&mut buffer).map_err(|err| err.to_string())?;
        if read == 0 {
            break;
        }
        output
            .write_all(&buffer[..read])
            .map_err(|err| err.to_string())?;
        downloaded = downloaded.saturating_add(read as u64);
        if let Some(total) = total {
            let percentage = ((downloaded.saturating_mul(100)) / total).min(99) as i32;
            if percentage > last_progress {
                last_progress = percentage;
                progress(percentage);
            }
        }
    }
    output.flush().map_err(|err| err.to_string())?;
    Ok(())
}

fn download_single_package(
    package: &GoogleVoicePackage,
    client: &Client,
    cancel: &Arc<AtomicBool>,
    mut progress: impl FnMut(i32),
) -> Result<(), String> {
    if is_package_installed(package) {
        progress(100);
        return Ok(());
    }
    let target = package_path(package);
    let parent = target
        .parent()
        .ok_or_else(|| "Invalid Google TTS voice path".to_string())?;
    fs::create_dir_all(parent).map_err(|err| err.to_string())?;
    let temporary = target.with_extension("zvoice.download");
    if temporary.exists()
        && let Err(err) = fs::remove_file(&temporary)
    {
        crate::log_debug(&format!(
            "Google TTS stale download cleanup failed for {}: {err}",
            temporary.display()
        ));
    }

    crate::log_debug(&format!(
        "Google TTS: downloading package {} from {}",
        package.id, package.url
    ));
    let mut response = client
        .get(&package.url)
        .header("User-Agent", "Sonarpad Google TTS")
        .send()
        .map_err(|err| err.to_string())?;
    if !response.status().is_success() {
        return Err(format!(
            "Google TTS download failed: HTTP {}",
            response.status()
        ));
    }
    let reported_total = response.content_length().unwrap_or(package.compressed_size);
    crate::log_debug(&format!(
        "Google TTS: package {} HTTP {} expected bytes {} reported bytes {}",
        package.id,
        response.status(),
        package.compressed_size,
        reported_total
    ));
    let total = (reported_total > 0).then_some(reported_total);
    let transfer_result =
        write_download_body(&mut response, &temporary, total, cancel, &mut progress);
    if let Err(err) = transfer_result {
        if let Err(cleanup_err) = fs::remove_file(&temporary)
            && cleanup_err.kind() != std::io::ErrorKind::NotFound
        {
            crate::log_debug(&format!(
                "Google TTS incomplete download cleanup failed: {cleanup_err}"
            ));
        }
        return Err(err);
    }
    if package.compressed_size > 0 {
        let size = fs::metadata(&temporary)
            .map_err(|err| err.to_string())?
            .len();
        if size != package.compressed_size {
            if let Err(err) = fs::remove_file(&temporary) {
                crate::log_debug(&format!(
                    "Google TTS invalid download cleanup failed: {err}"
                ));
            }
            return Err(format!(
                "Google TTS voice size mismatch: expected {}, received {}",
                package.compressed_size, size
            ));
        }
    }
    if !package.sha256_checksum.trim().is_empty() {
        let actual = file_sha256(&temporary)?;
        if !actual.eq_ignore_ascii_case(&package.sha256_checksum) {
            if let Err(err) = fs::remove_file(&temporary) {
                crate::log_debug(&format!("Google TTS checksum cleanup failed: {err}"));
            }
            return Err("Google TTS voice checksum mismatch".to_string());
        }
    }
    fs::rename(&temporary, &target).map_err(|err| err.to_string())?;
    crate::log_debug(&format!(
        "Google TTS: package {} installed at {}",
        package.id,
        target.display()
    ));
    clear_verification_cache(&package.id);
    progress(100);
    Ok(())
}

pub fn download_package(
    package_id: &str,
    cancel: &Arc<AtomicBool>,
    mut progress: impl FnMut(i32),
) -> Result<(), String> {
    let mut plan = Vec::new();
    collect_download_plan(package_id, &mut HashSet::new(), &mut plan)?;
    let client = Client::builder()
        .connect_timeout(Duration::from_secs(30))
        .timeout(Duration::from_secs(20 * 60))
        .build()
        .map_err(|err| err.to_string())?;
    let count = plan.len().max(1) as i32;
    for (index, package) in plan.iter().enumerate() {
        let start = (index as i32 * 100) / count;
        let end = ((index as i32 + 1) * 100) / count;
        download_single_package(package, &client, cancel, |package_progress| {
            let scaled = start + ((end - start) * package_progress.clamp(0, 100)) / 100;
            progress(scaled.min(99));
        })?;
    }
    progress(100);
    restart_runtime();
    Ok(())
}

fn runtime_catalog_json() -> Result<Vec<u8>, String> {
    let values: Vec<Value> = catalog_cache()
        .iter()
        .filter(|package| is_package_installed(package))
        .map(|package| {
            json!({
                "id": package.id.clone(),
                "fileId": package.file_id.clone(),
                "url": format!("/{}.zvoice", package.id),
                "sha256Checksum": package.sha256_checksum.clone(),
                "compressedSize": package.compressed_size,
                "speakers": package.speakers.clone(),
                "remote": false,
                "dependentVoiceId": package.dependent_voice_id.clone()
            })
        })
        .collect();
    serde_json::to_vec(&values).map_err(|err| err.to_string())
}

struct EmbeddedHttpServer {
    port: u16,
    stop: Arc<AtomicBool>,
    thread: Option<thread::JoinHandle<()>>,
}

impl EmbeddedHttpServer {
    fn start() -> Result<Self, String> {
        let listener = TcpListener::bind("127.0.0.1:0").map_err(|err| err.to_string())?;
        listener
            .set_nonblocking(true)
            .map_err(|err| err.to_string())?;
        let port = listener.local_addr().map_err(|err| err.to_string())?.port();
        crate::log_debug(&format!(
            "Google TTS HTTP server: listening on 127.0.0.1:{port}; accepted sockets use blocking I/O"
        ));
        let stop = Arc::new(AtomicBool::new(false));
        let stop_copy = stop.clone();
        let thread = thread::spawn(move || {
            while !stop_copy.load(Ordering::Relaxed) {
                match listener.accept() {
                    Ok((mut stream, _)) => {
                        let _request_thread = thread::spawn(move || {
                            if let Err(err) = configure_http_client_stream(&stream)
                                .and_then(|()| handle_http_request(&mut stream))
                            {
                                crate::log_debug(&format!("Google TTS HTTP request failed: {err}"));
                            }
                        });
                    }
                    Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(10));
                    }
                    Err(err) => {
                        crate::log_debug(&format!("Google TTS HTTP server failed: {err}"));
                        break;
                    }
                }
            }
        });
        Ok(Self {
            port,
            stop,
            thread: Some(thread),
        })
    }
}

impl Drop for EmbeddedHttpServer {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(handle) = self.thread.take()
            && handle.join().is_err()
        {
            crate::log_debug("Google TTS HTTP server thread join failed");
        }
    }
}

const HTTP_REQUEST_LIMIT: usize = 64 * 1024;
const HTTP_IO_TIMEOUT: Duration = Duration::from_secs(10);

fn configure_http_client_stream(stream: &TcpStream) -> Result<(), String> {
    // On some Windows configurations a socket accepted from a non-blocking
    // listener inherits that mode. Chrome can connect before sending the HTTP
    // request, so an immediate read then fails with WSAEWOULDBLOCK (10035).
    // Client sockets are handled on their own threads and must be blocking.
    stream
        .set_nonblocking(false)
        .map_err(|err| format!("could not enable blocking mode: {err}"))?;
    stream
        .set_read_timeout(Some(HTTP_IO_TIMEOUT))
        .map_err(|err| format!("could not set read timeout: {err}"))?;
    stream
        .set_write_timeout(Some(HTTP_IO_TIMEOUT))
        .map_err(|err| format!("could not set write timeout: {err}"))?;
    Ok(())
}

fn read_http_request(stream: &mut TcpStream) -> Result<Vec<u8>, String> {
    let mut request = Vec::with_capacity(2048);
    let mut buffer = [0u8; 2048];
    loop {
        let read = match stream.read(&mut buffer) {
            Ok(read) => read,
            Err(err) if err.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(err) => return Err(format!("request read failed: {err}")),
        };
        if read == 0 {
            break;
        }
        request.extend_from_slice(&buffer[..read]);
        if request.windows(4).any(|window| window == b"\r\n\r\n") {
            break;
        }
        if request.len() >= HTTP_REQUEST_LIMIT {
            return Err("HTTP request headers are too large".to_string());
        }
    }
    if request.is_empty() {
        return Ok(request);
    }
    if !request.windows(4).any(|window| window == b"\r\n\r\n") {
        return Err("HTTP request ended before the headers were complete".to_string());
    }
    Ok(request)
}

fn handle_http_request(stream: &mut TcpStream) -> Result<(), String> {
    let request = read_http_request(stream)?;
    if request.is_empty() {
        return Ok(());
    }
    let request_text = String::from_utf8_lossy(&request);
    let path = request_text
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .unwrap_or("/")
        .split('?')
        .next()
        .unwrap_or("/");

    let (status, content_type, body) = match http_resource(path) {
        Ok(Some((content_type, body))) => ("200 OK", content_type, body),
        Ok(None) => (
            "404 Not Found",
            "text/plain; charset=utf-8",
            b"Not found".to_vec(),
        ),
        Err(err) => {
            crate::log_debug(&format!("Google TTS HTTP resource error: {err}"));
            (
                "500 Internal Server Error",
                "text/plain; charset=utf-8",
                b"Internal error".to_vec(),
            )
        }
    };
    let content_language = if content_type.starts_with("text/html") {
        "Content-Language: zxx\r\n"
    } else {
        ""
    };
    let header = format!(
        "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\n{content_language}Content-Length: {}\r\nConnection: close\r\nCross-Origin-Opener-Policy: same-origin\r\nCross-Origin-Embedder-Policy: require-corp\r\nCross-Origin-Resource-Policy: same-origin\r\n\r\n",
        body.len()
    );
    stream
        .write_all(header.as_bytes())
        .map_err(|err| err.to_string())?;
    stream.write_all(&body).map_err(|err| err.to_string())?;
    stream.flush().map_err(|err| err.to_string())?;
    Ok(())
}

fn http_resource(path: &str) -> Result<Option<(&'static str, Vec<u8>)>, String> {
    let mut normalized = path.replace("..", "");
    while normalized.contains("//") {
        normalized = normalized.replace("//", "/");
    }
    let embedded = match normalized.as_str() {
        "/" | "/index.html" => Some(("text/html; charset=utf-8", INDEX_HTML)),
        "/bridgeHarness.js" => Some(("application/javascript", BRIDGE_HARNESS_JS)),
        "/engine/background_compiled.js" => {
            Some(("application/javascript", BACKGROUND_COMPILED_JS))
        }
        "/engine/bindings_main.js" => Some(("application/javascript", BINDINGS_MAIN_JS)),
        "/engine/bindings_main.wasm" => Some(("application/wasm", BINDINGS_MAIN_WASM)),
        "/engine/manifest.json" => Some(("application/json", ENGINE_MANIFEST_JSON)),
        "/engine/offscreen.html" => Some(("text/html; charset=utf-8", OFFSCREEN_HTML)),
        "/engine/offscreen_compiled.js" => Some(("application/javascript", OFFSCREEN_COMPILED_JS)),
        "/engine/streaming_worklet_processor.js" | "/streaming_worklet_processor.js" => {
            Some(("application/javascript", STREAMING_WORKLET_JS))
        }
        "/engine/voices.json" | "/voices.json" => {
            return runtime_catalog_json().map(|body| Some(("application/json", body)));
        }
        "/engine/wasm_tts_manifest_v3.json" | "/wasm_tts_manifest_v3.json" => {
            Some(("application/json", WASM_MANIFEST_JSON))
        }
        _ => None,
    };
    if let Some((content_type, bytes)) = embedded {
        return Ok(Some((content_type, bytes.to_vec())));
    }
    if normalized.ends_with(".zvoice") {
        let file_name: String = normalized
            .rsplit('/')
            .next()
            .unwrap_or_default()
            .chars()
            .filter(|character| *character != '/' && *character != '\\')
            .collect();
        if file_name.is_empty() {
            return Ok(None);
        }
        let path = voice_dir().join(file_name);
        if path.is_file() {
            return fs::read(path)
                .map(|body| Some(("application/octet-stream", body)))
                .map_err(|err| err.to_string());
        }
    }
    Ok(None)
}

struct GoogleTtsRuntime {
    _server: EmbeddedHttpServer,
    browser: Child,
    browser_name: &'static str,
    profile_dir: PathBuf,
    socket: WebSocket<MaybeTlsStream<TcpStream>>,
    next_message_id: u64,
}

impl GoogleTtsRuntime {
    fn start(cancel: &Arc<AtomicBool>) -> Result<Self, String> {
        let startup_lock_started = Instant::now();
        let _startup_guard = lock_runtime_start(cancel)?;
        crate::log_debug(&format!(
            "Google TTS runtime startup lock acquired after {} ms",
            startup_lock_started.elapsed().as_millis()
        ));
        if cancel.load(Ordering::Relaxed) {
            return Err("cancelled".to_string());
        }
        if !has_installed_voices() {
            return Err("No Google TTS voice packages are installed.".to_string());
        }
        fs::create_dir_all(runtime_dir()).map_err(|err| err.to_string())?;
        let browsers = find_browsers();
        if browsers.is_empty() {
            return Err(
                "Google Chrome and Microsoft Edge were not found. Install one of them, or set CHROME_PATH or EDGE_PATH."
                    .to_string(),
            );
        }
        let browser_count = browsers.len();
        let server = EmbeddedHttpServer::start()?;
        let page_url = format!("http://127.0.0.1:{}/", server.port);
        let mut started_browser: Option<(Child, &'static str, PathBuf, u16)> = None;
        let mut last_browser_start_error: Option<String> = None;

        for (index, browser) in browsers.into_iter().enumerate() {
            let profiles_dir = browser_profiles_dir(browser.runtime);
            fs::create_dir_all(&profiles_dir).map_err(|err| err.to_string())?;
            cleanup_old_profiles(&profiles_dir, browser.runtime.display_name());
            crate::log_debug(&format!(
                "Google TTS browser runtime: {} ({})",
                browser.runtime.display_name(),
                browser.path.display()
            ));
            let timestamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|value| value.as_nanos())
                .unwrap_or(0);
            let profile_sequence = PROFILE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
            let profile_dir = profiles_dir.join(format!(
                "session-{}-{timestamp}-{profile_sequence}",
                std::process::id()
            ));
            fs::create_dir_all(&profile_dir).map_err(|err| err.to_string())?;
            let devtools_file = profile_dir.join("DevToolsActivePort");
            let browser_args = vec![
                "--headless=new".to_string(),
                "--remote-debugging-port=0".to_string(),
                "--remote-allow-origins=*".to_string(),
                format!("--user-data-dir={}", profile_dir.display()),
                "--no-first-run".to_string(),
                "--no-default-browser-check".to_string(),
                "--disable-translate".to_string(),
                "--disable-features=Translate,TranslateUI".to_string(),
                "--disable-renderer-accessibility".to_string(),
                "--disable-background-networking".to_string(),
                "--disable-breakpad".to_string(),
                "--disable-crash-reporter".to_string(),
                "--noerrdialogs".to_string(),
                "--autoplay-policy=no-user-gesture-required".to_string(),
                page_url.clone(),
            ];
            let browser_name = browser.runtime.display_name();
            let mut browser_process = match Command::new(&browser.path)
                .args(&browser_args)
                .stdin(Stdio::null())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .creation_flags(CREATE_NO_WINDOW)
                .spawn()
            {
                Ok(process) => process,
                Err(err) => {
                    let message = format!("Failed to start {browser_name}: {err}");
                    if index + 1 < browser_count {
                        crate::log_debug(&format!(
                            "Google TTS {browser_name} could not be started; trying the alternate browser runtime: {message}"
                        ));
                        cleanup_failed_browser_profile(&profile_dir, browser_name);
                        last_browser_start_error = Some(message);
                        continue;
                    }
                    return Err(message);
                }
            };
            let startup_stdout = browser_process
                .stdout
                .take()
                .map(spawn_browser_output_reader);
            let startup_stderr = browser_process
                .stderr
                .take()
                .map(spawn_browser_output_reader);
            match wait_for_devtools_port(&mut browser_process, browser_name, &devtools_file, cancel)
            {
                Ok(debug_port) => {
                    started_browser =
                        Some((browser_process, browser_name, profile_dir, debug_port));
                    break;
                }
                Err(err)
                    if is_browser_early_exit_error(&err)
                        && index + 1 < browser_count
                        && !cancel.load(Ordering::Relaxed) =>
                {
                    if let Err(wait_err) = browser_process.wait() {
                        crate::log_debug(&format!(
                            "Google TTS {browser_name} failed while waiting for the exited browser process: {wait_err}"
                        ));
                    }
                    log_browser_startup_failure_output(
                        browser_name,
                        startup_stdout,
                        startup_stderr,
                    );
                    crate::log_debug(&format!(
                        "Google TTS {browser_name} exited before startup; trying the alternate browser runtime: {err}"
                    ));
                    cleanup_failed_browser_profile(&profile_dir, browser_name);
                    last_browser_start_error = Some(err);
                }
                Err(err) if is_browser_early_exit_error(&err) => {
                    if let Err(wait_err) = browser_process.wait() {
                        crate::log_debug(&format!(
                            "Google TTS {browser_name} failed while waiting for the exited browser process: {wait_err}"
                        ));
                    }
                    log_browser_startup_failure_output(
                        browser_name,
                        startup_stdout,
                        startup_stderr,
                    );
                    return Err(err);
                }
                Err(err) => return Err(err),
            }
        }

        let (browser_process, browser_name, profile_dir, debug_port) =
            started_browser.ok_or_else(|| {
                last_browser_start_error.unwrap_or_else(|| {
                    "Google TTS could not start an available browser runtime.".to_string()
                })
            })?;
        let websocket_url = wait_for_page_websocket(debug_port, &page_url, cancel)?;
        let (mut socket, _) = connect(websocket_url.as_str())
            .map_err(|err| format!("Google TTS DevTools connection failed: {err}"))?;
        if let MaybeTlsStream::Plain(stream) = socket.get_mut()
            && let Err(err) = stream.set_read_timeout(Some(Duration::from_millis(25)))
        {
            crate::log_debug(&format!("Google TTS read timeout setup failed: {err}"));
        }
        let mut runtime = Self {
            _server: server,
            browser: browser_process,
            browser_name,
            profile_dir,
            socket,
            next_message_id: 1,
        };
        runtime.cdp_request(
            "Runtime.enable",
            json!({}),
            Duration::from_secs(15),
            cancel,
            None,
        )?;
        runtime.cdp_request(
            "Page.enable",
            json!({}),
            Duration::from_secs(15),
            cancel,
            None,
        )?;
        runtime.cdp_request(
            "Runtime.addBinding",
            json!({"name": BINDING_NAME}),
            Duration::from_secs(15),
            cancel,
            None,
        )?;
        runtime.wait_until_ready(cancel)?;
        Ok(runtime)
    }

    fn wait_until_ready(&mut self, cancel: &Arc<AtomicBool>) -> Result<(), String> {
        let bridge_expression = r#"
            typeof window.googleTtsForSonarpadSpeak === "function"
            && typeof window.googleTtsForSonarpadPreload === "function"
            && typeof window.googleTtsForSonarpadBridge === "function"
            && typeof window.googleTtsForSonarpadWaitForEngine === "function"
            && typeof window.googleTtsForSonarpadEngineStatus === "function"
        "#;
        let mut bridge_ready = false;
        for attempt in 0..400 {
            let response = match self.cdp_request(
                "Runtime.evaluate",
                json!({"expression": bridge_expression, "returnByValue": true}),
                Duration::from_secs(5),
                cancel,
                None,
            ) {
                Ok(response) => response,
                Err(err) if is_transient_execution_context_error(&err) => {
                    if attempt == 0 || attempt % 20 == 19 {
                        crate::log_debug(&format!(
                            "Google TTS runtime: JavaScript execution context not ready during startup (attempt {}): {}",
                            attempt + 1,
                            err
                        ));
                    }
                    thread::sleep(Duration::from_millis(50));
                    continue;
                }
                Err(err) => return Err(err),
            };
            if response
                .pointer("/result/result/value")
                .and_then(Value::as_bool)
                .unwrap_or(false)
            {
                bridge_ready = true;
                break;
            }
            thread::sleep(Duration::from_millis(50));
        }
        if !bridge_ready {
            return Err("Google TTS bridge did not finish loading.".to_string());
        }

        crate::log_debug("Google TTS runtime: bridge loaded; waiting for the WASM engine");
        let response = self.cdp_request(
            "Runtime.evaluate",
            json!({
                "expression": "window.googleTtsForSonarpadWaitForEngine(30000)",
                "awaitPromise": true,
                "returnByValue": true
            }),
            Duration::from_secs(35),
            cancel,
            None,
        )?;
        let status = response
            .pointer("/result/result/value")
            .cloned()
            .unwrap_or(Value::Null);
        if status.get("ready").and_then(Value::as_bool) != Some(true) {
            return Err(format!(
                "Google TTS WASM engine did not finish loading. status={status}"
            ));
        }
        crate::log_debug(&format!(
            "Google TTS runtime: WASM engine ready; status={status}"
        ));
        Ok(())
    }

    fn cdp_request(
        &mut self,
        method: &str,
        params: Value,
        timeout: Duration,
        cancel: &Arc<AtomicBool>,
        mut event_handler: CdpEventHandler<'_>,
    ) -> Result<Value, String> {
        let id = self.next_message_id;
        self.next_message_id = self.next_message_id.saturating_add(1);
        let request = json!({"id": id, "method": method, "params": params});
        self.socket
            .send(Message::Text(request.to_string().into()))
            .map_err(|err| err.to_string())?;
        let deadline = Instant::now() + timeout;
        while Instant::now() < deadline {
            if cancel.load(Ordering::Relaxed) {
                self.send_stop();
                return Err("cancelled".to_string());
            }
            match self.socket.read() {
                Ok(Message::Text(text)) => {
                    let message: Value =
                        serde_json::from_str(text.as_str()).map_err(|err| err.to_string())?;
                    if let Some(handler) = event_handler.as_mut() {
                        handler(&message)?;
                    }
                    if message.get("id").and_then(Value::as_u64) != Some(id) {
                        continue;
                    }
                    if let Some(error) = message.get("error") {
                        return Err(format!("Google TTS CDP error for {method}: {error}"));
                    }
                    if let Some(details) = message.pointer("/result/exceptionDetails") {
                        return Err(format!("Google TTS JavaScript error: {details}"));
                    }
                    return Ok(message);
                }
                Ok(Message::Close(_)) => {
                    return Err("Google TTS DevTools connection closed.".to_string());
                }
                Ok(_) => {}
                Err(tungstenite::Error::Io(err))
                    if matches!(
                        err.kind(),
                        std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                    ) => {}
                Err(err) => return Err(format!("Google TTS DevTools read failed: {err}")),
            }
        }
        Err(format!("Google TTS timed out waiting for {method}."))
    }

    fn send_stop(&mut self) {
        let id = self.next_message_id;
        self.next_message_id = self.next_message_id.saturating_add(1);
        let request = json!({
            "id": id,
            "method": "Runtime.evaluate",
            "params": {
                "expression": STOP_EXPRESSION,
                "awaitPromise": false,
                "returnByValue": true
            }
        });
        if let Err(err) = self.socket.send(Message::Text(request.to_string().into())) {
            crate::log_debug(&format!("Google TTS stop command failed: {err}"));
        }
    }

    fn synthesize(
        &mut self,
        text: &str,
        speaker: &GoogleSpeaker,
        rate: i32,
        pitch: i32,
        volume: i32,
        cancel: &Arc<AtomicBool>,
    ) -> Result<Vec<u8>, String> {
        if text.trim().is_empty() {
            return Ok(wav_from_pcm(&[]));
        }
        let synthesis_started = Instant::now();
        let mapped_rate = google_rate(rate);
        let synthesis_timeout = google_synthesis_timeout(mapped_rate);
        let javascript_timeout_ms = synthesis_timeout
            .saturating_sub(Duration::from_secs(5))
            .as_millis() as u64;
        let pitch_percent = google_pitch_percent_from_internal(pitch);
        let mapped_pitch = google_pitch(pitch);
        let mapped_volume = (volume as f64 / 100.0).clamp(0.0, 1.0);
        let output_gain = (volume as f64 / 50.0).clamp(0.0, 2.0);
        crate::log_debug(&format!(
            "Google TTS synth start: voice_id={} voice_name={:?} text_chars={} rate={} mapped_rate={:.4} pitch_internal={} pitch_percent={} mapped_pitch={:.4} volume={} mapped_volume={:.3}",
            speaker.id,
            speaker.name,
            text.chars().count(),
            rate,
            mapped_rate,
            pitch,
            pitch_percent,
            mapped_pitch,
            volume,
            mapped_volume
        ));
        let session_id = format!("{}-{}", std::process::id(), self.next_message_id);
        let payload = json!({
            "sessionId": session_id,
            "text": text,
            "voiceName": speaker.name.clone(),
            "lang": speaker.language.clone(),
            "rate": mapped_rate,
            "pitch": mapped_pitch,
            "volume": mapped_volume,
            "outputGain": output_gain
        });
        let expression = format!(
            "window.googleTtsForSonarpadSpeak({})",
            serde_json::to_string(&payload).map_err(|err| err.to_string())?
        );
        let mut pcm = Vec::new();
        let mut completed = false;
        let session_for_events = session_id.clone();
        let mut handler = |message: &Value| -> Result<(), String> {
            if message.get("method").and_then(Value::as_str) != Some("Runtime.bindingCalled") {
                return Ok(());
            }
            let params = message.get("params").unwrap_or(&Value::Null);
            if params.get("name").and_then(Value::as_str) != Some(BINDING_NAME) {
                return Ok(());
            }
            let Some(raw_payload) = params.get("payload").and_then(Value::as_str) else {
                return Ok(());
            };
            let event: Value = serde_json::from_str(raw_payload).map_err(|err| err.to_string())?;
            if event.get("sessionId").and_then(Value::as_str) != Some(session_for_events.as_str()) {
                return Ok(());
            }
            match event.get("type").and_then(Value::as_str) {
                Some("started") => {
                    crate::log_debug(&format!(
                        "Google TTS bridge started: session={} rate={:?} pitch={:?} volume={:?}",
                        session_for_events,
                        event.get("rate").and_then(Value::as_f64),
                        event.get("pitch").and_then(Value::as_f64),
                        event.get("volume").and_then(Value::as_f64)
                    ));
                }
                Some("audio") => {
                    let encoded = event
                        .get("data")
                        .and_then(Value::as_str)
                        .unwrap_or_default();
                    let decoded = base64::engine::general_purpose::STANDARD
                        .decode(encoded)
                        .map_err(|err| err.to_string())?;
                    pcm.extend_from_slice(&decoded);
                }
                Some("done") => completed = true,
                Some("error") => {
                    return Err(event
                        .get("message")
                        .and_then(Value::as_str)
                        .unwrap_or("Google TTS synthesis failed")
                        .to_string());
                }
                _ => {}
            }
            Ok(())
        };
        let response = self.cdp_request(
            "Runtime.evaluate",
            json!({
                "expression": expression,
                "awaitPromise": true,
                "returnByValue": true,
                "userGesture": true,
                "timeout": javascript_timeout_ms
            }),
            synthesis_timeout,
            cancel,
            Some(&mut handler),
        )?;
        if response
            .pointer("/result/result/subtype")
            .and_then(Value::as_str)
            == Some("error")
        {
            return Err("Google TTS JavaScript evaluation failed.".to_string());
        }
        if !completed && pcm.is_empty() {
            return Err("Google TTS produced no audio.".to_string());
        }
        crate::log_debug(&format!(
            "Google TTS synth complete: session={} elapsed_ms={} pcm_bytes={} completed_event={}",
            session_id,
            synthesis_started.elapsed().as_millis(),
            pcm.len(),
            completed
        ));
        Ok(wav_from_pcm(&pcm))
    }
}

impl Drop for GoogleTtsRuntime {
    fn drop(&mut self) {
        self.send_stop();
        if let Err(err) = self.browser.kill() {
            crate::log_debug(&format!(
                "Google TTS {} shutdown failed: {err}",
                self.browser_name
            ));
        }
        if let Err(err) = self.browser.wait() {
            crate::log_debug(&format!(
                "Google TTS {} wait failed: {err}",
                self.browser_name
            ));
        }
        if let Err(err) = fs::remove_dir_all(&self.profile_dir)
            && err.kind() != std::io::ErrorKind::NotFound
        {
            crate::log_debug(&format!(
                "Google TTS {} profile cleanup failed: {err}",
                self.browser_name
            ));
        }
    }
}

fn is_transient_execution_context_error(error: &str) -> bool {
    error.contains("Cannot find default execution context")
        || error.contains("Execution context was destroyed")
        || error.contains("Cannot find context with specified id")
        || error.contains("Inspected target navigated or closed")
}

fn is_transient_google_engine_startup_error(error: &str) -> bool {
    is_transient_execution_context_error(error)
        || error.contains("Chrome WASM TTS engine was not loaded")
        || error.contains("Google TTS WASM engine did not finish loading")
        || error.contains("Google TTS engine did not finish loading")
}

fn start_google_runtime_with_retry(
    cancel: &Arc<AtomicBool>,
    session_label: &str,
) -> Result<GoogleTtsRuntime, String> {
    match GoogleTtsRuntime::start(cancel) {
        Ok(runtime) => Ok(runtime),
        Err(err)
            if is_transient_google_engine_startup_error(&err)
                && !cancel.load(Ordering::Relaxed) =>
        {
            crate::log_debug(&format!(
                "Google TTS runtime [{}]: startup was incomplete; recreating the browser runtime and retrying once: {}",
                session_label, err
            ));
            thread::sleep(Duration::from_millis(200));
            GoogleTtsRuntime::start(cancel)
        }
        Err(err) => Err(err),
    }
}

fn runtime_slot() -> &'static Mutex<Option<GoogleTtsRuntime>> {
    static RUNTIME: OnceLock<Mutex<Option<GoogleTtsRuntime>>> = OnceLock::new();
    RUNTIME.get_or_init(|| Mutex::new(None))
}

pub fn restart_runtime() {
    match runtime_slot().lock() {
        Ok(mut runtime) => *runtime = None,
        Err(_) => crate::log_debug("Google TTS runtime lock poisoned during restart"),
    }
}

pub fn shutdown() {
    restart_runtime();
}

fn installed_speaker_for_voice(stored_voice: &str) -> Result<GoogleSpeaker, String> {
    let voice_id = voice_id_from_stored(stored_voice);
    all_installed_speakers()
        .into_iter()
        .find(|speaker| speaker.id == voice_id)
        .ok_or_else(|| {
            "The selected Google TTS voice is not installed. Open the Google voice manager."
                .to_string()
        })
}

struct RuntimeSynthesisRequest<'a> {
    text: &'a str,
    speaker: &'a GoogleSpeaker,
    rate: i32,
    pitch: i32,
    volume: i32,
    cancel: &'a Arc<AtomicBool>,
    session_label: &'a str,
}

fn synthesize_with_runtime(
    runtime: &mut Option<GoogleTtsRuntime>,
    request: RuntimeSynthesisRequest<'_>,
) -> Result<Vec<u8>, String> {
    let RuntimeSynthesisRequest {
        text,
        speaker,
        rate,
        pitch,
        volume,
        cancel,
        session_label,
    } = request;
    if runtime.is_none() {
        let runtime_started = Instant::now();
        crate::log_debug(&format!(
            "Google TTS runtime [{}]: cold start requested",
            session_label
        ));
        *runtime = Some(start_google_runtime_with_retry(cancel, session_label)?);
        crate::log_debug(&format!(
            "Google TTS runtime [{}]: cold start completed in {} ms",
            session_label,
            runtime_started.elapsed().as_millis()
        ));
    } else {
        crate::log_debug(&format!(
            "Google TTS runtime [{}]: reusing active browser session",
            session_label
        ));
    }

    let result = runtime
        .as_mut()
        .ok_or_else(|| "Google TTS runtime unavailable".to_string())?
        .synthesize(text, speaker, rate, pitch, volume, cancel);
    if let Err(err) = &result {
        let retry_transient_runtime =
            is_transient_google_engine_startup_error(err) && !cancel.load(Ordering::Relaxed);
        *runtime = None;
        if retry_transient_runtime {
            crate::log_debug(&format!(
                "Google TTS runtime [{}]: transient browser or WASM engine failure; restarting the browser runtime and retrying once: {}",
                session_label, err
            ));
            thread::sleep(Duration::from_millis(100));
            let runtime_started = Instant::now();
            *runtime = Some(start_google_runtime_with_retry(cancel, session_label)?);
            crate::log_debug(&format!(
                "Google TTS runtime [{}]: recovery cold start completed in {} ms",
                session_label,
                runtime_started.elapsed().as_millis()
            ));
            let retry_result = runtime
                .as_mut()
                .ok_or_else(|| "Google TTS runtime unavailable after recovery".to_string())?
                .synthesize(text, speaker, rate, pitch, volume, cancel);
            if retry_result.is_err() {
                *runtime = None;
            }
            return retry_result;
        }
    }
    result
}

/// An independent browser/WASM session used by Google audiobook workers.
/// Each worker owns one instance, so long audiobook blocks can be synthesized
/// concurrently without contending on the shared interactive TTS runtime.
pub(crate) struct GoogleTtsWorkerSession {
    runtime: Option<GoogleTtsRuntime>,
    worker_id: usize,
}

impl GoogleTtsWorkerSession {
    pub(crate) fn new(worker_id: usize) -> Self {
        Self {
            runtime: None,
            worker_id,
        }
    }

    pub(crate) fn synthesize_wav_bytes(
        &mut self,
        text: &str,
        stored_voice: &str,
        rate: i32,
        pitch: i32,
        volume: i32,
        cancel: &Arc<AtomicBool>,
    ) -> Result<Vec<u8>, String> {
        let speaker = installed_speaker_for_voice(stored_voice)?;
        let label = format!("audiobook-worker-{}", self.worker_id);
        synthesize_with_runtime(
            &mut self.runtime,
            RuntimeSynthesisRequest {
                text,
                speaker: &speaker,
                rate,
                pitch,
                volume,
                cancel,
                session_label: &label,
            },
        )
    }
}

pub fn synthesize_wav_bytes(
    text: &str,
    stored_voice: &str,
    rate: i32,
    pitch: i32,
    volume: i32,
    cancel: &Arc<AtomicBool>,
) -> Result<Vec<u8>, String> {
    let speaker = installed_speaker_for_voice(stored_voice)?;
    let slot = runtime_slot();
    let mut guard = slot
        .lock()
        .map_err(|_| "Google TTS runtime lock poisoned".to_string())?;
    synthesize_with_runtime(
        &mut guard,
        RuntimeSynthesisRequest {
            text,
            speaker: &speaker,
            rate,
            pitch,
            volume,
            cancel,
            session_label: "shared",
        },
    )
}

fn google_rate(rate: i32) -> f64 {
    let normalized = ((rate.clamp(-100, 100) + 100) as f64) / 200.0;
    (0.35 + (2.0 - 0.35) * normalized).clamp(0.1, 10.0)
}

fn google_synthesis_timeout(mapped_rate: f64) -> Duration {
    let seconds = (30.0 / mapped_rate.max(0.1)).ceil().clamp(35.0, 120.0) as u64;
    Duration::from_secs(seconds)
}

fn google_pitch(pitch: i32) -> f64 {
    // Exact conversion used by googleTtsForNvda: 0..100, 50 = normal.
    let percent = google_pitch_percent_from_internal(pitch) as f64;
    let pitch_semitones = -12.0 + 24.0 * percent / 100.0;
    let chrome_pitch = (1.0 + pitch_semitones / 20.0).clamp(0.1, 3.0);
    (chrome_pitch * 1000.0).round() / 1000.0
}

fn wav_from_pcm(pcm: &[u8]) -> Vec<u8> {
    let data_len = u32::try_from(pcm.len()).unwrap_or(u32::MAX);
    let riff_len = 36u32.saturating_add(data_len);
    let byte_rate = SAMPLE_RATE.saturating_mul(2);
    let mut wav = Vec::with_capacity(44usize.saturating_add(pcm.len()));
    wav.extend_from_slice(b"RIFF");
    wav.extend_from_slice(&riff_len.to_le_bytes());
    wav.extend_from_slice(b"WAVEfmt ");
    wav.extend_from_slice(&16u32.to_le_bytes());
    wav.extend_from_slice(&1u16.to_le_bytes());
    wav.extend_from_slice(&1u16.to_le_bytes());
    wav.extend_from_slice(&SAMPLE_RATE.to_le_bytes());
    wav.extend_from_slice(&byte_rate.to_le_bytes());
    wav.extend_from_slice(&2u16.to_le_bytes());
    wav.extend_from_slice(&16u16.to_le_bytes());
    wav.extend_from_slice(b"data");
    wav.extend_from_slice(&data_len.to_le_bytes());
    wav.extend_from_slice(pcm);
    wav
}

fn browser_candidates(runtime: BrowserRuntime) -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    if let Some(path) = std::env::var_os(runtime.environment_variable()) {
        candidates.push(PathBuf::from(path));
    }
    for key in ["PROGRAMFILES", "PROGRAMFILES(X86)", "LOCALAPPDATA"] {
        let Some(root) = std::env::var_os(key) else {
            continue;
        };
        let root = PathBuf::from(root);
        match runtime {
            BrowserRuntime::Chrome => candidates.push(
                root.join("Google")
                    .join("Chrome")
                    .join("Application")
                    .join(runtime.executable_name()),
            ),
            BrowserRuntime::Edge => candidates.push(
                root.join("Microsoft")
                    .join("Edge")
                    .join("Application")
                    .join(runtime.executable_name()),
            ),
        }
    }
    if let Some(path_value) = std::env::var_os("PATH") {
        for directory in std::env::split_paths(&path_value) {
            candidates.push(directory.join(runtime.executable_name()));
            candidates.push(directory.join(match runtime {
                BrowserRuntime::Chrome => "chrome",
                BrowserRuntime::Edge => "msedge",
            }));
        }
    }
    candidates
}

fn find_browsers() -> Vec<BrowserExecutable> {
    // Preserve Sonarpad's existing behavior: Chrome remains the first choice.
    // Edge is kept as a dormant fallback and is only used if Chrome cannot
    // start the isolated Google TTS runtime on this machine.
    let mut browsers = Vec::new();
    for runtime in [BrowserRuntime::Chrome, BrowserRuntime::Edge] {
        if let Some(path) = browser_candidates(runtime)
            .into_iter()
            .find(|path| path.is_file())
        {
            browsers.push(BrowserExecutable { runtime, path });
        }
    }
    browsers
}

fn is_browser_early_exit_error(error: &str) -> bool {
    error.contains("exited before Google TTS started:")
}

const BROWSER_STARTUP_OUTPUT_LIMIT: usize = 16 * 1024;

fn spawn_browser_output_reader<R>(mut reader: R) -> thread::JoinHandle<String>
where
    R: Read + Send + 'static,
{
    thread::spawn(move || {
        let mut captured = Vec::with_capacity(BROWSER_STARTUP_OUTPUT_LIMIT.min(4096));
        let mut buffer = [0_u8; 4096];
        loop {
            match reader.read(&mut buffer) {
                Ok(0) => break,
                Ok(read) => {
                    if captured.len() < BROWSER_STARTUP_OUTPUT_LIMIT {
                        let remaining = BROWSER_STARTUP_OUTPUT_LIMIT - captured.len();
                        captured.extend_from_slice(&buffer[..read.min(remaining)]);
                    }
                }
                Err(_) => break,
            }
        }
        String::from_utf8_lossy(&captured).trim().to_string()
    })
}

fn log_browser_startup_failure_output(
    browser_name: &str,
    stdout: Option<thread::JoinHandle<String>>,
    stderr: Option<thread::JoinHandle<String>>,
) {
    let stdout = stdout
        .and_then(|handle| handle.join().ok())
        .unwrap_or_default();
    let stderr = stderr
        .and_then(|handle| handle.join().ok())
        .unwrap_or_default();

    if !stdout.is_empty() {
        crate::log_debug(&format!(
            "Google TTS {browser_name} startup stdout (reported only after early exit): {stdout}"
        ));
    }
    if !stderr.is_empty() {
        crate::log_debug(&format!(
            "Google TTS {browser_name} startup stderr (reported only after early exit): {stderr}"
        ));
    }
    if stdout.is_empty() && stderr.is_empty() {
        crate::log_debug(&format!(
            "Google TTS {browser_name} early exit produced no stdout/stderr output."
        ));
    }
}

fn cleanup_failed_browser_profile(profile_dir: &Path, browser_name: &str) {
    if let Err(err) = fs::remove_dir_all(profile_dir)
        && err.kind() != std::io::ErrorKind::NotFound
    {
        crate::log_debug(&format!(
            "Google TTS {browser_name} failed-start profile cleanup failed: {err}"
        ));
    }
}

fn wait_for_devtools_port(
    browser: &mut Child,
    browser_name: &str,
    devtools_file: &Path,
    cancel: &Arc<AtomicBool>,
) -> Result<u16, String> {
    for _ in 0..400 {
        if cancel.load(Ordering::Relaxed) {
            if let Err(err) = browser.kill() {
                crate::log_debug(&format!(
                    "Google TTS cancelled {browser_name} kill failed: {err}"
                ));
            }
            return Err("cancelled".to_string());
        }
        match browser.try_wait() {
            Ok(Some(status)) => {
                return Err(format!(
                    "{browser_name} exited before Google TTS started: {status}"
                ));
            }
            Ok(None) => {}
            Err(err) => return Err(err.to_string()),
        }
        if devtools_file.is_file() {
            let content = fs::read_to_string(devtools_file).map_err(|err| err.to_string())?;
            if let Some(first) = content.lines().next()
                && let Ok(port) = first.trim().parse::<u16>()
            {
                return Ok(port);
            }
        }
        thread::sleep(Duration::from_millis(50));
    }
    Err(format!("Timed out waiting for {browser_name} DevTools."))
}

fn wait_for_page_websocket(
    debug_port: u16,
    page_url: &str,
    cancel: &Arc<AtomicBool>,
) -> Result<String, String> {
    let client = Client::builder()
        .timeout(Duration::from_secs(2))
        .build()
        .map_err(|err| err.to_string())?;
    let endpoint = format!("http://127.0.0.1:{debug_port}/json/list");
    for _ in 0..200 {
        if cancel.load(Ordering::Relaxed) {
            return Err("cancelled".to_string());
        }
        if let Ok(response) = client.get(&endpoint).send()
            && let Ok(targets) = response.json::<Vec<Value>>()
        {
            for target in targets {
                if target.get("type").and_then(Value::as_str) == Some("page")
                    && target.get("url").and_then(Value::as_str) == Some(page_url)
                    && let Some(url) = target.get("webSocketDebuggerUrl").and_then(Value::as_str)
                {
                    return Ok(url.to_string());
                }
            }
        }
        thread::sleep(Duration::from_millis(50));
    }
    Err("Could not find the Google TTS browser page.".to_string())
}

fn cleanup_old_profiles(root: &Path, browser_name: &str) {
    let Ok(entries) = fs::read_dir(root) else {
        return;
    };
    let cutoff = SystemTime::now()
        .checked_sub(Duration::from_secs(2 * 24 * 60 * 60))
        .unwrap_or(SystemTime::UNIX_EPOCH);
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let should_remove = entry
            .metadata()
            .ok()
            .and_then(|metadata| metadata.modified().ok())
            .is_some_and(|modified| modified < cutoff);
        if should_remove && let Err(err) = fs::remove_dir_all(&path) {
            crate::log_debug(&format!(
                "Google TTS old {browser_name} profile cleanup failed for {}: {err}",
                path.display()
            ));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn synthesis_timeout_is_short_at_normal_rate_and_scales_for_slow_speech() {
        assert_eq!(google_synthesis_timeout(google_rate(0)).as_secs(), 35);
        assert_eq!(google_synthesis_timeout(google_rate(100)).as_secs(), 35);
        assert_eq!(google_synthesis_timeout(google_rate(-100)).as_secs(), 86);
    }

    #[test]
    fn browser_exit_before_devtools_is_detected_for_safe_fallback() {
        assert!(is_browser_early_exit_error(
            "Google Chrome exited before Google TTS started: exit code: 0"
        ));
        assert!(is_browser_early_exit_error(
            "Microsoft Edge exited before Google TTS started: exit code: 1"
        ));
        assert!(!is_browser_early_exit_error(
            "Timed out waiting for Google Chrome DevTools."
        ));
    }

    #[test]
    fn wasm_engine_startup_errors_are_treated_as_transient() {
        assert!(is_transient_google_engine_startup_error(
            "Chrome WASM TTS engine was not loaded."
        ));
        assert!(is_transient_google_engine_startup_error(
            "Google TTS WASM engine did not finish loading."
        ));
        assert!(!is_transient_google_engine_startup_error(
            "The selected Google TTS voice is not installed."
        ));
    }

    #[test]
    fn embedded_bridge_exposes_wasm_readiness_helpers() {
        let script = String::from_utf8_lossy(BRIDGE_HARNESS_JS);
        assert!(script.contains("googleTtsForSonarpadWaitForEngine"));
        assert!(script.contains("googleTtsForSonarpadEngineStatus"));
        assert!(script.contains("waitForTtsEngine"));
    }

    #[test]
    fn embedded_google_tts_pages_disable_translation() {
        for page in [INDEX_HTML, OFFSCREEN_HTML] {
            let html = String::from_utf8_lossy(page);
            assert!(html.contains("lang=\"zxx\""));
            assert!(html.contains("translate=\"no\""));
            assert!(html.contains("name=\"google\" content=\"notranslate\""));
        }
    }

    #[test]
    fn embedded_http_handler_waits_for_a_delayed_partial_request() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind test listener");
        listener
            .set_nonblocking(true)
            .expect("set test listener nonblocking");
        let address = listener.local_addr().expect("test listener address");
        let client = thread::spawn(move || {
            let mut stream = TcpStream::connect(address).expect("connect test client");
            stream
                .write_all(b"GET / HTTP/1.1\r\nHost: 127.0.0.1")
                .expect("write first request part");
            thread::sleep(Duration::from_millis(40));
            stream
                .write_all(b"\r\nConnection: close\r\n\r\n")
                .expect("write second request part");
            let mut response = Vec::new();
            stream.read_to_end(&mut response).expect("read response");
            response
        });

        let mut accepted = loop {
            match listener.accept() {
                Ok((stream, _)) => break stream,
                Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => {
                    thread::sleep(Duration::from_millis(5));
                }
                Err(err) => panic!("accept test client: {err}"),
            }
        };
        // Reproduce the Windows failure deterministically even on platforms
        // where accepted sockets do not inherit the listener's mode.
        accepted
            .set_nonblocking(true)
            .expect("set accepted test stream nonblocking");
        configure_http_client_stream(&accepted).expect("configure accepted stream");
        handle_http_request(&mut accepted).expect("serve delayed request");
        accepted
            .shutdown(std::net::Shutdown::Both)
            .expect("shutdown accepted test stream");

        let response = client.join().expect("join test client");
        let response = String::from_utf8_lossy(&response);
        assert!(response.starts_with("HTTP/1.1 200 OK"));
        assert!(response.contains("\r\nContent-Language: zxx\r\n"));
    }
}
