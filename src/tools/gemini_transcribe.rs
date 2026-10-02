use base64::Engine;
use reqwest::blocking::Client;
use reqwest::header::CONTENT_TYPE;
use serde_json::Value;
use std::fs;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

fn language_hint(language: crate::settings::Language) -> &'static str {
    match language {
        crate::settings::Language::Italian => "Italian",
        crate::settings::Language::German => "German",
        crate::settings::Language::English => "English",
        crate::settings::Language::Spanish => "Spanish",
        crate::settings::Language::Portuguese => "Portuguese (Portugal)",
        crate::settings::Language::PortugueseBrazilian => "Portuguese (Brazil)",
        crate::settings::Language::Swedish => "Swedish",
        crate::settings::Language::Vietnamese => "Vietnamese",
        crate::settings::Language::Czech => "Czech",
        crate::settings::Language::Polish => "Polish",
        crate::settings::Language::French => "French",
        crate::settings::Language::Serbian => "Serbian",
        crate::settings::Language::Ukrainian => "Ukrainian",
        crate::settings::Language::Lithuanian => "Lithuanian",
        crate::settings::Language::Russian => "Russian",
        crate::settings::Language::Chinese => "Chinese",
        crate::settings::Language::Hindi => "Hindi",
    }
}

pub fn transcribe_wav(
    wav_path: &Path,
    api_key: &str,
    language: crate::settings::Language,
    cancel: &Arc<AtomicBool>,
) -> Result<String, String> {
    let _power_awake = crate::power_awake::acquire("media-transcription");
    if cancel.load(Ordering::Relaxed) {
        return Err("cancelled".to_string());
    }

    let audio = fs::read(wav_path).map_err(|e| format!("read wav failed: {e}"))?;
    if audio.is_empty() {
        return Err("empty audio input".to_string());
    }

    // Keep payload bounded for inlineData requests.
    const MAX_INLINE_BYTES: usize = 18 * 1024 * 1024;
    if audio.len() > MAX_INLINE_BYTES {
        return Err(format!(
            "audio too large for Gemini inline upload ({} bytes > {} bytes)",
            audio.len(),
            MAX_INLINE_BYTES
        ));
    }

    let prompt = format!(
        "Transcribe this audio verbatim. Return only plain text, no markdown. \
If speech language is clear, prefer {}.",
        language_hint(language)
    );
    let data_b64 = base64::engine::general_purpose::STANDARD.encode(audio);
    let body = serde_json::json!({
        "contents": [
            {
                "role": "user",
                "parts": [
                    { "text": prompt },
                    {
                        "inlineData": {
                            "mimeType": "audio/wav",
                            "data": data_b64
                        }
                    }
                ]
            }
        ]
    });

    let url = format!(
        "https://generativelanguage.googleapis.com/v1beta/models/gemini-2.0-flash:generateContent?key={}",
        api_key
    );
    let client = Client::builder()
        .connect_timeout(std::time::Duration::from_secs(20))
        .timeout(std::time::Duration::from_secs(60 * 20))
        .build()
        .map_err(|e| format!("http client build failed: {e}"))?;

    if cancel.load(Ordering::Relaxed) {
        return Err("cancelled".to_string());
    }

    let response = client
        .post(url)
        .header(CONTENT_TYPE, "application/json")
        .json(&body)
        .send()
        .map_err(|e| format!("gemini request failed: {e}"))?;
    let status = response.status();
    let raw = response
        .text()
        .map_err(|e| format!("gemini response read failed: {e}"))?;
    if !status.is_success() {
        return Err(format!("gemini HTTP {}: {}", status, raw));
    }

    let v: Value = serde_json::from_str(&raw)
        .map_err(|e| format!("gemini response json parse failed: {e}"))?;
    let Some(candidates) = v.get("candidates").and_then(|x| x.as_array()) else {
        return Err("gemini response missing candidates".to_string());
    };
    for c in candidates {
        if let Some(parts) = c
            .get("content")
            .and_then(|x| x.get("parts"))
            .and_then(|x| x.as_array())
        {
            let mut text = String::new();
            for part in parts {
                if let Some(t) = part.get("text").and_then(|x| x.as_str())
                    && !t.trim().is_empty()
                {
                    if !text.is_empty() {
                        text.push('\n');
                    }
                    text.push_str(t.trim());
                }
            }
            if !text.trim().is_empty() {
                return Ok(text);
            }
        }
    }

    Err("gemini response did not contain transcription text".to_string())
}
