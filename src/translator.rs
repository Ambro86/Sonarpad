use flate2::read::GzDecoder;
use rand::Rng;
use reqwest::Client;
use reqwest::header::{
    ACCEPT, ACCEPT_ENCODING, ACCEPT_LANGUAGE, AUTHORIZATION, CONTENT_TYPE, HeaderMap, HeaderName,
    HeaderValue, ORIGIN, REFERER, USER_AGENT,
};
use serde_json::{Value, json};
use std::fmt;
use std::io::Read;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub struct TranslatorDeepLFree {
    pub source_lang: Option<String>,
    pub target_lang: String,
    endpoint: String,
    client: Client,
}

#[derive(Debug)]
pub enum TranslatorError {
    HttpClientBuild(String),
    Network(String),
    HttpStatus { status: u16, body: String },
    ReadResponse(String),
    DecodeGzip(String),
    DecodeUtf8(String),
    ParseJson(String),
    MissingTranslatedText,
    Cancelled,
    PartialSummary { summary: String, error: String },
}

impl fmt::Display for TranslatorError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TranslatorError::HttpClientBuild(err) => {
                write!(f, "Failed to build HTTP client: {err}")
            }
            TranslatorError::Network(err) => {
                write!(f, "Network error: {err}")
            }
            TranslatorError::HttpStatus { status, body } => {
                write!(f, "HTTP error {status}: {body}")
            }
            TranslatorError::ReadResponse(err) => {
                write!(f, "Failed to read response body: {err}")
            }
            TranslatorError::DecodeGzip(err) => {
                write!(f, "Failed to decode gzip response: {err}")
            }
            TranslatorError::DecodeUtf8(err) => {
                write!(f, "Failed to decode response as UTF-8: {err}")
            }
            TranslatorError::ParseJson(err) => {
                write!(f, "Failed to parse JSON response: {err}")
            }
            TranslatorError::MissingTranslatedText => {
                write!(f, "Translated text field was not found in the response")
            }
            TranslatorError::Cancelled => {
                write!(f, "Translation canceled")
            }
            TranslatorError::PartialSummary { error, .. } => {
                write!(f, "{error}")
            }
        }
    }
}

impl std::error::Error for TranslatorError {}

pub struct TranslatorGoogleFree {
    pub source_lang: String,
    pub target_lang: String,
    client: Client,
}

const GOOGLE_TRANSLATE_URL: &str = "https://translate-pa.googleapis.com/v1/translate";
const GOOGLE_TRANSLATE_API_KEY: &str = "AIzaSyDLEeFI5OtFBwYBIoK_jj5m32rZK5CkCXA";

#[cfg(test)]
mod google_sentence_tests {
    use super::TranslatorGoogleFree;

    #[test]
    fn preserves_sentences_and_separators() {
        let text = "Prima frase, lunga.\nSeconda: Terza? Quarta! Fine";
        let chunks = TranslatorGoogleFree::split_translation_chunks(text, 12);
        assert_eq!(chunks.concat(), text);
        assert_eq!(chunks[0], "Prima frase, lunga.");
        for chunk in &chunks[..chunks.len() - 1] {
            assert!(chunk.ends_with(['.', ':', '?', '!']));
        }
    }

    #[test]
    fn keeps_long_sentences_and_unicode() {
        let text = format!("{}! Fine.", "è".repeat(2000));
        let chunks = TranslatorGoogleFree::split_translation_chunks(&text, 1500);
        assert_eq!(chunks.concat(), text);
        assert_eq!(chunks[0].chars().count(), 2001);
        assert_eq!(
            TranslatorGoogleFree::split_translation_chunks("senza punteggiatura", 3),
            vec!["senza punteggiatura"]
        );
    }

    #[test]
    fn keeps_decimals_urls_and_empty_text() {
        let text = "Costa 3.14: visita https://example.com/path! Bene?";
        let chunks = TranslatorGoogleFree::split_translation_chunks(text, 10);
        assert_eq!(chunks.concat(), text);
        assert!(
            chunks
                .iter()
                .any(|c| c.contains("https://example.com/path!"))
        );
        assert_eq!(
            TranslatorGoogleFree::split_translation_chunks("", 1500),
            vec![""]
        );
    }
}

impl TranslatorGoogleFree {
    // A soft limit: an entire sentence may exceed it, but is never truncated.
    fn split_translation_chunks(text: &str, max_chars: usize) -> Vec<String> {
        let mut chunks = Vec::new();
        let mut start = 0;
        let mut sentence_start = 0;
        let mut count = 0;
        for (offset, ch) in text.char_indices() {
            let end = offset + ch.len_utf8();
            let boundary = matches!(ch, '.' | ':' | '?' | '!' | '。' | '：' | '？' | '！')
                && (end == text.len() || text[end..].starts_with(char::is_whitespace));
            if !boundary && end != text.len() {
                continue;
            }
            let sentence_count = text[sentence_start..end].chars().count();
            if count > 0 && count + sentence_count > max_chars {
                chunks.push(text[start..sentence_start].to_string());
                start = sentence_start;
                count = 0;
            }
            count += sentence_count;
            sentence_start = end;
        }
        chunks.push(text[start..].to_string());
        chunks
    }

    pub fn new(
        target_lang: impl Into<String>,
        source_lang: impl Into<String>,
    ) -> Result<Self, TranslatorError> {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(|err| TranslatorError::HttpClientBuild(err.to_string()))?;

        Ok(Self {
            source_lang: source_lang.into(),
            target_lang: target_lang.into(),
            client,
        })
    }

    fn normalized_google_lang(lang: &str) -> String {
        match lang.trim().to_ascii_lowercase().as_str() {
            "iw" => "he".to_string(),
            "jw" => "jv".to_string(),
            "zh-cn" | "zh_tw" | "zh-tw" => "zh".to_string(),
            "" => "auto".to_string(),
            other => other.to_string(),
        }
    }

    fn parse_result(response: Value) -> Result<String, TranslatorError> {
        let mut translated = String::new();

        if let Some(sentences) = response.get(1).and_then(Value::as_array) {
            for sentence in sentences {
                if let Some(text) = sentence.get(0).and_then(Value::as_str) {
                    translated.push_str(text);
                }
            }
        }
        if translated.is_empty()
            && let Some(text) = response.get(0).and_then(Value::as_str)
        {
            translated.push_str(text);
        }

        if translated.is_empty() {
            Err(TranslatorError::MissingTranslatedText)
        } else {
            Ok(translated)
        }
    }

    async fn translate_with_endpoint(
        &self,
        endpoint: &str,
        text: &str,
    ) -> Result<String, TranslatorError> {
        let source_lang = Self::normalized_google_lang(&self.source_lang);
        let target_lang = Self::normalized_google_lang(&self.target_lang);

        let response = self
            .client
            .get(endpoint)
            .header(USER_AGENT, HeaderValue::from_static("Mozilla/5.0"))
            .header(
                CONTENT_TYPE,
                HeaderValue::from_static("application/json+protobuf"),
            )
            .query(&[
                ("params.client", "gtx"),
                ("query.source_language", source_lang.as_str()),
                ("query.target_language", target_lang.as_str()),
                ("query.display_language", target_lang.as_str()),
                ("query.text", text),
                ("key", GOOGLE_TRANSLATE_API_KEY),
                ("data_types", "TRANSLATION"),
                ("data_types", "SENTENCE_SPLITS"),
            ])
            .send()
            .await
            .map_err(|err| TranslatorError::Network(err.to_string()))?;

        let status = response.status();
        let body = response
            .text()
            .await
            .map_err(|err| TranslatorError::ReadResponse(err.to_string()))?;

        if !status.is_success() {
            return Err(TranslatorError::HttpStatus {
                status: status.as_u16(),
                body,
            });
        }

        let response_json: Value = serde_json::from_str(&body)
            .map_err(|err| TranslatorError::ParseJson(err.to_string()))?;

        Self::parse_result(response_json)
    }

    pub async fn translate(&self, text: &str) -> Result<String, TranslatorError> {
        self.translate_with_endpoint(GOOGLE_TRANSLATE_URL, text)
            .await
    }

    pub async fn translate_chunked_cancellable_with_progress(
        &self,
        text: &str,
        cancel: Option<&AtomicBool>,
        progress: &mut dyn FnMut(usize, usize),
    ) -> Result<String, TranslatorError> {
        const MAX_CHUNK_CHARS: usize = 1_500;
        const MAX_CHUNK_ATTEMPTS: usize = 3;

        let chunks = Self::split_translation_chunks(text, MAX_CHUNK_CHARS);
        if chunks.len() == 1 {
            if cancel.is_some_and(|flag| flag.load(Ordering::Relaxed)) {
                return Err(TranslatorError::Cancelled);
            }
            let translated = self.translate(text).await?;
            progress(1, 1);
            return Ok(translated);
        }

        let mut translated_text = String::new();

        for (index, chunk) in chunks.iter().enumerate() {
            if cancel.is_some_and(|flag| flag.load(Ordering::Relaxed)) {
                return Err(TranslatorError::Cancelled);
            }
            if index > 0 {
                let delay_seconds = rand::thread_rng().gen_range(1u64..=3u64);
                tokio::time::sleep(Duration::from_secs(delay_seconds)).await;
            }

            if chunk.trim().is_empty() {
                translated_text.push_str(chunk);
                progress(index + 1, chunks.len());
                continue;
            }

            let mut last_error = None;
            for attempt in 1..=MAX_CHUNK_ATTEMPTS {
                if cancel.is_some_and(|flag| flag.load(Ordering::Relaxed)) {
                    return Err(TranslatorError::Cancelled);
                }
                match self.translate(chunk).await {
                    Ok(translated_chunk) => {
                        // Preserve separators even if Google trims the response.
                        let leading = chunk.len() - chunk.trim_start().len();
                        let trailing = chunk.trim_end().len();
                        translated_text.push_str(&chunk[..leading]);
                        translated_text.push_str(translated_chunk.trim());
                        translated_text.push_str(&chunk[trailing..]);
                        progress(index + 1, chunks.len());
                        last_error = None;
                        break;
                    }
                    Err(err) => {
                        last_error = Some(err);
                        if attempt < MAX_CHUNK_ATTEMPTS {
                            let delay_seconds = rand::thread_rng().gen_range(1u64..=3u64);
                            tokio::time::sleep(Duration::from_secs(delay_seconds)).await;
                        }
                    }
                }
            }

            if let Some(err) = last_error {
                return Err(err);
            }
        }

        Ok(translated_text)
    }
}

impl TranslatorDeepLFree {
    pub fn new(target_lang: impl Into<String>) -> Result<Self, TranslatorError> {
        let client = Client::builder()
            .no_gzip()
            .http1_only()
            .timeout(Duration::from_secs(10))
            .build()
            .map_err(|err| TranslatorError::HttpClientBuild(err.to_string()))?;

        Ok(Self {
            source_lang: None,
            target_lang: target_lang.into(),
            endpoint: "https://www2.deepl.com/jsonrpc?client=chrome-extension,1.5.1".to_string(),
            client,
        })
    }

    pub fn with_source_lang(
        target_lang: impl Into<String>,
        source_lang: impl Into<String>,
    ) -> Result<Self, TranslatorError> {
        let mut translator = Self::new(target_lang)?;
        translator.source_lang = Some(source_lang.into());
        Ok(translator)
    }

    fn vars(&self, text: &str) -> (u64, u64) {
        let uid = rand::thread_rng().gen_range(1_000_000_000u64..=9_999_999_999u64);

        let mut count_i = text.matches('i').count() as u64;

        let mut ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        if count_i > 0 {
            count_i += 1;
            ts = ts - ts % count_i + count_i;
        }

        (uid, ts)
    }

    fn headers(&self) -> HeaderMap {
        let mut headers = HeaderMap::new();

        headers.insert(ACCEPT, HeaderValue::from_static("*/*"));
        headers.insert(ACCEPT_ENCODING, HeaderValue::from_static("gzip, deflate"));
        headers.insert(ACCEPT_LANGUAGE, HeaderValue::from_static("en-US,en;q=0.9"));
        headers.insert(AUTHORIZATION, HeaderValue::from_static("None"));

        headers.insert(
            HeaderName::from_static("authority"),
            HeaderValue::from_static("www2.deepl.com"),
        );

        headers.insert(
            CONTENT_TYPE,
            HeaderValue::from_static("application/json; charset=utf-8"),
        );

        headers.insert(
            USER_AGENT,
            HeaderValue::from_static(
                "DeepLBrowserExtension/1.5.1 Mozilla/5.0 (Macintosh; \
                 Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, \
                 like Gecko) Chrome/114.0.0.0 Safari/537.36",
            ),
        );

        headers.insert(
            ORIGIN,
            HeaderValue::from_static("chrome-extension://cofdbpoegempjloogbagkncekinflcnj"),
        );

        headers.insert(REFERER, HeaderValue::from_static("https://www.deepl.com/"));

        headers
    }

    fn source_code(&self) -> String {
        self.source_lang
            .clone()
            .unwrap_or_else(|| "auto".to_string())
    }

    fn build_body(&self, text: &str) -> String {
        let mut target_lang = self.target_lang.clone();

        let common_job_params = if target_lang.contains('-') {
            let portions: Vec<&str> = target_lang.split('-').collect();

            let variant = format!("{}-{}", portions[0].to_lowercase(), portions[1]);

            target_lang = portions[0].to_string();

            json!({
                "regionalVariant": variant
            })
        } else {
            json!({})
        };

        let (uid, ts) = self.vars(text);

        let body = json!({
            "jsonrpc": "2.0",
            "method": "LMT_handle_texts",
            "params": {
                "commonJobParams": common_job_params,
                "texts": [
                    {
                        "text": text
                    }
                ],
                "splitting": "newlines",
                "lang": {
                    "source_lang_user_selected": self.source_code(),
                    "target_lang": target_lang
                },
                "timestamp": ts
            },
            "id": uid
        });

        let body = body.to_string();

        if (uid + 3) % 13 == 0 || (uid + 5) % 29 == 0 {
            body.replace("\"method\":\"", "\"method\" : \"")
        } else {
            body.replace("\"method\":\"", "\"method\": \"")
        }
    }

    fn is_strong_chunk_boundary(ch: char) -> bool {
        matches!(
            ch,
            '\n' | '\r'
                | '.'
                | ','
                | ';'
                | ':'
                | '!'
                | '?'
                | '。'
                | '，'
                | '；'
                | '：'
                | '！'
                | '？'
        )
    }

    fn is_soft_chunk_boundary(ch: char) -> bool {
        ch.is_whitespace()
    }

    fn next_chunk_end(text: &str, start: usize, max_chars: usize) -> usize {
        let mut char_count = 0usize;
        let mut hard_end = text.len();
        let mut last_strong_boundary = None;
        let mut last_soft_boundary = None;

        for (offset, ch) in text[start..].char_indices() {
            char_count += 1;
            let end = start + offset + ch.len_utf8();

            if Self::is_strong_chunk_boundary(ch) {
                last_strong_boundary = Some(end);
            } else if Self::is_soft_chunk_boundary(ch) {
                last_soft_boundary = Some(end);
            }

            if char_count >= max_chars {
                hard_end = end;
                break;
            }
        }

        if hard_end == text.len() {
            return text.len();
        }

        last_strong_boundary
            .or(last_soft_boundary)
            .filter(|end| *end > start)
            .unwrap_or(hard_end)
    }

    fn split_translation_chunks(text: &str, max_chars: usize) -> Vec<String> {
        if text.chars().count() <= max_chars {
            return vec![text.to_string()];
        }

        let mut chunks = Vec::new();
        let mut start = 0usize;

        while start < text.len() {
            let end = Self::next_chunk_end(text, start, max_chars);
            chunks.push(text[start..end].to_string());
            start = end;
        }

        chunks
    }

    fn parse_result(response: &str) -> Result<String, TranslatorError> {
        let response_json: Value = serde_json::from_str(response)
            .map_err(|err| TranslatorError::ParseJson(err.to_string()))?;

        response_json["result"]["texts"][0]["text"]
            .as_str()
            .map(|text| text.to_string())
            .ok_or(TranslatorError::MissingTranslatedText)
    }

    pub async fn translate(&self, text: &str) -> Result<String, TranslatorError> {
        let response = self
            .client
            .post(&self.endpoint)
            .headers(self.headers())
            .body(self.build_body(text))
            .send()
            .await
            .map_err(|err| TranslatorError::Network(err.to_string()))?;

        let status = response.status();
        let is_gzip = response
            .headers()
            .get("Content-Encoding")
            .and_then(|value| value.to_str().ok())
            == Some("gzip");

        let bytes = response
            .bytes()
            .await
            .map_err(|err| TranslatorError::ReadResponse(err.to_string()))?;

        if !status.is_success() {
            let body = String::from_utf8_lossy(&bytes).to_string();

            return Err(TranslatorError::HttpStatus {
                status: status.as_u16(),
                body,
            });
        }

        let response_text = if is_gzip {
            let mut decoder = GzDecoder::new(&bytes[..]);
            let mut decoded = String::new();

            decoder
                .read_to_string(&mut decoded)
                .map_err(|err| TranslatorError::DecodeGzip(err.to_string()))?;

            decoded
        } else {
            String::from_utf8(bytes.to_vec())
                .map_err(|err| TranslatorError::DecodeUtf8(err.to_string()))?
        };

        Self::parse_result(&response_text)
    }

    pub async fn translate_chunked_cancellable_with_progress(
        &self,
        text: &str,
        cancel: Option<&AtomicBool>,
        progress: &mut dyn FnMut(usize, usize),
    ) -> Result<String, TranslatorError> {
        const MAX_CHUNK_CHARS: usize = 3_000;
        const MAX_CHUNK_ATTEMPTS: usize = 3;

        let chunks = Self::split_translation_chunks(text, MAX_CHUNK_CHARS);
        if chunks.len() == 1 {
            if cancel.is_some_and(|flag| flag.load(Ordering::Relaxed)) {
                return Err(TranslatorError::Cancelled);
            }
            let translated = self.translate(text).await?;
            progress(1, 1);
            return Ok(translated);
        }

        let mut translated_text = String::new();

        for (index, chunk) in chunks.iter().enumerate() {
            if cancel.is_some_and(|flag| flag.load(Ordering::Relaxed)) {
                return Err(TranslatorError::Cancelled);
            }
            if index > 0 {
                let delay_seconds = rand::thread_rng().gen_range(1u64..=3u64);
                tokio::time::sleep(Duration::from_secs(delay_seconds)).await;
            }

            if chunk.trim().is_empty() {
                translated_text.push_str(chunk);
                progress(index + 1, chunks.len());
                continue;
            }

            let mut last_error = None;
            for attempt in 1..=MAX_CHUNK_ATTEMPTS {
                if cancel.is_some_and(|flag| flag.load(Ordering::Relaxed)) {
                    return Err(TranslatorError::Cancelled);
                }
                match self.translate(chunk).await {
                    Ok(translated_chunk) => {
                        translated_text.push_str(&translated_chunk);
                        progress(index + 1, chunks.len());
                        last_error = None;
                        break;
                    }
                    Err(err) => {
                        let rate_limited =
                            matches!(&err, TranslatorError::HttpStatus { status: 429, .. });
                        crate::log_debug(&format!(
                            "DeepL translation: chunk {} attempt {}/{} failed: {}",
                            index + 1,
                            attempt,
                            MAX_CHUNK_ATTEMPTS,
                            err
                        ));
                        last_error = Some(err);
                        if attempt < MAX_CHUNK_ATTEMPTS {
                            let delay_seconds = if rate_limited {
                                15 * attempt as u64
                            } else {
                                rand::thread_rng().gen_range(1u64..=3u64)
                            };
                            tokio::time::sleep(Duration::from_secs(delay_seconds)).await;
                        }
                    }
                }
            }

            if let Some(err) = last_error {
                crate::log_debug(&format!(
                    "DeepL translation: chunk {} failed after retries; discarding partial output so the complete-document fallback can continue: {}",
                    index + 1,
                    err
                ));
                return Err(err);
            }
        }

        Ok(translated_text)
    }
}

pub struct TranslatorGemini {
    pub api_key: String,
    pub model: String,
    pub target_lang: String,
    pub source_lang: Option<String>,
    client: Client,
}

impl TranslatorGemini {
    pub fn new(
        api_key: String,
        model: String,
        target_lang: String,
        source_lang: Option<String>,
    ) -> Result<Self, TranslatorError> {
        let client = Client::builder()
            .timeout(Duration::from_secs(45))
            .build()
            .map_err(|err| TranslatorError::HttpClientBuild(err.to_string()))?;

        Ok(Self {
            api_key,
            model: if model.trim().is_empty() {
                crate::settings::DEFAULT_GEMINI_MODEL.to_string()
            } else {
                model.trim().to_string()
            },
            target_lang,
            source_lang,
            client,
        })
    }

    fn next_chunk_end(text: &str, start: usize, max_chars: usize) -> usize {
        let mut hard_end = text.len();
        let mut last_soft_boundary = None;

        for (char_count, (offset, ch)) in text[start..].char_indices().enumerate() {
            if char_count >= max_chars {
                hard_end = start + offset;
                break;
            }
            if matches!(ch, '\n' | '.' | '!' | '?' | ';') {
                last_soft_boundary = Some(start + offset + ch.len_utf8());
            }
        }

        if hard_end == text.len() {
            return hard_end;
        }

        last_soft_boundary
            .filter(|end| *end > start)
            .unwrap_or(hard_end)
    }

    fn split_translation_chunks(text: &str, max_chars: usize) -> Vec<String> {
        if text.chars().count() <= max_chars {
            return vec![text.to_string()];
        }

        let mut chunks = Vec::new();
        let mut start = 0usize;
        while start < text.len() {
            let end = Self::next_chunk_end(text, start, max_chars);
            chunks.push(text[start..end].to_string());
            start = end;
        }
        chunks
    }

    pub async fn translate(
        &self,
        text: &str,
        cancel: &AtomicBool,
    ) -> Result<String, TranslatorError> {
        if cancel.load(Ordering::Relaxed) {
            return Err(TranslatorError::Cancelled);
        }

        let prompt = if let Some(source_lang) = self.source_lang.as_deref() {
            format!(
                "Translate the following text from {} to {}. Return ONLY the translated text, no comments, no intro, no formatting codes.\n\n{}",
                source_lang, self.target_lang, text
            )
        } else {
            format!(
                "Translate the following text to {}. Return ONLY the translated text, no comments, no intro, no formatting codes.\n\n{}",
                self.target_lang, text
            )
        };

        self.generate_text(&prompt).await
    }

    async fn generate_text(&self, prompt: &str) -> Result<String, TranslatorError> {
        let body = json!({
            "contents": [{
                "parts": [{"text": prompt}]
            }]
        });

        let api_key = HeaderValue::from_str(self.api_key.trim())
            .map_err(|err| TranslatorError::HttpClientBuild(err.to_string()))?;
        let url = format!(
            "https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent",
            self.model
        );

        let response = self
            .client
            .post(url)
            .header(CONTENT_TYPE, "application/json")
            .header(HeaderName::from_static("x-goog-api-key"), api_key)
            .json(&body)
            .send()
            .await
            .map_err(|err| TranslatorError::Network(err.to_string()))?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let body = response.text().await.unwrap_or_default();
            return Err(TranslatorError::HttpStatus { status, body });
        }

        let resp_json: Value = response
            .json()
            .await
            .map_err(|err| TranslatorError::ParseJson(err.to_string()))?;

        let generated = resp_json["candidates"][0]["content"]["parts"][0]["text"]
            .as_str()
            .ok_or(TranslatorError::MissingTranslatedText)?
            .trim()
            .to_string();

        Ok(generated)
    }

    pub async fn summarize_same_language(
        &self,
        text: &str,
        cancel: &AtomicBool,
    ) -> Result<String, TranslatorError> {
        if cancel.load(Ordering::Relaxed) {
            return Err(TranslatorError::Cancelled);
        }

        let prompt = format!(
            "Summarize the following text while keeping the same language as the original text. Do not translate. Preserve important names, dates, numbers, and concrete facts. Return ONLY the summary, no comments, no intro, no formatting codes.\n\n{}",
            text
        );
        self.generate_text(&prompt).await
    }

    pub async fn summarize_same_language_chunked_cancellable_with_progress(
        &self,
        text: &str,
        cancel: &AtomicBool,
        progress: &mut dyn FnMut(usize, usize),
    ) -> Result<String, TranslatorError> {
        const MAX_CHUNK_CHARS: usize = 6_000;

        let chunks = Self::split_translation_chunks(text, MAX_CHUNK_CHARS);
        if chunks.len() == 1 {
            let summary = self.summarize_same_language(text, cancel).await?;
            progress(1, 1);
            return Ok(summary);
        }

        let chunk_count = chunks.len();
        crate::log_debug(&format!(
            "Gemini summary: chunking input chars={} chunks={}",
            text.chars().count(),
            chunk_count
        ));
        let mut partial_summaries = Vec::new();
        for (index, chunk) in chunks.into_iter().enumerate() {
            if cancel.load(Ordering::Relaxed) {
                return Err(TranslatorError::Cancelled);
            }
            if chunk.trim().is_empty() {
                progress(index + 1, chunk_count);
                continue;
            }
            crate::log_debug(&format!(
                "Gemini summary: summarizing chunk {}/{} chars={}",
                index + 1,
                chunk_count,
                chunk.chars().count()
            ));
            match self.summarize_same_language(&chunk, cancel).await {
                Ok(summary) => {
                    partial_summaries.push(summary);
                    progress(index + 1, chunk_count);
                }
                Err(TranslatorError::Cancelled) => return Err(TranslatorError::Cancelled),
                Err(err) if !partial_summaries.is_empty() => {
                    crate::log_debug(&format!(
                        "Gemini summary: chunk {} failed after {} completed chunks: {}",
                        index + 1,
                        partial_summaries.len(),
                        err
                    ));
                    return Err(TranslatorError::PartialSummary {
                        summary: partial_summaries.join("\n\n"),
                        error: err.to_string(),
                    });
                }
                Err(err) => return Err(err),
            }
        }

        if cancel.load(Ordering::Relaxed) {
            return Err(TranslatorError::Cancelled);
        }
        Ok(partial_summaries.join("\n\n"))
    }

    pub async fn translate_chunked_cancellable_with_progress(
        &self,
        text: &str,
        cancel: &AtomicBool,
        progress: &mut dyn FnMut(usize, usize),
    ) -> Result<String, TranslatorError> {
        const MAX_CHUNK_CHARS: usize = 6_000;

        let chunks = Self::split_translation_chunks(text, MAX_CHUNK_CHARS);
        if chunks.len() == 1 {
            let translated = self.translate(text, cancel).await?;
            progress(1, 1);
            return Ok(translated);
        }

        let chunk_count = chunks.len();
        let mut translated_text = String::new();
        for (index, chunk) in chunks.into_iter().enumerate() {
            if cancel.load(Ordering::Relaxed) {
                return Err(TranslatorError::Cancelled);
            }
            if chunk.trim().is_empty() {
                translated_text.push_str(&chunk);
                progress(index + 1, chunk_count);
                continue;
            }
            match self.translate(&chunk, cancel).await {
                Ok(translated_chunk) => {
                    translated_text.push_str(&translated_chunk);
                    progress(index + 1, chunk_count);
                }
                Err(TranslatorError::Cancelled) => return Err(TranslatorError::Cancelled),
                Err(err) => return Err(err),
            }
        }

        Ok(translated_text)
    }
}

#[cfg(test)]
mod google_translate_tests {
    use super::{TranslatorError, TranslatorGoogleFree};
    use serde_json::json;

    #[test]
    fn parses_new_google_sentence_array() {
        let response = json!([
            "Ciao mondo",
            [["Ciao ", "Hello "], ["mondo", "world"]],
            null,
            null,
            [["en"], null, [1], ["en"]],
            "en"
        ]);

        let translated = TranslatorGoogleFree::parse_result(response).unwrap();
        assert_eq!(translated, "Ciao mondo");
    }

    #[test]
    fn falls_back_to_top_level_translation() {
        let response = json!(["Ciao mondo", null, null, null, null, "en"]);

        let translated = TranslatorGoogleFree::parse_result(response).unwrap();
        assert_eq!(translated, "Ciao mondo");
    }

    #[test]
    fn rejects_response_without_translation() {
        let response = json!([null, [], null, null, null, "en"]);

        assert!(matches!(
            TranslatorGoogleFree::parse_result(response),
            Err(TranslatorError::MissingTranslatedText)
        ));
    }
}
