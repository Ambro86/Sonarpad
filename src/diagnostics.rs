// src/diagnostics.rs
//
// Export diagnostics for troubleshooting.
// Creates a zip file with logs, settings, and version info.
// All paths are sanitized to avoid PII leakage.

use std::fs::File;
use std::io::{Read, Write};
use std::path::Path;
use zip::ZipWriter;
use zip::write::FileOptions;

use crate::sentry_integration;
use crate::settings;

/// Sanitizza i path nel contenuto testuale per evitare PII.
/// Sostituisce path Windows (C:\Users\...) con <user_path>.
fn sanitize_content(content: &str) -> String {
    // These patterns need no look-around/backreferences. Use the linear-time
    // engine: fancy-regex can exhaust its backtracking budget on large logs.
    static PATTERNS: std::sync::OnceLock<Option<Vec<(regex::Regex, &'static str)>>> =
        std::sync::OnceLock::new();
    let patterns = PATTERNS.get_or_init(|| {
        [
            (r"(?i)C:\\Users\\[^\\\r\n]+", "<user_path>"),
            (r"AIza[\w-]{20,}", "[REDACTED]"),
            (r"\b(?:sp_|sst_)[\w-]{12,}", "[REDACTED]"),
            (r"(?i)Bearer\s+[\w.+/=-]+", "[REDACTED]"),
            (r#"(?i)((?:api[_-]?key|token|password|secret|access[_-]?code)\s*[=:]\s*)[^\s&"',;]+"#, "[REDACTED]"),
            (r"(?i)(https?://)[^/\s@]+:[^/\s@]+@", "[REDACTED]"),
        ].into_iter().map(|(pattern, replacement)| {
            regex::Regex::new(pattern).ok().map(|regex| (regex, replacement))
        }).collect()
    });
    let Some(patterns) = patterns else {
        // Fail closed if a future edit introduces an invalid pattern.
        return "[Diagnostic text omitted: redaction unavailable]".to_string();
    };
    let mut text = content.to_string();
    for (regex, replacement) in patterns {
        if let std::borrow::Cow::Owned(redacted) = regex.replace_all(&text, *replacement) {
            text = redacted;
        }
    }
    text
}

/// Genera il contenuto di version.txt
fn generate_version_info() -> String {
    let version = crate::app_display_version();

    #[cfg(feature = "standalone")]
    let mode = "standalone";
    #[cfg(not(feature = "standalone"))]
    let mode = "normal";

    let debug = if cfg!(debug_assertions) {
        "debug"
    } else {
        "release"
    };

    let commit = option_env!("GIT_COMMIT_HASH").unwrap_or("unknown");
    let build_date = option_env!("BUILD_DATE").unwrap_or("unknown");
    let rustc = option_env!("RUSTC_VERSION").unwrap_or("unknown");
    let arch = std::env::consts::ARCH;
    let os = std::env::consts::OS;

    format!(
        "Sonarpad Diagnostics\n\
         ==================\n\n\
         Version: {}\n\
         Mode: {}\n\
         Build: {}\n\
         Commit: {}\n\
         Build date: {}\n\
         Rustc: {}\n\
         Arch: {}\n\
         OS: {}\n\
         Sentry enabled: {}\n",
        version,
        mode,
        debug,
        commit,
        build_date,
        rustc,
        arch,
        os,
        sentry_integration::is_enabled()
    )
}

/// Aggiunge un file al zip, gestendo errori gracefully.
fn add_file_to_zip<W: Write + std::io::Seek>(
    zip: &mut ZipWriter<W>,
    source_path: &Path,
    archive_name: &str,
    sanitize: bool,
) -> Result<(), String> {
    let mut file = match File::open(source_path) {
        Ok(f) => f,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            // File non esiste, skip silently
            return Ok(());
        }
        Err(e) => {
            return Err(format!("Failed to open {}: {}", archive_name, e));
        }
    };

    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)
        .map_err(|e| format!("Failed to read {archive_name}: {e}"))?;
    // All diagnostic inputs are text. Invalid UTF-8 must never bypass redaction.
    let content = String::from_utf8_lossy(&bytes);

    let final_content = if archive_name == "settings.json" {
        sanitized_settings(&content)?
    } else if sanitize {
        sanitize_content(&content)
    } else {
        content.into_owned()
    };

    let options = FileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    if let Err(e) = zip.start_file(archive_name, options) {
        return Err(format!("Failed to create {} in zip: {}", archive_name, e));
    }
    if let Err(e) = zip.write_all(final_content.as_bytes()) {
        return Err(format!("Failed to write {} to zip: {}", archive_name, e));
    }

    Ok(())
}

/// Aggiunge contenuto testuale direttamente al zip.
fn add_text_to_zip<W: Write + std::io::Seek>(
    zip: &mut ZipWriter<W>,
    archive_name: &str,
    content: &str,
) -> Result<(), String> {
    let options = FileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    if let Err(e) = zip.start_file(archive_name, options) {
        return Err(format!("Failed to create {} in zip: {}", archive_name, e));
    }
    if let Err(e) = zip.write_all(content.as_bytes()) {
        return Err(format!("Failed to write {} to zip: {}", archive_name, e));
    }
    Ok(())
}

/// Esporta i file diagnostici in un archivio zip.
///
/// Include:
/// - version.txt: informazioni sulla versione e build
/// - Sonarpad.log: file di log corrente (sanitizzato)
/// - Sonarpad.log.1: log precedente dopo la rotazione (se esiste, sanitizzato)
/// - settings.json: configurazione (sanitizzata)
/// - last_error.txt: ultimo errore fatale (se esiste)
/// - sentry_last_event_id.txt: ultimo event ID Sentry (se esiste)
pub fn export_diagnostics_zip(dest_path: &Path) -> Result<(), String> {
    let file = File::create(dest_path).map_err(|e| format!("Failed to create zip file: {}", e))?;
    write_diagnostics_zip(file).map(|_| ())
}

fn write_diagnostics_zip<W: Write + std::io::Seek>(writer: W) -> Result<W, String> {
    let settings_dir = settings::settings_dir();
    let mut zip = ZipWriter::new(writer);

    // 1. version.txt - sempre generato
    let version_info = generate_version_info();
    add_text_to_zip(&mut zip, "version.txt", &version_info)?;

    // 2. Sonarpad.log - sanitizzato
    let log_path = settings_dir.join("Sonarpad.log");
    add_file_to_zip(&mut zip, &log_path, "Sonarpad.log", true)?;

    // 3. Sonarpad.log.1 - log precedente dopo la rotazione, se esiste (sanitizzato)
    let rotated_log_path = settings_dir.join("Sonarpad.log.1");
    add_file_to_zip(&mut zip, &rotated_log_path, "Sonarpad.log.1", true)?;

    // 4. settings.json - sanitizzato
    let settings_path = settings_dir.join("settings.json");
    add_file_to_zip(&mut zip, &settings_path, "settings.json", true)?;

    // 5. last_error.txt - se esiste (sanitizzato)
    let last_error_path = settings_dir.join("last_error.txt");
    add_file_to_zip(&mut zip, &last_error_path, "last_error.txt", true)?;

    // 6. sentry_last_event_id.txt - se esiste
    let sentry_id_path = settings_dir.join("sentry_last_event_id.txt");
    add_file_to_zip(&mut zip, &sentry_id_path, "sentry_last_event_id.txt", false)?;

    // 7. last_hang.txt - se esiste (info su possibili freeze)
    let last_hang_path = settings_dir.join("last_hang.txt");
    add_file_to_zip(&mut zip, &last_hang_path, "last_hang.txt", true)?;

    // Finalizza il zip
    zip.finish()
        .map_err(|e| format!("Failed to finalize zip: {}", e))
}

// Never include saved credentials, even encrypted ones, in an exported report.
fn sanitized_settings(content: &str) -> Result<String, String> {
    fn redact(value: &mut serde_json::Value) {
        match value {
            serde_json::Value::Object(fields) => {
                for (key, value) in fields {
                    let key = key.to_ascii_lowercase();
                    if [
                        "password",
                        "secret",
                        "token",
                        "api_key",
                        "access_code",
                        "credentials",
                        "username",
                        "rai_luce_code",
                    ]
                    .iter()
                    .any(|part| key.contains(part))
                    {
                        *value = serde_json::Value::String("[REDACTED]".into());
                    } else {
                        redact(value);
                    }
                }
            }
            serde_json::Value::Array(values) => values.iter_mut().for_each(redact),
            _ => {}
        }
    }
    let mut value: serde_json::Value = serde_json::from_str(content)
        .map_err(|_| "Unable to sanitize diagnostic settings".to_string())?;
    redact(&mut value);
    // Sanitize actual strings before serializing, so escaped backslashes cannot hide paths.
    fn paths(value: &mut serde_json::Value) {
        match value {
            serde_json::Value::String(text) => *text = sanitize_content(text),
            serde_json::Value::Array(values) => values.iter_mut().for_each(paths),
            serde_json::Value::Object(fields) => fields.values_mut().for_each(paths),
            _ => {}
        }
    }
    paths(&mut value);
    serde_json::to_string_pretty(&value).map_err(|e| e.to_string())
}

const REPORT_ENDPOINT: &str = "https://sonarpad.com/api/logdeveloper.php";
const MAX_ATTACHMENT_BYTES: usize = 6 * 1024 * 1024;

pub fn valid_reply_email(email: &str) -> bool {
    let email = email.trim();
    if email.is_empty() {
        return true;
    }
    if email.len() > 254 || !email.is_ascii() {
        return false;
    }
    let Some((local, domain)) = email.split_once('@') else {
        return false;
    };
    !local.is_empty()
        && local.len() <= 64
        && !local.starts_with('.')
        && !local.ends_with('.')
        && !local.contains("..")
        && local
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b".!#$%&'*+-/=?^_`{|}~".contains(&b))
        && domain.contains('.')
        && domain.split('.').all(|label| {
            !label.is_empty()
                && label.len() <= 63
                && !label.starts_with('-')
                && !label.ends_with('-')
                && label
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'-')
        })
}

pub fn send_report(name: &str, report: &str, email: &str) -> Result<(), String> {
    send_report_to(REPORT_ENDPOINT, name, report, email, || {
        write_diagnostics_zip(std::io::Cursor::new(Vec::new())).map(|cursor| cursor.into_inner())
    })
}

fn send_report_to(
    endpoint: &str,
    name: &str,
    report: &str,
    email: &str,
    archive: impl FnOnce() -> Result<Vec<u8>, String>,
) -> Result<(), String> {
    use base64::Engine;
    if report.trim().is_empty()
        || report.len() > 32768
        || name.len() > 800
        || !valid_reply_email(email)
    {
        return Err("Invalid report fields".into());
    }
    let client = reqwest::blocking::Client::builder()
        .user_agent("Sonarpad-Windows")
        .connect_timeout(std::time::Duration::from_secs(15))
        .timeout(std::time::Duration::from_secs(90))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|_| "Unable to initialize report client")?;
    // Older Mobile endpoints accept JSON but silently ignore attachments.
    // Check support before sending any personal data.
    let capabilities: serde_json::Value = client
        .get(endpoint)
        .query(&[("capabilities", "1")])
        .send()
        .and_then(|response| response.error_for_status())
        .and_then(|response| response.json())
        .map_err(|_| "Report server unavailable or not updated")?;
    if capabilities["email_zip_v1"] != true {
        return Err("Report server does not support email attachments".into());
    }
    crate::log_debug("Diagnostics report: server supports email ZIP; preparing archive");
    let started = std::time::Instant::now();
    let zip = archive()?;
    crate::log_debug(&format!(
        "Diagnostics report: archive prepared bytes={} elapsed_ms={}",
        zip.len(),
        started.elapsed().as_millis()
    ));
    if zip.len() > MAX_ATTACHMENT_BYTES {
        return Err("Diagnostic ZIP exceeds the 6 MiB attachment limit".into());
    }
    let payload = serde_json::json!({
        "name": name.trim(), "report": report.trim(), "email": email.trim(),
        "log": generate_version_info(),
        "platform": "windows",
        "diagnostics_zip_base64": base64::engine::general_purpose::STANDARD.encode(&zip),
    });
    crate::log_debug("Diagnostics report: uploading archive");
    let response = client
        .post(endpoint)
        .header("Accept", "application/json")
        .json(&payload)
        .send()
        .map_err(|_| "Report transfer failed or timed out")?;
    crate::log_debug(&format!(
        "Diagnostics report: upload response HTTP {}",
        response.status().as_u16()
    ));
    if !response.status().is_success() {
        return Err(format!(
            "Report server returned HTTP {}",
            response.status().as_u16()
        ));
    }
    let result: serde_json::Value = response
        .json()
        .map_err(|_| "Invalid report server response")?;
    if result["ok"] != true
        || result["delivery"] != "email"
        || result["attachment_received"] != true
    {
        return Err("Server did not confirm email delivery with attachment".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diagnostic_reply_email_is_optional_and_rejects_invalid_headers() {
        for email in ["", "  ", "name+tag@example.org", " name@example.org "] {
            assert!(valid_reply_email(email));
        }
        for email in [
            "a@b@c.org",
            "a..b@example.org",
            "a@-example.org",
            "a@example.org\r\nBcc: other@example.org",
            "not an email",
        ] {
            assert!(!valid_reply_email(email));
        }
    }

    #[test]
    fn diagnostic_settings_remove_nested_credentials_and_keep_voice_preferences() {
        let source = serde_json::json!({"tts_voice":"Isabella", "gemini_api_key":"secret-value",
            "ytdlp_site_credentials":{"site":"encrypted-value"},
            "nested":{"password":"hidden", "path":r"C:\Users\someone\Documents\book.txt"}});
        let output = sanitized_settings(&source.to_string()).unwrap();
        assert!(!output.contains("secret-value"));
        assert!(!output.contains("encrypted-value"));
        assert!(!output.contains("hidden"));
        assert!(!output.contains("someone"));
        assert!(output.contains("Isabella"));
        assert!(sanitized_settings("broken JSON").is_err());
    }

    #[test]
    fn diagnostic_redaction_handles_large_unicode_logs_without_backtracking() {
        let line =
            "Streaming: stato riproduzione è pronto 日本語 https://example.com/video?item=123\n";
        let mut log = line.repeat(25000);
        log.push_str("Bearer abcdefghijklmnop\n");
        let output = sanitize_content(&log);
        assert!(output.starts_with(line));
        assert_eq!(output.lines().count(), 25001);
        assert!(!output.contains("abcdefghijklmnop"));
    }

    #[test]
    #[ignore = "Reads local diagnostic files; creates an archive in memory only, never uploads"]
    fn diagnostic_real_local_archive_can_be_prepared() {
        let archive = write_diagnostics_zip(std::io::Cursor::new(Vec::new())).unwrap();
        let bytes = archive.into_inner();
        println!("Local diagnostic ZIP prepared: {} bytes", bytes.len());
        assert!(bytes.len() <= MAX_ATTACHMENT_BYTES);
        let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
        assert!(zip.by_name("version.txt").is_ok());
        assert!(zip.by_name("Sonarpad.log").is_ok());
        let mut settings = String::new();
        zip.by_name("settings.json")
            .unwrap()
            .read_to_string(&mut settings)
            .unwrap();
        let value: serde_json::Value = serde_json::from_str(&settings).unwrap();
        assert_eq!(value["gemini_api_key"], "[REDACTED]");
    }

    #[test]
    fn diagnostic_log_redacts_tokens_and_preserves_error_details() {
        let output = sanitize_content(
            "failed: Bearer abcdefghijklmnop password=secret https://user:pass@example.com/path",
        );
        assert!(!output.contains("abcdefghijklmnop"));
        assert!(!output.contains("secret"));
        assert!(!output.contains("user:pass"));
        assert!(output.contains("failed:"));
    }

    fn server(responses: Vec<&'static str>) -> (String, std::thread::JoinHandle<Vec<String>>) {
        use std::net::TcpListener;
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = format!("http://{}", listener.local_addr().unwrap());
        let thread = std::thread::spawn(move || {
            let mut requests = Vec::new();
            for response in responses {
                let (mut stream, _) = listener.accept().unwrap();
                stream
                    .set_read_timeout(Some(std::time::Duration::from_secs(5)))
                    .unwrap();
                let mut raw = Vec::new();
                let mut byte = [0u8];
                while !raw.ends_with(b"\r\n\r\n") {
                    stream.read_exact(&mut byte).unwrap();
                    raw.push(byte[0]);
                }
                let headers = String::from_utf8(raw).unwrap();
                let length = headers
                    .lines()
                    .find_map(|line| {
                        line.to_lowercase()
                            .strip_prefix("content-length:")
                            .map(|v| v.trim().parse::<usize>().unwrap())
                    })
                    .unwrap_or(0);
                let mut body = vec![0; length];
                stream.read_exact(&mut body).unwrap();
                requests.push(format!("{headers}{}", String::from_utf8(body).unwrap()));
                write!(stream, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nConnection: close\r\nContent-Length: {}\r\n\r\n{response}", response.len()).unwrap();
            }
            requests
        });
        (address, thread)
    }

    #[test]
    fn diagnostic_report_requires_zip_email_acknowledgement() {
        let (url, handle) = server(vec![
            r#"{"email_zip_v1":true}"#,
            r#"{"ok":true,"delivery":"email","attachment_received":true}"#,
        ]);
        assert!(
            send_report_to(&url, " Name ", " Description ", "reply@example.org", || Ok(
                b"zip sample".to_vec()
            ))
            .is_ok()
        );
        let requests = handle.join().unwrap();
        assert!(requests[0].starts_with("GET /?capabilities=1"));
        let payload: serde_json::Value =
            serde_json::from_str(requests[1].split("\r\n\r\n").nth(1).unwrap()).unwrap();
        assert_eq!(payload["name"], "Name");
        assert_eq!(payload["email"], "reply@example.org");
        assert_eq!(payload["report"], "Description");
        assert_eq!(payload["diagnostics_zip_base64"], "emlwIHNhbXBsZQ==");
        let (url, handle) = server(vec![r#"{"email_zip_v1":true}"#, r#"{"ok":true}"#]);
        assert!(send_report_to(&url, "", "Description", "", || Ok(vec![])).is_err());
        handle.join().unwrap();
    }

    #[test]
    fn diagnostic_report_does_not_upload_to_old_server() {
        let (url, handle) = server(vec![r#"{"ok":false}"#]);
        assert!(
            send_report_to(&url, "", "Description", "", || panic!(
                "must not collect diagnostics"
            ))
            .is_err()
        );
        assert_eq!(handle.join().unwrap().len(), 1);
    }
}
