//! Gemini speech with the user's existing, encrypted-at-rest API key.
use base64::Engine;
use serde_json::{Value, json};
use std::io::Cursor;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use crate::settings::VoiceInfo;

pub(crate) const MAX_PARALLEL_REQUESTS: usize = 8;
static REQUEST_SLOTS: tokio::sync::Semaphore =
    tokio::sync::Semaphore::const_new(MAX_PARALLEL_REQUESTS);

const MODEL: &str = "gemini-3.8-flash-tts";
const VOICES: &[&str] = &[
    "Zephyr",
    "Puck",
    "Charon",
    "Kore",
    "Fenrir",
    "Leda",
    "Orus",
    "Aoede",
    "Callirrhoe",
    "Autonoe",
    "Enceladus",
    "Iapetus",
    "Umbriel",
    "Algieba",
    "Despina",
    "Erinome",
    "Algenib",
    "Rasalgethi",
    "Laomedeia",
    "Achernar",
    "Alnilam",
    "Schedar",
    "Gacrux",
    "Pulcherrima",
    "Achird",
    "Zubenelgenubi",
    "Vindemiatrix",
    "Sadachbia",
    "Sadaltager",
    "Sulafat",
];

pub fn voices() -> Vec<VoiceInfo> {
    VOICES
        .iter()
        .map(|name| VoiceInfo {
            short_name: (*name).to_string(),
            locale: String::new(),
            is_multilingual: true,
        })
        .collect()
}

pub const LANGUAGES: &[(&str, &str)] = &[
    ("", "options.choice.automatic"),
    ("it-IT", "options.lang.it"),
    ("en-US", "options.lang.en"),
    ("es-ES", "options.lang.es"),
    ("pt-PT", "options.lang.pt"),
    ("pt-BR", "options.lang.pt_br"),
    ("sv-SE", "options.lang.sv"),
    ("vi-VN", "options.lang.vi"),
    ("cs-CZ", "options.lang.cs"),
    ("pl-PL", "options.lang.pl"),
    ("fr-FR", "options.lang.fr"),
    ("sr-RS", "options.lang.sr"),
    ("uk-UA", "options.lang.uk"),
    ("lt-LT", "options.lang.lt"),
    ("ru-RU", "options.lang.ru"),
    ("zh-CN", "options.lang.zh"),
    ("hi-IN", "options.lang.hi"),
    ("de-DE", "options.lang.de"),
    ("id-ID", "voice.lang.id"),
];

fn language_instruction(language: &str) -> &'static str {
    match language.trim().to_ascii_lowercase().as_str() {
        "it-it" => "Utilizza esclusivamente un timbro e una pronuncia italiani.",
        "en-us" => "Use exclusively an American English vocal timbre and pronunciation.",
        "es-es" => "Utiliza exclusivamente un timbre y una pronunciación del español de España.",
        "pt-pt" => "Utiliza exclusivamente um timbre e uma pronúncia do português europeu.",
        "pt-br" => "Utilize exclusivamente um timbre e uma pronúncia do português brasileiro.",
        "sv-se" => "Använd uteslutande svensk röstklang och svenskt uttal.",
        "vi-vn" => "Chỉ sử dụng âm sắc và cách phát âm tiếng Việt.",
        "cs-cz" => "Používej výhradně české zabarvení hlasu a českou výslovnost.",
        "pl-pl" => "Używaj wyłącznie polskiej barwy głosu i polskiej wymowy.",
        "fr-fr" => "Utilise exclusivement un timbre et une prononciation français.",
        "sr-rs" => "Користи искључиво српску боју гласа и српски изговор.",
        "uk-ua" => "Використовуй виключно український тембр голосу та українську вимову.",
        "lt-lt" => "Naudok tik lietuvišką balso tembrą ir lietuvišką tarimą.",
        "ru-ru" => "Используй исключительно русский тембр голоса и русское произношение.",
        "zh-cn" => "仅使用标准普通话的音色和发音。",
        "hi-in" => "केवल हिंदी स्वर और हिंदी उच्चारण का उपयोग करें।",
        "de-de" => "Verwende ausschließlich eine deutsche Stimmfärbung und deutsche Aussprache.",
        "id-id" => "Gunakan hanya warna suara dan pelafalan bahasa Indonesia.",
        _ => "",
    }
}

fn request_body(text: &str, voice: &str, rate: i32, pitch: i32, language: &str) -> Value {
    let mut part = json!({"text": text});
    let mut style = language_instruction(language).to_string();
    if rate != 0 || pitch != 0 {
        if style.is_empty() {
            style.push_str("Read in the original language.");
        }
        style.push_str(&format!(
            " Speaking speed: {} percent of normal. Pitch: {:+} semitones relative to normal.",
            100 + rate.clamp(-90, 100),
            pitch.clamp(-12, 12)
        ));
    }
    if !style.is_empty() {
        part["speech_metadata"] = json!({"style": style});
    }
    json!({
        "contents": [{"role": "user", "parts": [part]}],
        "generationConfig": {
            "responseModalities": ["AUDIO"],
            "speechConfig": {"voiceConfig": {"voice": voice}}
        }
    })
}

fn response_audio(response: &Value, volume: i32) -> Result<Vec<u8>, String> {
    let finish = response["candidates"][0]["finishReason"]
        .as_str()
        .unwrap_or("");
    let blocked = response["promptFeedback"]["blockReason"]
        .as_str()
        .unwrap_or("");
    if !blocked.is_empty() || matches!(finish, "SAFETY" | "PROHIBITED_CONTENT" | "RECITATION") {
        return Err("Gemini TTS: blocked response.".to_string());
    }
    if finish == "MALFORMED_RESPONSE" {
        return Err("Gemini TTS: malformed response.".to_string());
    }
    if finish == "MAX_TOKENS" {
        return Err("Gemini TTS: audio generation was truncated.".to_string());
    }
    let encoded = response["candidates"][0]["content"]["parts"]
        .as_array()
        .into_iter()
        .flatten()
        .find_map(|part| part["inlineData"]["data"].as_str())
        .ok_or_else(|| "Gemini TTS: no audio returned.".to_string())?;
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(encoded)
        .map_err(|_| "Gemini TTS: invalid audio encoding.".to_string())?;
    let mut reader = hound::WavReader::new(Cursor::new(&bytes))
        .map_err(|_| "Gemini TTS: invalid WAV audio.".to_string())?;
    let spec = reader.spec();
    if spec.bits_per_sample != 16
        || spec.sample_format != hound::SampleFormat::Int
        || reader.duration() == 0
    {
        return Err("Gemini TTS: unsupported or empty audio.".to_string());
    }
    let samples = reader
        .samples::<i16>()
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| "Gemini TTS: invalid WAV samples.".to_string())?;
    if !samples.iter().any(|sample| *sample != 0) {
        return Err("Gemini TTS: empty WAV signal.".to_string());
    }
    if volume == 100 {
        return Ok(bytes);
    }
    let mut output = Cursor::new(Vec::new());
    {
        let mut writer = hound::WavWriter::new(&mut output, spec).map_err(|e| e.to_string())?;
        let gain = volume.clamp(0, 100) as f32 / 100.0;
        for sample in samples {
            writer
                .write_sample((f32::from(sample) * gain) as i16)
                .map_err(|e| e.to_string())?;
        }
        writer.finalize().map_err(|e| e.to_string())?;
    }
    Ok(output.into_inner())
}

pub async fn synthesize_wav_bytes(
    text: &str,
    voice: &str,
    rate: i32,
    pitch: i32,
    volume: i32,
    cancel: &AtomicBool,
) -> Result<Vec<u8>, String> {
    let settings = crate::settings::load_settings();
    if settings.gemini_api_key.trim().is_empty() {
        return Err(crate::i18n::tr(
            settings.language,
            "gemini_tts.key_required",
        ));
    }
    if !VOICES.contains(&voice) {
        return Err("Gemini TTS: unknown voice.".to_string());
    }
    if cancel.load(Ordering::Relaxed) {
        return Err("Cancelled".to_string());
    }
    let client = reqwest::Client::builder()
        // Same timeout and transient-error retry interval as audio-description analysis.
        .timeout(Duration::from_secs(8 * 60))
        .build()
        .map_err(|e| e.to_string())?;
    let body = request_body(text, voice, rate, pitch, &settings.gemini_tts_language);
    retry_synthesis(|| async {
        let _permit = REQUEST_SLOTS.acquire().await.map_err(|_| SynthesisError {
            message: "Gemini TTS: request queue closed.".into(), retry: false,
        })?;
        let response = client
            .post(format!("https://generativelanguage.googleapis.com/v1beta/models/{MODEL}:generateContent"))
            .header("x-goog-api-key", settings.gemini_api_key.trim())
            .json(&body).send().await.map_err(transport_error)?;
        let status = response.status();
        if !status.is_success() {
            // Inspect only to classify; never expose response text or credentials in logs.
            let data: Value = response.json().await.unwrap_or(Value::Null);
            let exhausted_credit = data["error"]["message"].as_str().is_some_and(|message|
                message.to_ascii_lowercase().contains("prepayment credits are depleted"));
            return Err(SynthesisError {
                message: format!("Gemini TTS: HTTP {}", status.as_u16()),
                retry: !exhausted_credit && retryable_status(status.as_u16()),
            });
        }
        let data: Value = response.json().await.map_err(transport_error)?;
        response_audio(&data, volume).map_err(|message| SynthesisError {
            retry: true, message,
        })
    }, cancel, Duration::from_secs(5)).await
}

struct SynthesisError {
    message: String,
    retry: bool,
}

fn retryable_status(status: u16) -> bool {
    matches!(status, 408 | 429 | 500 | 502 | 503 | 504)
}

fn transport_error(error: reqwest::Error) -> SynthesisError {
    SynthesisError {
        retry: error.is_timeout()
            || error.is_connect()
            || error.is_body()
            || error.is_decode()
            || error.is_request(),
        message: format!("Gemini TTS: {}", error.without_url()),
    }
}

async fn retry_synthesis<F, Fut>(
    mut operation: F,
    cancel: &AtomicBool,
    delay: Duration,
) -> Result<Vec<u8>, String>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Result<Vec<u8>, SynthesisError>>,
{
    let mut attempt = 0u64;
    loop {
        if cancel.load(Ordering::Relaxed) {
            return Err("Cancelled".into());
        }
        let result = tokio::select! {
            result = operation() => result,
            () = async {
                while !cancel.load(Ordering::Relaxed) {
                    tokio::time::sleep(Duration::from_millis(100)).await;
                }
            } => return Err("Cancelled".into()),
        };
        match result {
            Ok(bytes) => return Ok(bytes),
            Err(error) if error.retry => {
                attempt = attempt.saturating_add(1);
                crate::log_debug(&format!("Gemini TTS: retry {attempt}; {}", error.message));
                if !crate::tts_engine::async_sleep_with_cancellation(cancel, delay).await {
                    return Err("Cancelled".into());
                }
            }
            Err(error) => return Err(error.message),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gemini_retry_recovers_same_segment_after_transient_errors() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let mut calls = 0;
        let audio = rt
            .block_on(retry_synthesis(
                || {
                    calls += 1;
                    std::future::ready(if calls < 5 {
                        Err(SynthesisError {
                            message: "Gemini TTS: HTTP 503".into(),
                            retry: true,
                        })
                    } else {
                        Ok(vec![1, 2, 3])
                    })
                },
                &AtomicBool::new(false),
                Duration::ZERO,
            ))
            .unwrap();
        assert_eq!(calls, 5);
        assert_eq!(audio, vec![1, 2, 3]);
        for status in [408, 429, 500, 502, 503, 504] {
            assert!(retryable_status(status));
        }
        for status in [400, 401, 403, 404] {
            assert!(!retryable_status(status));
        }
    }

    #[test]
    fn gemini_retry_stops_on_permanent_errors_and_recovers_invalid_audio() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        for message in [
            "Gemini TTS: blocked response.",
            "Gemini TTS: malformed response.",
            "Gemini TTS: no audio returned.",
            "Gemini TTS: unsupported or empty audio.",
            "Gemini TTS: audio generation was truncated.",
        ] {
            let mut calls = 0;
            let result = rt
                .block_on(retry_synthesis(
                    || {
                        calls += 1;
                        std::future::ready(if calls < 10 {
                            Err(SynthesisError {
                                message: message.into(),
                                retry: true,
                            })
                        } else {
                            Ok(vec![1, 2])
                        })
                    },
                    &AtomicBool::new(false),
                    Duration::ZERO,
                ))
                .unwrap();
            assert_eq!(calls, 10);
            assert_eq!(result, vec![1, 2]);
        }
        let mut calls = 0;
        assert!(
            rt.block_on(retry_synthesis(
                || {
                    calls += 1;
                    std::future::ready(Err(SynthesisError {
                        message: "Gemini TTS: HTTP 403".into(),
                        retry: false,
                    }))
                },
                &AtomicBool::new(false),
                Duration::ZERO
            ))
            .is_err()
        );
        assert_eq!(calls, 1);
    }

    #[test]
    fn gemini_retry_cancellation_interrupts_requests_and_waits() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            for in_flight in [true, false] {
                let cancel = AtomicBool::new(false);
                let stop = async {
                    tokio::time::sleep(Duration::from_millis(20)).await;
                    cancel.store(true, Ordering::Relaxed);
                };
                let request = retry_synthesis(
                    || async {
                        if in_flight {
                            std::future::pending::<()>().await;
                        }
                        Err(SynthesisError {
                            message: "timeout".into(),
                            retry: true,
                        })
                    },
                    &cancel,
                    Duration::from_secs(5),
                );
                let (result, ()) = tokio::time::timeout(Duration::from_secs(1), async {
                    tokio::join!(request, stop)
                })
                .await
                .expect("cancellation must be prompt");
                assert_eq!(result.unwrap_err(), "Cancelled");
            }
        });
    }

    #[test]
    fn gemini_settings_and_voice_tags_round_trip_independently() {
        use crate::settings::{
            AppSettings, TtsEngine, TtsTuning, set_tts_tuning_for_engine, tts_tuning_for_engine,
        };
        let mut settings = AppSettings::default();
        let edge = tts_tuning_for_engine(&settings, TtsEngine::Edge);
        let google = tts_tuning_for_engine(&settings, TtsEngine::Google);
        let tuning = TtsTuning::new(20, -2, 80);
        set_tts_tuning_for_engine(&mut settings, TtsEngine::Gemini, tuning);
        let restored: AppSettings =
            serde_json::from_str(&serde_json::to_string(&settings).unwrap()).unwrap();
        assert!(tts_tuning_for_engine(&restored, TtsEngine::Gemini) == tuning);
        assert!(tts_tuning_for_engine(&restored, TtsEngine::Edge) == edge);
        assert!(tts_tuning_for_engine(&restored, TtsEngine::Google) == google);
        assert_eq!(
            serde_json::to_string(&TtsEngine::Gemini).unwrap(),
            "\"gemini\""
        );
        assert_eq!(voices().len(), 30);
        assert!(voices().iter().all(|voice| voice.is_multilingual));
    }

    #[test]
    fn gemini_language_instruction_is_localized_and_separate_from_spoken_text() {
        for (code, _) in LANGUAGES.iter().skip(1) {
            let body = request_body("Harry Potter e la pietra filosofale", "Leda", 0, 0, code);
            assert!(
                body["generationConfig"]["speechConfig"]
                    .get("languageCode")
                    .is_none()
            );
            let part = &body["contents"][0]["parts"][0];
            assert_eq!(part["text"], "Harry Potter e la pietra filosofale");
            assert!(!language_instruction(code).is_empty());
            assert_eq!(part["speech_metadata"]["style"], language_instruction(code));
        }
        for code in ["", "auto", "invalid"] {
            let body = request_body("viziosa", "Leda", 0, 0, code);
            assert!(
                body["generationConfig"]["speechConfig"]
                    .get("languageCode")
                    .is_none()
            );
            assert_eq!(body["contents"][0]["parts"][0], json!({"text": "viziosa"}));
        }
        let tuned = request_body("viziosa", "Leda", 15, -2, "it-IT");
        let style = tuned["contents"][0]["parts"][0]["speech_metadata"]["style"]
            .as_str()
            .unwrap();
        assert!(style.starts_with("Utilizza esclusivamente un timbro e una pronuncia italiani."));
        assert!(style.contains("115 percent of normal"));
        assert!(style.contains("Pitch: -2 semitones"));
        assert_eq!(LANGUAGES.len(), 19);
        for (_, key) in LANGUAGES {
            assert_ne!(
                crate::i18n::tr(crate::settings::Language::Italian, key),
                *key
            );
        }
    }

    #[test]
    fn gemini_language_persists_in_settings_and_profiles_and_defaults_to_auto() {
        use crate::settings::{
            AppSettings, VoiceProfile, apply_voice_profile_to_settings_fields,
            voice_profile_from_settings_fields,
        };
        let mut settings = AppSettings::default();
        settings.gemini_tts_language = "pt-BR".into();
        let mut json = serde_json::to_value(&settings).unwrap();
        let restored: AppSettings = serde_json::from_value(json.clone()).unwrap();
        assert_eq!(restored.gemini_tts_language, "pt-BR");
        json.as_object_mut().unwrap().remove("gemini_tts_language");
        let old: AppSettings = serde_json::from_value(json).unwrap();
        assert!(old.gemini_tts_language.is_empty());
        let profile = voice_profile_from_settings_fields("Test".into(), &settings);
        let restored_profile: VoiceProfile =
            serde_json::from_value(serde_json::to_value(profile).unwrap()).unwrap();
        let mut target = AppSettings::default();
        apply_voice_profile_to_settings_fields(&mut target, &restored_profile);
        assert_eq!(target.gemini_tts_language, "pt-BR");
    }

    #[test]
    fn gemini_parallel_requests_are_bounded_ordered_and_cancellable() {
        use std::sync::atomic::AtomicUsize;
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let active = AtomicUsize::new(0);
            let peak = AtomicUsize::new(0);
            let results = futures_util::future::join_all((0..32).map(|index| {
                let active = &active;
                let peak = &peak;
                async move {
                    let _permit = REQUEST_SLOTS.acquire().await.unwrap();
                    let count = active.fetch_add(1, Ordering::SeqCst) + 1;
                    peak.fetch_max(count, Ordering::SeqCst);
                    tokio::time::sleep(Duration::from_millis((8 - index % 8) * 2)).await;
                    active.fetch_sub(1, Ordering::SeqCst);
                    index
                }
            }))
            .await;
            assert_eq!(peak.load(Ordering::SeqCst), 8);
            assert_eq!(results, (0..32).collect::<Vec<_>>());
            {
                let _held = REQUEST_SLOTS.acquire_many(8).await.unwrap();
                let cancel = AtomicBool::new(false);
                let request = retry_synthesis(
                    || async {
                        let _permit = REQUEST_SLOTS.acquire().await.unwrap();
                        Ok(vec![1])
                    },
                    &cancel,
                    Duration::ZERO,
                );
                let stop = async {
                    tokio::time::sleep(Duration::from_millis(20)).await;
                    cancel.store(true, Ordering::Relaxed);
                };
                let (result, ()) = tokio::time::timeout(Duration::from_secs(1), async {
                    tokio::join!(request, stop)
                })
                .await
                .unwrap();
                assert_eq!(result.unwrap_err(), "Cancelled");
            }
            assert_eq!(REQUEST_SLOTS.available_permits(), 8);
        });
        assert_eq!(
            crate::tts_engine::audiobook_synthesis_parallelism(
                crate::settings::TtsEngine::Gemini,
                "Achird"
            ),
            8
        );
    }

    #[test]
    fn gemini_rejects_empty_silent_and_invalid_media() {
        for samples in [vec![], vec![0i16; 32]] {
            let mut wav = Cursor::new(Vec::new());
            {
                let mut writer = hound::WavWriter::new(
                    &mut wav,
                    hound::WavSpec {
                        channels: 1,
                        sample_rate: 24000,
                        bits_per_sample: 16,
                        sample_format: hound::SampleFormat::Int,
                    },
                )
                .unwrap();
                for sample in samples {
                    writer.write_sample(sample).unwrap();
                }
                writer.finalize().unwrap();
            }
            let response = json!({"candidates": [{"content": {"parts": [{"inlineData": {
                "data": base64::engine::general_purpose::STANDARD.encode(wav.into_inner())
            }}]}}]});
            assert!(response_audio(&response, 100).is_err());
        }
        let response = json!({"candidates": [{"content": {"parts": [{"inlineData": {
            "data": base64::engine::general_purpose::STANDARD.encode(b"invalid media")
        }}]}}]});
        assert!(response_audio(&response, 100).is_err());
    }

    #[test]
    fn transcript_and_style_are_separate() {
        let body = request_body("Ciao, mondo!", "Puck", 20, -2, "");
        assert_eq!(body["contents"][0]["parts"][0]["text"], "Ciao, mondo!");
        assert_eq!(
            body["generationConfig"]["speechConfig"]["voiceConfig"]["voice"],
            "Puck"
        );
        assert!(body["contents"][0]["parts"][0]["speech_metadata"].is_object());
        assert!(
            request_body("Hello", "Kore", 0, 0, "")["contents"][0]["parts"][0]
                .get("speech_metadata")
                .is_none()
        );
    }

    #[test]
    fn audio_response_preserves_wav_and_adjusts_volume() {
        let mut wav = Cursor::new(Vec::new());
        {
            let mut writer = hound::WavWriter::new(
                &mut wav,
                hound::WavSpec {
                    channels: 1,
                    sample_rate: 24000,
                    bits_per_sample: 16,
                    sample_format: hound::SampleFormat::Int,
                },
            )
            .unwrap();
            writer.write_sample(1000i16).unwrap();
            writer.finalize().unwrap();
        }
        let original = wav.into_inner();
        let response = json!({"candidates": [{"finishReason": "STOP", "content": {"parts": [{
            "inlineData": {"mimeType": "audio/wav", "data": base64::engine::general_purpose::STANDARD.encode(&original)}
        }]}}]});
        assert_eq!(response_audio(&response, 100).unwrap(), original);
        let adjusted = response_audio(&response, 50).unwrap();
        let mut reader = hound::WavReader::new(Cursor::new(adjusted)).unwrap();
        assert_eq!(reader.spec().sample_rate, 24000);
        assert_eq!(reader.samples::<i16>().next().unwrap().unwrap(), 500);
        assert!(response_audio(&json!({"steps": []}), 100).is_err());
    }
}
