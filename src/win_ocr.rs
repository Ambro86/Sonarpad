use std::path::Path;
use windows::Data::Pdf::PdfDocument;
use windows::Globalization::Language as WinLanguage;
use windows::Graphics::Imaging::BitmapDecoder;
use windows::Media::Ocr::{OcrEngine, OcrResult};
use windows::Storage::StorageFile;
use windows::Storage::Streams::{IRandomAccessStream, InMemoryRandomAccessStream};
use windows::core::{HSTRING, Interface};

use crate::settings::Language;

fn language_tag(language: Language) -> HSTRING {
    match language {
        Language::Italian => HSTRING::from("it-IT"),
        Language::Ukrainian | Language::English => HSTRING::from("en-US"),
        Language::German => HSTRING::from("de-DE"),
        Language::Spanish => HSTRING::from("es-ES"),
        Language::Portuguese => HSTRING::from("pt-PT"),
        Language::PortugueseBrazilian => HSTRING::from("pt-BR"),
        Language::Swedish => HSTRING::from("sv-SE"),
        Language::Vietnamese => HSTRING::from("vi-VN"),
        Language::Czech => HSTRING::from("cs-CZ"),
        Language::Polish => HSTRING::from("pl-PL"),
        Language::French => HSTRING::from("fr-FR"),
        Language::Serbian => HSTRING::from("sr-RS"),
        Language::Lithuanian => HSTRING::from("lt-LT"),
        Language::Russian => HSTRING::from("ru-RU"),
        Language::Chinese => HSTRING::from("zh-CN"),
        Language::Hindi => HSTRING::from("hi-IN"),
        Language::Indonesian => HSTRING::from("id-ID"),
    }
}

// This is the OCR-only path: native PDF text extraction uses file_handler.rs.
// Retain hard lines in the result. A workbook's dialog turns, writing exercises
// and columns must not be collapsed into one paragraph by OCR formatting.
#[derive(Debug, Clone)]
struct ScannedLine {
    text: String,
    x: f32,
    y: f32,
    right: f32,
    bottom: f32,
}

fn contains_japanese(text: &str) -> bool {
    text.chars().any(|ch| {
        matches!(ch,
            '\u{3040}'..='\u{30ff}' | '\u{31f0}'..='\u{31ff}'
            | '\u{3400}'..='\u{4dbf}' | '\u{4e00}'..='\u{9fff}'
            | '\u{ff66}'..='\u{ff9f}'
        )
    })
}

fn lines_from_ocr(result: &OcrResult) -> windows::core::Result<Vec<ScannedLine>> {
    let mut lines_out = Vec::new();
    let lines = result.Lines()?;
    for i in 0..lines.Size()? {
        let line = lines.GetAt(i)?;
        let text = line.Text()?.to_string();
        if text.trim().is_empty() {
            continue;
        }
        let words = line.Words()?;
        let mut bounds: Option<(f32, f32, f32, f32)> = None;
        for j in 0..words.Size()? {
            let rect = words.GetAt(j)?.BoundingRect()?;
            bounds = Some(match bounds {
                Some((x, y, right, bottom)) => (
                    x.min(rect.X),
                    y.min(rect.Y),
                    right.max(rect.X + rect.Width),
                    bottom.max(rect.Y + rect.Height),
                ),
                None => (rect.X, rect.Y, rect.X + rect.Width, rect.Y + rect.Height),
            });
        }
        let (x, y, right, bottom) = bounds.unwrap_or((0.0, i as f32, 0.0, i as f32 + 1.0));
        lines_out.push(ScannedLine {
            text: text.trim().to_owned(),
            x,
            y,
            right,
            bottom,
        });
    }
    Ok(lines_out)
}

// OCRs in different languages can report the same image text twice. Give
// precedence to Japanese OCR where it actually recognizes kana/kanji; keep
// separate English lines and the approximate original page reading order.
fn merge_japanese_lines(
    mut primary: Vec<ScannedLine>,
    japanese: Vec<ScannedLine>,
) -> Vec<ScannedLine> {
    for ja in japanese {
        if !contains_japanese(&ja.text) {
            continue;
        }
        let overlap = primary.iter().position(|existing| {
            let x_intersection = (existing.right.min(ja.right) - existing.x.max(ja.x)).max(0.0);
            let y_intersection = (existing.bottom.min(ja.bottom) - existing.y.max(ja.y)).max(0.0);
            let narrow_width = (existing.right - existing.x).min(ja.right - ja.x);
            let short_height = (existing.bottom - existing.y).min(ja.bottom - ja.y);
            narrow_width > 0.0
                && short_height > 0.0
                && x_intersection / narrow_width >= 0.55
                && y_intersection / short_height >= 0.50
        });
        match overlap {
            Some(index) if !contains_japanese(&primary[index].text) => {
                primary[index] = ja;
            }
            Some(index) if ja.text.chars().count() > primary[index].text.chars().count() => {
                primary[index] = ja;
            }
            Some(_) => {} // Identical physical text: retain only one copy.
            None => primary.push(ja),
        }
    }
    primary
}

fn append_scanned_lines(result: &mut String, lines: &[ScannedLine]) {
    let mut prev_bottom: Option<f32> = None;
    let mut prev_height = 0.0_f32;
    for line in lines {
        if !result.is_empty() {
            result.push_str("\r\n");
            if let Some(bottom) = prev_bottom
                && prev_height > 0.0
                && line.y - bottom > prev_height * 1.3
            {
                result.push_str("\r\n");
            }
        }
        result.push_str(&line.text);
        prev_height = (line.bottom - line.y).max(1.0);
        prev_bottom = Some(line.bottom);
    }
}

fn render_ocr_lines(mut lines: Vec<ScannedLine>, page_width: f32) -> String {
    // The OCR's line boxes preserve dialog turns. When two *independent* text
    // columns have a clear gutter, read each column top-to-bottom instead of
    // alternating the left and right lines. Ambiguous mixed layouts retain
    // the normal line order: guessing columns in tables would be destructive.
    lines.sort_by(|a, b| a.y.total_cmp(&b.y).then_with(|| a.x.total_cmp(&b.x)));

    let center = page_width * 0.5;
    let gutter = page_width * 0.025;
    let mut left = Vec::new();
    let mut right = Vec::new();
    let mut crossing = Vec::new();
    for line in &lines {
        if line.right < center - gutter {
            left.push(line.clone());
        } else if line.x > center + gutter {
            right.push(line.clone());
        } else {
            crossing.push(line.clone());
        }
    }

    if let (Some(first_left), Some(first_right)) = (left.first(), right.first())
        && left.len() >= 8
        && right.len() >= 8
        && crossing.len() <= 5
    {
        let first_column_y = first_left.y.min(first_right.y);
        let last_column_bottom = left
            .iter()
            .chain(right.iter())
            .map(|line| line.bottom)
            .fold(0.0_f32, f32::max);
        let internal_crossing = crossing
            .iter()
            .any(|line| line.y > first_column_y && line.y < last_column_bottom);
        if !internal_crossing {
            let mut text = String::new();
            let mut header = Vec::new();
            let mut footer = Vec::new();
            for line in crossing {
                if line.y <= first_column_y {
                    header.push(line);
                } else {
                    footer.push(line);
                }
            }
            // Keep the pre-existing boundary and page heading behavior.
            for group in [&header, &left, &right, &footer] {
                if !group.is_empty() {
                    if !text.is_empty() {
                        text.push_str("\r\n\r\n");
                    }
                    append_scanned_lines(&mut text, group);
                }
            }
            return text;
        }
    }

    let mut text = String::new();
    append_scanned_lines(&mut text, &lines);
    text
}

fn create_supported_engine(tag: &str) -> windows::core::Result<Option<OcrEngine>> {
    let language = WinLanguage::CreateLanguage(&HSTRING::from(tag))?;
    if OcrEngine::IsLanguageSupported(&language)? {
        Ok(Some(OcrEngine::TryCreateFromLanguage(&language)?))
    } else {
        Ok(None)
    }
}

/// Returns whether the Windows Japanese OCR recognizer is usable. Called only
/// when a PDF has no extractable text; ordinary PDF opening is unaffected.
pub(crate) fn japanese_ocr_is_available() -> bool {
    match create_supported_engine("ja-JP") {
        Ok(Some(_)) => true,
        Ok(None) => false,
        Err(err) => {
            crate::log_debug(&format!(
                "PDF OCR: unable to check ja-JP language support: {err}"
            ));
            // Do not offer installation when Windows OCR itself cannot be queried.
            true
        }
    }
}

fn ocr_file_path(
    path: &Path,
    canonicalize: impl FnOnce(&Path) -> std::io::Result<std::path::PathBuf>,
) -> Result<String, String> {
    // Some mapped/virtual drives can read files but cannot resolve their final
    // handle path (ERROR_INVALID_FUNCTION). WinRT only needs an absolute path.
    let abs_path = canonicalize(path)
        .or_else(|err| {
            crate::log_debug(&format!(
                "PDF OCR: canonicalize failed for {}: {err}; trying absolute path",
                path.display()
            ));
            std::path::absolute(path)
        })
        .map_err(|err| format!("PDF OCR: absolute path failed: {err}"))?;
    let mut path_str = abs_path.to_string_lossy().to_string();

    // WinRT APIs can be picky about the \\?\ prefix added by canonicalize.
    if path_str.starts_with(r"\\?\UNC\") {
        path_str = format!("\\\\{}", &path_str[8..]);
    } else if path_str.starts_with(r"\\?\") {
        path_str = path_str[4..].to_string();
    }
    Ok(path_str)
}

pub fn recognize_text_from_pdf(path: &Path, language: Language) -> Result<String, String> {
    // This function must be synchronous/blocking as per caller expectation.
    let path_str = ocr_file_path(path, |path| path.canonicalize())?;
    let future = async {
        let file = StorageFile::GetFileFromPathAsync(&HSTRING::from(&path_str))?.await?;
        let pdf_doc = PdfDocument::LoadFromFileAsync(&file)?.await?;

        let tag = language_tag(language);
        let win_lang = WinLanguage::CreateLanguage(&tag)?;
        let primary_engine = if OcrEngine::IsLanguageSupported(&win_lang)? {
            OcrEngine::TryCreateFromLanguage(&win_lang)?
        } else {
            OcrEngine::TryCreateFromUserProfileLanguages()?
        };
        // Optional second OCR for mixed English/Japanese textbooks. No cloud,
        // no external executable, no changed behavior for text-based PDFs.
        // On Windows the Japanese OCR language capability must be installed.
        let japanese_engine = create_supported_engine("ja-JP")?;
        let english_engine = create_supported_engine("en-US")?;
        if japanese_engine.is_none() {
            crate::log_debug(
                "PDF OCR: ja-JP recognition is not installed; Japanese text in scanned pages may be missing. Install the Japanese OCR language feature in Windows.",
            );
        }

        let page_count = pdf_doc.PageCount()?;
        let mut full_text = String::new();
        let mut japanese_seen = false;
        for i in 0..page_count {
            if i % 25 == 0 {
                crate::log_debug(&format!(
                    "PDF OCR: processing page {} of {} (Japanese recognizer installed: {})",
                    i + 1,
                    page_count,
                    japanese_engine.is_some()
                ));
            }
            let page = pdf_doc.GetPage(i)?;
            let stream = InMemoryRandomAccessStream::new()?;
            let i_stream: IRandomAccessStream = stream.cast()?;
            page.RenderToStreamAsync(&i_stream)?.await?;
            let decoder = BitmapDecoder::CreateAsync(&i_stream)?.await?;
            let bitmap = decoder.GetSoftwareBitmapAsync()?.await?;

            let native_result = primary_engine.RecognizeAsync(&bitmap)?.await?;
            // Keep the original OCR engine if there is no installed ja-JP
            // capability or this isn't a bilingual Japanese document.
            let japanese_result = if let Some(ref engine) = japanese_engine {
                match engine.RecognizeAsync(&bitmap) {
                    Ok(operation) => match operation.await {
                        Ok(result) => Some(result),
                        Err(err) => {
                            crate::log_debug(&format!(
                                "PDF OCR: Japanese pass failed on page {}: {}",
                                i + 1,
                                err
                            ));
                            None
                        }
                    },
                    Err(err) => {
                        crate::log_debug(&format!(
                            "PDF OCR: Japanese pass could not start on page {}: {}",
                            i + 1,
                            err
                        ));
                        None
                    }
                }
            } else {
                None
            };

            if let Some(ref result) = japanese_result
                && contains_japanese(&result.Text()?.to_string())
            {
                japanese_seen = true;
            }
            let mut page_lines = if japanese_seen {
                if let Some(ref engine) = english_engine {
                    // Prefer English for the Latin part of English/Japanese
                    // books; retain the UI-language recognizer otherwise.
                    if tag != "en-US" {
                        match engine.RecognizeAsync(&bitmap) {
                            Ok(operation) => match operation.await {
                                Ok(english_result) => lines_from_ocr(&english_result)?,
                                Err(_) => lines_from_ocr(&native_result)?,
                            },
                            Err(_) => lines_from_ocr(&native_result)?,
                        }
                    } else {
                        lines_from_ocr(&native_result)?
                    }
                } else {
                    lines_from_ocr(&native_result)?
                }
            } else {
                lines_from_ocr(&native_result)?
            };

            if let Some(result) = japanese_result {
                page_lines = merge_japanese_lines(page_lines, lines_from_ocr(&result)?);
            }
            let text = render_ocr_lines(page_lines, bitmap.PixelWidth()? as f32);

            if !full_text.is_empty() {
                full_text.push_str("\r\n\r\n");
            }
            full_text.push_str(&format!("--- Pagina {} ---\r\n", i + 1));
            full_text.push_str(&text);
        }
        Ok(full_text)
    };

    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| e.to_string())?;

    rt.block_on(future)
        .map_err(|e: windows::core::Error| e.message().to_string())
}

#[cfg(test)]
mod tests {
    use super::{ScannedLine, contains_japanese, merge_japanese_lines, render_ocr_lines};

    #[test]
    fn ocr_mapped_drive_invalid_function_uses_absolute_path() {
        let path = std::path::Path::new(r"R:\CONV. J. ORD. HAYA 20-22 27.10.26.pdf");
        let resolved =
            super::ocr_file_path(path, |_| Err(std::io::Error::from_raw_os_error(1))).unwrap();
        assert_eq!(resolved, path.to_string_lossy());
    }

    #[test]
    fn ocr_canonical_paths_keep_existing_winrt_prefix_handling() {
        for (canonical, expected) in [
            (r"\\?\C:\Documents\scan.pdf", r"C:\Documents\scan.pdf"),
            (r"\\?\UNC\server\share\scan.pdf", r"\\server\share\scan.pdf"),
        ] {
            let resolved = super::ocr_file_path(std::path::Path::new("scan.pdf"), |_| {
                Ok(std::path::PathBuf::from(canonical))
            })
            .unwrap();
            assert_eq!(resolved, expected);
        }
    }

    fn line(text: &str, x: f32, y: f32) -> ScannedLine {
        ScannedLine {
            text: text.into(),
            x,
            y,
            right: x + 80.0,
            bottom: y + 12.0,
        }
    }

    #[test]
    fn japanese_script_detected_without_confusing_english() {
        assert!(contains_japanese("こんにちは、カタカナと漢字"));
        assert!(!contains_japanese("Hello from Japan"));
    }

    #[test]
    fn japanese_replaces_unreadable_same_position_and_keeps_english() {
        let original = vec![
            line("AI 93*#", 10.0, 20.0),
            line("How are you?", 10.0, 60.0),
        ];
        let japanese = vec![line("アイです。", 11.0, 21.0)];
        let merged = merge_japanese_lines(original, japanese);
        let text = render_ocr_lines(merged, 300.0);
        assert!(text.contains("アイです。"));
        assert!(text.contains("How are you?"));
        assert!(!text.contains("93*#"));
    }

    #[test]
    fn two_independent_columns_are_read_sequentially() {
        let mut lines = vec![line("Page title", 280.0, 0.0)];
        for i in 0..9 {
            lines.push(line(&format!("Left {i}"), 10.0, 20.0 + i as f32 * 18.0));
            lines.push(line(&format!("Right {i}"), 320.0, 20.0 + i as f32 * 18.0));
        }
        let text = render_ocr_lines(lines, 600.0);
        assert!(text.find("Left 8").unwrap() < text.find("Right 0").unwrap());
        assert!(text.find("Page title").unwrap() < text.find("Left 0").unwrap());
    }

    #[test]
    fn separate_dialog_lines_remain_separate() {
        let text = render_ocr_lines(
            vec![
                line("Jean: I'm fine", 4.0, 35.0),
                line("Ai: Hello", 4.0, 10.0),
            ],
            300.0,
        );
        assert_eq!(text, "Ai: Hello\r\nJean: I'm fine");
    }
}
