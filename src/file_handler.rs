use crate::i18n;
use crate::settings::{Language, TextEncoding, error_open_file_message};
use calamine::{Data as CalamineData, Reader, open_workbook_auto};
use cfb::CompoundFile;
use docx_rs::{
    DocumentChild, Docx, Paragraph, ParagraphChild, Run, RunChild, Table, TableCellContent,
    read_docx,
};
use encoding_rs::{Encoding, WINDOWS_1250, WINDOWS_1252};
use lopdf::{Dictionary as LoDictionary, Document as LoDocument, Object as LoObject, StringFormat};
use pdf_extract::extract_text;
use pdfium_render::prelude::*;
use printpdf::{
    Color, Mm, Op, ParsedFont, PdfDocument, PdfPage, PdfSaveOptions, Point, Pt, Rgb, TextItem,
};
use quick_xml::events::Event;
use quick_xml::reader::Reader as XmlReader;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use windows::Win32::Globalization::{
    CP_ACP, MULTI_BYTE_TO_WIDE_CHAR_FLAGS, MultiByteToWideChar, WideCharToMultiByte,
};
use zip::ZipArchive;

// --- Path identification ---

pub fn is_docx_path(path: &Path) -> bool {
    path.extension()
        .and_then(|s| s.to_str())
        .map(|s| s.eq_ignore_ascii_case("docx"))
        .unwrap_or(false)
}

pub fn is_odt_path(path: &Path) -> bool {
    path.extension()
        .and_then(|s| s.to_str())
        .map(|s| s.eq_ignore_ascii_case("odt"))
        .unwrap_or(false)
}

pub fn is_doc_path(path: &Path) -> bool {
    path.extension()
        .and_then(|s| s.to_str())
        .map(|s| s.eq_ignore_ascii_case("doc"))
        .unwrap_or(false)
}

pub fn is_spreadsheet_path(path: &Path) -> bool {
    path.extension()
        .and_then(|s| s.to_str())
        .map(|s| {
            s.eq_ignore_ascii_case("xls")
                || s.eq_ignore_ascii_case("xlsx")
                || s.eq_ignore_ascii_case("ods")
        })
        .unwrap_or(false)
}

pub fn is_pptx_path(path: &Path) -> bool {
    path.extension()
        .and_then(|s| s.to_str())
        .map(|s| s.eq_ignore_ascii_case("pptx"))
        .unwrap_or(false)
}

pub fn is_ppt_path(path: &Path) -> bool {
    path.extension()
        .and_then(|s| s.to_str())
        .map(|s| s.eq_ignore_ascii_case("ppt"))
        .unwrap_or(false)
}

pub fn is_odp_path(path: &Path) -> bool {
    path.extension()
        .and_then(|s| s.to_str())
        .map(|s| s.eq_ignore_ascii_case("odp"))
        .unwrap_or(false)
}

pub fn is_pdf_path(path: &Path) -> bool {
    path.extension()
        .and_then(|s| s.to_str())
        .map(|s| s.eq_ignore_ascii_case("pdf"))
        .unwrap_or(false)
}

pub fn is_epub_path(path: &Path) -> bool {
    path.extension()
        .and_then(|s| s.to_str())
        .map(|s| s.eq_ignore_ascii_case("epub"))
        .unwrap_or(false)
}

pub fn is_gdoc_path(path: &Path) -> bool {
    path.extension()
        .and_then(|s| s.to_str())
        .map(|s| {
            s.eq_ignore_ascii_case("gdoc")
                || s.eq_ignore_ascii_case("gsheet")
                || s.eq_ignore_ascii_case("gslides")
        })
        .unwrap_or(false)
}

pub fn is_html_path(path: &Path) -> bool {
    path.extension()
        .and_then(|s| s.to_str())
        .map(|s| s.eq_ignore_ascii_case("html") || s.eq_ignore_ascii_case("htm"))
        .unwrap_or(false)
}

pub fn is_audio_path(path: &Path) -> bool {
    path.extension()
        .and_then(|s| s.to_str())
        .map(|s| {
            s.eq_ignore_ascii_case("mp3")
                || s.eq_ignore_ascii_case("m4a")
                || s.eq_ignore_ascii_case("mp4")
                || s.eq_ignore_ascii_case("aac")
                || s.eq_ignore_ascii_case("mkv")
                || s.eq_ignore_ascii_case("avi")
                || s.eq_ignore_ascii_case("mov")
                || s.eq_ignore_ascii_case("m4v")
                || s.eq_ignore_ascii_case("webm")
                || s.eq_ignore_ascii_case("mpg")
                || s.eq_ignore_ascii_case("mpeg")
                || s.eq_ignore_ascii_case("ts")
                || s.eq_ignore_ascii_case("m2ts")
                || s.eq_ignore_ascii_case("mts")
                || s.eq_ignore_ascii_case("wmv")
                || s.eq_ignore_ascii_case("asf")
                || s.eq_ignore_ascii_case("flv")
                || s.eq_ignore_ascii_case("vob")
                || s.eq_ignore_ascii_case("3gp")
                || s.eq_ignore_ascii_case("flac")
                || s.eq_ignore_ascii_case("ogg")
                || s.eq_ignore_ascii_case("opus")
                || s.eq_ignore_ascii_case("wma")
                || s.eq_ignore_ascii_case("aiff")
                || s.eq_ignore_ascii_case("m4b")
                || s.eq_ignore_ascii_case("ogg")
                || s.eq_ignore_ascii_case("opus")
                || s.eq_ignore_ascii_case("wav")
                || s.eq_ignore_ascii_case("flac")
        })
        .unwrap_or(false)
}

pub fn is_video_path(path: &Path) -> bool {
    path.extension()
        .and_then(|s| s.to_str())
        .map(|s| {
            s.eq_ignore_ascii_case("mp4")
                || s.eq_ignore_ascii_case("mkv")
                || s.eq_ignore_ascii_case("avi")
                || s.eq_ignore_ascii_case("mov")
                || s.eq_ignore_ascii_case("m4v")
                || s.eq_ignore_ascii_case("webm")
                || s.eq_ignore_ascii_case("mpg")
                || s.eq_ignore_ascii_case("mpeg")
                || s.eq_ignore_ascii_case("ts")
                || s.eq_ignore_ascii_case("m2ts")
                || s.eq_ignore_ascii_case("mts")
                || s.eq_ignore_ascii_case("wmv")
                || s.eq_ignore_ascii_case("asf")
                || s.eq_ignore_ascii_case("flv")
                || s.eq_ignore_ascii_case("vob")
                || s.eq_ignore_ascii_case("3gp")
        })
        .unwrap_or(false)
}

pub(crate) fn media_files_in_folder(current: &Path) -> std::io::Result<Vec<PathBuf>> {
    let folder = current
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let mut paths = Vec::new();
    for entry in std::fs::read_dir(folder)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_file() && is_audio_path(&path) {
            paths.push(path);
        }
    }
    paths.sort_by(|a, b| {
        let a_name =
            crate::accessibility::to_wide(&a.file_name().unwrap_or_default().to_string_lossy());
        let b_name =
            crate::accessibility::to_wide(&b.file_name().unwrap_or_default().to_string_lossy());
        let order = unsafe {
            windows::Win32::UI::Shell::StrCmpLogicalW(
                windows::core::PCWSTR(a_name.as_ptr()),
                windows::core::PCWSTR(b_name.as_ptr()),
            )
        };
        order.cmp(&0).then_with(|| a.cmp(b))
    });
    Ok(paths)
}

// --- Text Encoding / Decoding ---

fn decode_ansi_with_acp(bytes: &[u8]) -> Option<String> {
    if bytes.is_empty() {
        return Some(String::new());
    }

    unsafe {
        let len = MultiByteToWideChar(CP_ACP, MULTI_BYTE_TO_WIDE_CHAR_FLAGS(0), bytes, None);
        if len <= 0 {
            return None;
        }

        let mut wide = vec![0u16; len as usize];
        let len2 = MultiByteToWideChar(
            CP_ACP,
            MULTI_BYTE_TO_WIDE_CHAR_FLAGS(0),
            bytes,
            Some(&mut wide),
        );
        if len2 <= 0 {
            return None;
        }

        wide.truncate(len2 as usize);
        Some(String::from_utf16_lossy(&wide))
    }
}

fn central_european_char_score(text: &str) -> usize {
    text.chars()
        .filter(|ch| {
            matches!(
                ch,
                'ě' | 'š'
                    | 'č'
                    | 'ř'
                    | 'ž'
                    | 'ý'
                    | 'á'
                    | 'í'
                    | 'é'
                    | 'ů'
                    | 'ú'
                    | 'ň'
                    | 'ď'
                    | 'ť'
                    | 'Ě'
                    | 'Š'
                    | 'Č'
                    | 'Ř'
                    | 'Ž'
                    | 'Ý'
                    | 'Á'
                    | 'Í'
                    | 'É'
                    | 'Ů'
                    | 'Ú'
                    | 'Ň'
                    | 'Ď'
                    | 'Ť'
                    | 'ą'
                    | 'ć'
                    | 'ę'
                    | 'ł'
                    | 'ń'
                    | 'ó'
                    | 'ś'
                    | 'ź'
                    | 'ż'
                    | 'Ą'
                    | 'Ć'
                    | 'Ę'
                    | 'Ł'
                    | 'Ń'
                    | 'Ó'
                    | 'Ś'
                    | 'Ź'
                    | 'Ż'
            )
        })
        .count()
}

fn western_european_char_score(text: &str) -> usize {
    text.chars()
        .filter(|ch| {
            matches!(
                ch,
                'à' | 'è'
                    | 'ì'
                    | 'ò'
                    | 'ù'
                    | 'À'
                    | 'È'
                    | 'Ì'
                    | 'Ò'
                    | 'Ù'
                    | 'á'
                    | 'é'
                    | 'í'
                    | 'ó'
                    | 'ú'
                    | 'Á'
                    | 'É'
                    | 'Í'
                    | 'Ó'
                    | 'Ú'
                    | 'â'
                    | 'ê'
                    | 'î'
                    | 'ô'
                    | 'û'
                    | 'Â'
                    | 'Ê'
                    | 'Î'
                    | 'Ô'
                    | 'Û'
                    | 'ã'
                    | 'õ'
                    | 'Ã'
                    | 'Õ'
                    | 'ç'
                    | 'Ç'
                    | 'ñ'
                    | 'Ñ'
            )
        })
        .count()
}

fn cjk_char_score(text: &str) -> usize {
    text.chars()
        .filter(|&ch| {
            let code = ch as u32;
            (0x4E00..=0x9FFF).contains(&code)
                || (0x3400..=0x4DBF).contains(&code)
                || (0xF900..=0xFAFF).contains(&code)
        })
        .count()
}

fn japanese_char_score(text: &str) -> usize {
    text.chars()
        .filter(|&ch| {
            let code = ch as u32;
            (0x3040..=0x309F).contains(&code) // Hiragana
                || (0x30A0..=0x30FF).contains(&code) // Katakana
                || (0xFF66..=0xFF9F).contains(&code) // Half-width Katakana
                || (0x4E00..=0x9FFF).contains(&code) // CJK Unified Ideographs
                || (0x3400..=0x4DBF).contains(&code) // CJK Ext A
        })
        .count()
}

fn mojibake_latin1_score(text: &str) -> usize {
    text.chars()
        .filter(|ch| {
            matches!(
                ch,
                'Â' | 'Ã'
                    | 'Ä'
                    | 'Å'
                    | 'Æ'
                    | 'Ç'
                    | 'Ð'
                    | 'Ñ'
                    | 'Ò'
                    | 'Ó'
                    | 'Ô'
                    | 'Õ'
                    | 'Ö'
                    | '×'
                    | 'Ø'
                    | 'Ù'
                    | 'Ú'
                    | 'Û'
                    | 'Ü'
                    | 'Ý'
                    | 'Þ'
                    | 'ß'
                    | 'à'
                    | 'á'
                    | 'â'
                    | 'ã'
                    | 'ä'
                    | 'å'
                    | 'æ'
                    | 'ç'
                    | 'è'
                    | 'é'
                    | 'ê'
                    | 'ë'
                    | 'ì'
                    | 'í'
                    | 'î'
                    | 'ï'
                    | 'ð'
                    | 'ñ'
                    | 'ò'
                    | 'ó'
                    | 'ô'
                    | 'õ'
                    | 'ö'
                    | 'ø'
                    | 'ù'
                    | 'ú'
                    | 'û'
                    | 'ü'
                    | 'ý'
                    | 'þ'
                    | 'ÿ'
            )
        })
        .count()
}

fn mojibake_cp1252_symbol_score(text: &str) -> usize {
    text.chars()
        .filter(|ch| {
            matches!(
                ch,
                '‚' | 'ƒ'
                    | '„'
                    | '…'
                    | '†'
                    | '‡'
                    | 'ˆ'
                    | '‰'
                    | 'Š'
                    | '‹'
                    | 'Œ'
                    | 'Ž'
                    | '‘'
                    | '’'
                    | '“'
                    | '”'
                    | '•'
                    | '–'
                    | '—'
                    | '˜'
                    | '™'
                    | 'š'
                    | '›'
                    | 'œ'
                    | 'ž'
                    | 'Ÿ'
            )
        })
        .count()
}

fn should_prefer_gb18030(current_text: &str, gb_text: &str) -> bool {
    let gb_cjk = cjk_char_score(gb_text);
    if gb_cjk < 4 {
        return false;
    }

    let current_cjk = cjk_char_score(current_text);
    if current_cjk >= gb_cjk {
        return false;
    }

    let replacement_count = gb_text.chars().filter(|&c| c == '\u{FFFD}').count();
    if replacement_count > 2 {
        return false;
    }

    let mojibake_score = mojibake_latin1_score(current_text);
    let letter_count = current_text
        .chars()
        .filter(|ch| ch.is_alphabetic())
        .count()
        .max(1);

    mojibake_score >= 8 && mojibake_score * 3 >= letter_count
}

fn should_prefer_shift_jis(current_text: &str, shift_jis_text: &str) -> bool {
    let sjis_jp = japanese_char_score(shift_jis_text);
    if sjis_jp < 6 {
        return false;
    }

    let current_jp = japanese_char_score(current_text);
    if current_jp >= sjis_jp {
        return false;
    }

    let replacement_count = shift_jis_text.chars().filter(|&c| c == '\u{FFFD}').count();
    if replacement_count > 2 {
        return false;
    }

    let cp1252_mojibake = mojibake_cp1252_symbol_score(current_text);
    cp1252_mojibake >= 6
}

fn prefer_cp1250_for_language(language: Language) -> bool {
    matches!(language, Language::Czech | Language::Polish)
}

fn choose_ansi_decoding(
    language: Language,
    cp1250_text: &str,
    cp1252_text: &str,
    acp_text: Option<&str>,
) -> String {
    let cp1250_score = central_european_char_score(cp1250_text);

    if let Some(acp_text) = acp_text {
        let acp_ce_score = central_european_char_score(acp_text);
        if prefer_cp1250_for_language(language) {
            if cp1250_score >= 2 && cp1250_score > acp_ce_score {
                return cp1250_text.to_string();
            }
            return acp_text.to_string();
        }

        // For non-central-European UI languages, prefer 1252 when it clearly carries
        // western diacritics better than ACP (e.g. Italian cp1252 opened on cp1250 systems).
        let acp_west_score = western_european_char_score(acp_text);
        let cp1252_west_score = western_european_char_score(cp1252_text);
        if cp1252_west_score > acp_west_score {
            return cp1252_text.to_string();
        }
        return acp_text.to_string();
    }

    if prefer_cp1250_for_language(language) && cp1250_score >= 2 {
        return cp1250_text.to_string();
    }

    cp1252_text.to_string()
}

pub(crate) fn decode_ansi_best_effort(bytes: &[u8], language: Language) -> String {
    let (cp1250_text, _, _) = WINDOWS_1250.decode(bytes);
    let cp1250_text = cp1250_text.into_owned();
    let (cp1252_text, _, _) = WINDOWS_1252.decode(bytes);
    let cp1252_text = cp1252_text.into_owned();
    let acp_text = decode_ansi_with_acp(bytes);
    let gb18030_text = Encoding::for_label(b"gb18030").map(|enc| {
        let (text, _, _) = enc.decode(bytes);
        text.into_owned()
    });
    let shift_jis_text = Encoding::for_label(b"shift_jis").map(|enc| {
        let (text, _, _) = enc.decode(bytes);
        text.into_owned()
    });

    let chosen = choose_ansi_decoding(language, &cp1250_text, &cp1252_text, acp_text.as_deref());
    if let Some(gb_text) = gb18030_text
        && should_prefer_gb18030(&chosen, &gb_text)
    {
        return gb_text;
    }
    if let Some(sjis_text) = shift_jis_text
        && should_prefer_shift_jis(&chosen, &sjis_text)
    {
        return sjis_text;
    }
    chosen
}

pub fn decode_text_with_encoding(
    bytes: &[u8],
    encoding: TextEncoding,
    language: Language,
) -> Result<String, String> {
    match encoding {
        TextEncoding::Utf8 => {
            String::from_utf8(bytes.to_vec()).map_err(|_| error_invalid_encoding_message(language))
        }
        TextEncoding::Utf8Bom => {
            let start =
                if bytes.len() >= 3 && bytes[0] == 0xEF && bytes[1] == 0xBB && bytes[2] == 0xBF {
                    3
                } else {
                    0
                };
            String::from_utf8(bytes[start..].to_vec())
                .map_err(|_| error_invalid_encoding_message(language))
        }
        TextEncoding::Utf16Le => {
            let start = if bytes.len() >= 2 && bytes[0] == 0xFF && bytes[1] == 0xFE {
                2
            } else {
                0
            };
            if !(bytes.len() - start).is_multiple_of(2) {
                return Err(error_invalid_utf16le_message(language));
            }
            let mut utf16 = Vec::with_capacity((bytes.len() - start) / 2);
            let mut i = start;
            while i + 1 < bytes.len() {
                utf16.push(u16::from_le_bytes([bytes[i], bytes[i + 1]]));
                i += 2;
            }
            Ok(String::from_utf16_lossy(&utf16))
        }
        TextEncoding::Utf16Be => {
            let start = if bytes.len() >= 2 && bytes[0] == 0xFE && bytes[1] == 0xFF {
                2
            } else {
                0
            };
            if !(bytes.len() - start).is_multiple_of(2) {
                return Err(error_invalid_utf16be_message(language));
            }
            let mut utf16 = Vec::with_capacity((bytes.len() - start) / 2);
            let mut i = start;
            while i + 1 < bytes.len() {
                utf16.push(u16::from_be_bytes([bytes[i], bytes[i + 1]]));
                i += 2;
            }
            Ok(String::from_utf16_lossy(&utf16))
        }
        TextEncoding::Ansi => Ok(decode_ansi_best_effort(bytes, language)),
    }
}

pub fn decode_text(bytes: &[u8], language: Language) -> Result<(String, TextEncoding), String> {
    if bytes.len() >= 3
        && bytes[0] == 0xEF
        && bytes[1] == 0xBB
        && bytes[2] == 0xBF
        && let Ok(text) = String::from_utf8(bytes[3..].to_vec())
    {
        return Ok((text, TextEncoding::Utf8Bom));
    }

    if bytes.len() >= 2 {
        if bytes[0] == 0xFF && bytes[1] == 0xFE {
            if !(bytes.len() - 2).is_multiple_of(2) {
                return Err(error_invalid_utf16le_message(language));
            }
            let mut utf16 = Vec::with_capacity((bytes.len() - 2) / 2);
            let mut i = 2;
            while i + 1 < bytes.len() {
                utf16.push(u16::from_le_bytes([bytes[i], bytes[i + 1]]));
                i += 2;
            }
            return Ok((String::from_utf16_lossy(&utf16), TextEncoding::Utf16Le));
        }
        if bytes[0] == 0xFE && bytes[1] == 0xFF {
            if !(bytes.len() - 2).is_multiple_of(2) {
                return Err(error_invalid_utf16be_message(language));
            }
            let mut utf16 = Vec::with_capacity((bytes.len() - 2) / 2);
            let mut i = 2;
            while i + 1 < bytes.len() {
                utf16.push(u16::from_be_bytes([bytes[i], bytes[i + 1]]));
                i += 2;
            }
            return Ok((String::from_utf16_lossy(&utf16), TextEncoding::Utf16Be));
        }
    }

    if let Ok(text) = String::from_utf8(bytes.to_vec()) {
        return Ok((text, TextEncoding::Utf8));
    }

    Ok((decode_ansi_best_effort(bytes, language), TextEncoding::Ansi))
}

pub fn encode_text(text: &str, encoding: TextEncoding) -> Vec<u8> {
    match encoding {
        TextEncoding::Utf8 => text.as_bytes().to_vec(),
        TextEncoding::Utf8Bom => {
            let mut out = Vec::with_capacity(3 + text.len());
            out.extend_from_slice(&[0xEF, 0xBB, 0xBF]);
            out.extend_from_slice(text.as_bytes());
            out
        }
        TextEncoding::Utf16Le => {
            let mut out = Vec::with_capacity(2 + text.len() * 2);
            out.extend_from_slice(&[0xFF, 0xFE]);
            for unit in text.encode_utf16() {
                out.extend_from_slice(&unit.to_le_bytes());
            }
            out
        }
        TextEncoding::Utf16Be => {
            let mut out = Vec::with_capacity(2 + text.len() * 2);
            out.extend_from_slice(&[0xFE, 0xFF]);
            for unit in text.encode_utf16() {
                out.extend_from_slice(&unit.to_be_bytes());
            }
            out
        }
        TextEncoding::Ansi => {
            let wide: Vec<u16> = text.encode_utf16().collect();
            if wide.is_empty() {
                return Vec::new();
            }
            unsafe {
                let len = WideCharToMultiByte(CP_ACP, 0, &wide, None, None, None);
                if len > 0 {
                    let mut buf = vec![0u8; len as usize];
                    let len2 = WideCharToMultiByte(CP_ACP, 0, &wide, Some(&mut buf), None, None);
                    if len2 > 0 {
                        buf.truncate(len2 as usize);
                        return buf;
                    }
                }
            }
            let (encoded, _, _) = WINDOWS_1252.encode(text);
            encoded.into_owned()
        }
    }
}

pub fn read_ppt_text(path: &Path, language: Language) -> Result<String, String> {
    if is_pptx_path(path) {
        return read_pptx_text(path, language);
    }
    if is_ppt_path(path) {
        if is_zip_container(path) {
            return read_pptx_text(path, language);
        }
        return read_ppt_binary_text(path, language);
    }
    let bytes = std::fs::read(path).map_err(|err| error_open_file_message(language, err))?;
    decode_text(&bytes, language).map(|(text, _)| text)
}

fn read_ppt_binary_text(path: &Path, language: Language) -> Result<String, String> {
    let bytes = std::fs::read(path).map_err(|err| error_open_file_message(language, err))?;
    let mut buffer = Vec::new();
    if let Ok(file) = std::fs::File::open(path)
        && let Ok(mut comp) = CompoundFile::open(&file)
        && let Ok(mut stream) = comp.open_stream("PowerPoint Document")
    {
        crate::log_if_err!(stream.read_to_end(&mut buffer));
    }
    let source = if buffer.is_empty() { &bytes } else { &buffer };
    let record_text = extract_ppt_record_text(source);
    let record_text = clean_ppt_text(record_text);
    if !record_text.trim().is_empty() {
        return Ok(record_text);
    }
    let text_utf16 = extract_utf16_strings(source);
    let text_ascii = extract_ascii_strings(source);
    if text_utf16.len() > 80 {
        return Ok(clean_doc_text(text_utf16));
    }
    if !text_ascii.is_empty() {
        return Ok(clean_doc_text(text_ascii));
    }
    if !text_utf16.is_empty() {
        return Ok(clean_doc_text(text_utf16));
    }
    Err(i18n::tr(language, "file_handler.file_read_unknown"))
}

fn is_zip_container(path: &Path) -> bool {
    let mut file = match std::fs::File::open(path) {
        Ok(file) => file,
        Err(_) => return false,
    };
    let mut header = [0u8; 4];
    if file.read_exact(&mut header).is_err() {
        return false;
    }
    matches!(
        header,
        [0x50, 0x4B, 0x03, 0x04] | [0x50, 0x4B, 0x05, 0x06] | [0x50, 0x4B, 0x07, 0x08]
    )
}

fn extract_ppt_record_text(data: &[u8]) -> String {
    let mut paragraphs = Vec::new();
    parse_ppt_records(data, &mut paragraphs);
    paragraphs.join("\n\n")
}

fn clean_ppt_text(text: String) -> String {
    let mut out = String::new();
    for block in text.split("\n\n") {
        let mut kept = Vec::new();
        for line in block.lines() {
            if should_keep_ppt_line(line) {
                kept.push(line);
            }
        }
        if kept.is_empty() {
            continue;
        }
        if !out.is_empty() {
            out.push_str("\n\n");
        }
        out.push_str(&kept.join("\n"));
    }
    out
}

fn should_keep_ppt_line(line: &str) -> bool {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return false;
    }
    let lower = trimmed.to_ascii_lowercase();
    if trimmed == "*" || trimmed == "•" {
        return false;
    }
    if lower.contains("click to edit")
        || lower.contains("click to add")
        || lower.contains("fare clic")
    {
        return false;
    }
    if lower.contains("master title")
        || lower.contains("master text")
        || lower.contains("master subtitle")
        || lower.contains("testo master")
        || lower.contains("titolo master")
    {
        return false;
    }
    if lower.contains("level") && lower.chars().any(|c| c.is_ascii_digit()) {
        return false;
    }
    if lower.contains("master") && lower.contains("level") {
        return false;
    }
    if is_ppt_placeholder_levels(&lower) {
        return false;
    }
    true
}

fn is_ppt_placeholder_levels(lower: &str) -> bool {
    let mut has_level = false;
    let mut has_ordinal = false;
    for token in lower.split_whitespace() {
        let token = token.trim_matches(|c: char| !c.is_ascii_alphabetic() && !c.is_ascii_digit());
        if token.is_empty() {
            continue;
        }
        match token {
            "level" => has_level = true,
            "first" | "second" | "third" | "fourth" | "fifth" => has_ordinal = true,
            "1" | "2" | "3" | "4" | "5" => has_ordinal = true,
            _ => return false,
        }
    }
    has_level && has_ordinal
}

fn parse_ppt_records(data: &[u8], out: &mut Vec<String>) {
    let mut pos = 0usize;
    while pos + 8 <= data.len() {
        let ver_inst = match data
            .get(pos..pos + 2)
            .and_then(|slice| slice.try_into().ok())
            .map(u16::from_le_bytes)
        {
            Some(v) => v,
            None => break,
        };
        let rec_type = match data
            .get(pos + 2..pos + 4)
            .and_then(|slice| slice.try_into().ok())
            .map(u16::from_le_bytes)
        {
            Some(v) => v,
            None => break,
        };
        let rec_len = match data
            .get(pos + 4..pos + 8)
            .and_then(|slice| slice.try_into().ok())
            .map(u32::from_le_bytes)
        {
            Some(v) => v as usize,
            None => break,
        };
        let body_start = pos + 8;
        let body_end = body_start.saturating_add(rec_len);
        if body_end > data.len() {
            break;
        }
        match rec_type {
            4000 => {
                let mut utf16 = Vec::with_capacity(rec_len / 2);
                for chunk in data[body_start..body_end].as_chunks::<2>().0 {
                    utf16.push(u16::from_le_bytes([chunk[0], chunk[1]]));
                }
                let text = String::from_utf16_lossy(&utf16);
                push_ppt_paragraph(out, text);
            }
            4008 => {
                let (decoded, _, _) = WINDOWS_1252.decode(&data[body_start..body_end]);
                push_ppt_paragraph(out, decoded.into_owned());
            }
            _ => {}
        }
        let ver = ver_inst & 0x000F;
        if ver == 0x000F && rec_len > 0 {
            parse_ppt_records(&data[body_start..body_end], out);
        }
        pos = body_end;
    }
}

fn push_ppt_paragraph(out: &mut Vec<String>, text: String) {
    let mut cleaned = text.replace('\r', "\n");
    cleaned = cleaned.trim_end_matches('\0').to_string();
    if cleaned.trim().is_empty() {
        return;
    }
    let lines: Vec<&str> = cleaned
        .lines()
        .map(|line| line.trim_end())
        .filter(|line| !line.is_empty())
        .collect();
    if lines.is_empty() {
        return;
    }
    out.push(lines.join("\n"));
}

fn read_pptx_text(path: &Path, language: Language) -> Result<String, String> {
    let file = std::fs::File::open(path).map_err(|err| error_open_file_message(language, err))?;
    let mut archive = ZipArchive::new(file).map_err(|err| {
        i18n::tr_f(
            language,
            "file_handler.file_read_error",
            &[("err", &err.to_string())],
        )
    })?;
    let mut slides: Vec<(u32, String)> = Vec::new();
    for i in 0..archive.len() {
        let mut file = archive.by_index(i).map_err(|err| {
            i18n::tr_f(
                language,
                "file_handler.file_read_error",
                &[("err", &err.to_string())],
            )
        })?;
        let name = file.name().to_string();
        if let Some(num) = pptx_slide_number(&name) {
            let mut bytes = Vec::new();
            file.read_to_end(&mut bytes).map_err(|err| {
                i18n::tr_f(
                    language,
                    "file_handler.file_read_error",
                    &[("err", &err.to_string())],
                )
            })?;
            let xml = String::from_utf8_lossy(&bytes);
            let text = extract_pptx_slide_text(&xml);
            slides.push((num, text));
        }
    }
    slides.sort_by_key(|(num, _)| *num);
    let mut out = String::new();
    for (_, text) in slides {
        let trimmed = text.trim();
        if trimmed.is_empty() {
            continue;
        }
        if !out.is_empty() {
            out.push_str("\n\n");
        }
        out.push_str(trimmed);
    }
    Ok(out)
}

pub fn read_odp_text(path: &Path, language: Language) -> Result<String, String> {
    let file = std::fs::File::open(path).map_err(|err| error_open_file_message(language, err))?;
    let mut archive = ZipArchive::new(file).map_err(|err| {
        i18n::tr_f(
            language,
            "file_handler.file_read_error",
            &[("err", &err.to_string())],
        )
    })?;
    let mut file = archive.by_name("content.xml").map_err(|err| {
        i18n::tr_f(
            language,
            "file_handler.file_read_error",
            &[("err", &err.to_string())],
        )
    })?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes).map_err(|err| {
        i18n::tr_f(
            language,
            "file_handler.file_read_error",
            &[("err", &err.to_string())],
        )
    })?;
    let xml = String::from_utf8_lossy(&bytes);
    Ok(extract_odf_text(&xml))
}

pub fn read_odt_text(path: &Path, language: Language) -> Result<String, String> {
    let file = std::fs::File::open(path).map_err(|err| error_open_file_message(language, err))?;
    let mut archive = ZipArchive::new(file).map_err(|err| {
        i18n::tr_f(
            language,
            "file_handler.file_read_error",
            &[("err", &err.to_string())],
        )
    })?;
    let mut file = archive.by_name("content.xml").map_err(|err| {
        i18n::tr_f(
            language,
            "file_handler.file_read_error",
            &[("err", &err.to_string())],
        )
    })?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes).map_err(|err| {
        i18n::tr_f(
            language,
            "file_handler.file_read_error",
            &[("err", &err.to_string())],
        )
    })?;
    let xml = String::from_utf8_lossy(&bytes);
    Ok(extract_odf_text(&xml))
}

fn extract_odf_text(xml: &str) -> String {
    let mut reader = XmlReader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut buf = Vec::new();
    let mut out = String::new();
    let mut paragraph_has_text = false;
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => {
                let name = e.name();
                if name.as_ref() == b"text:p" || name.as_ref() == b"text:h" {
                    paragraph_has_text = false;
                }
            }
            Ok(Event::End(e)) => {
                let name = e.name();
                if (name.as_ref() == b"text:p" || name.as_ref() == b"text:h")
                    && paragraph_has_text
                    && !out.ends_with('\n')
                {
                    out.push('\n');
                }
            }
            Ok(Event::Empty(e)) => {
                let name = e.name();
                if name.as_ref() == b"text:line-break" {
                    if !out.ends_with('\n') {
                        out.push('\n');
                    }
                } else if name.as_ref() == b"text:tab" {
                    out.push('\t');
                    paragraph_has_text = true;
                } else if name.as_ref() == b"text:s" {
                    let mut count = 1usize;
                    for attr in e.attributes().flatten() {
                        if attr.key.as_ref() == b"text:c"
                            && let Ok(val) = attr.unescape_value()
                            && let Ok(parsed) = val.parse::<usize>()
                        {
                            count = parsed.max(1);
                        }
                    }
                    for _ in 0..count {
                        out.push(' ');
                    }
                    paragraph_has_text = true;
                }
            }
            Ok(Event::Text(e)) => {
                let text = e.decode().unwrap_or_default();
                if !text.is_empty() {
                    out.push_str(&text);
                    paragraph_has_text = true;
                }
            }
            Ok(Event::CData(e)) => {
                let text = String::from_utf8_lossy(e.as_ref());
                if !text.is_empty() {
                    out.push_str(&text);
                    paragraph_has_text = true;
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
        buf.clear();
    }
    while out.ends_with('\n') {
        out.pop();
    }
    out
}

fn pptx_slide_number(name: &str) -> Option<u32> {
    let rest = name.strip_prefix("ppt/slides/slide")?;
    let number = rest.strip_suffix(".xml")?;
    number.parse().ok()
}

fn extract_pptx_slide_text(xml: &str) -> String {
    let mut reader = XmlReader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut buf = Vec::new();
    let mut out = String::new();
    let mut paragraph_has_text = false;
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) if e.name().as_ref() == b"a:p" => {
                paragraph_has_text = false;
            }
            Ok(Event::Start(_)) => {}
            Ok(Event::End(e))
                if e.name().as_ref() == b"a:p" && paragraph_has_text && !out.ends_with('\n') =>
            {
                out.push('\n');
            }
            Ok(Event::End(_)) => {}
            Ok(Event::Empty(e)) => {
                let name = e.name();
                if name.as_ref() == b"a:br" {
                    if !out.ends_with('\n') {
                        out.push('\n');
                    }
                } else if name.as_ref() == b"a:tab" {
                    out.push('\t');
                    paragraph_has_text = true;
                }
            }
            Ok(Event::Text(e)) => {
                let text = e.decode().unwrap_or_default();
                if !text.is_empty() {
                    out.push_str(&text);
                    paragraph_has_text = true;
                }
            }
            Ok(Event::CData(e)) => {
                let text = String::from_utf8_lossy(e.as_ref());
                if !text.is_empty() {
                    out.push_str(&text);
                    paragraph_has_text = true;
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
        buf.clear();
    }
    if out.ends_with('\n') {
        out.pop();
    }
    out
}

// --- EPUB Parsing ---

#[derive(Clone, Debug)]
pub struct EpubIndexEntry {
    pub title: String,
    pub target_utf16: i32,
    pub children: Vec<EpubIndexEntry>,
}

#[derive(Clone, Debug)]
pub struct EpubDocumentContent {
    pub text: String,
    pub index: Vec<EpubIndexEntry>,
}

#[derive(Clone, Debug)]
struct RawEpubIndexEntry {
    title: String,
    target: String,
    children: Vec<RawEpubIndexEntry>,
}

#[derive(Clone, Debug)]
struct EpubResourcePlacement {
    text_start: usize,
    text_len: usize,
    anchors: std::collections::HashMap<String, usize>,
}

pub fn read_epub_text(path: &Path, language: Language) -> Result<String, String> {
    read_epub_document(path, language).map(|document| document.text)
}

pub fn read_epub_document(path: &Path, language: Language) -> Result<EpubDocumentContent, String> {
    use epub::doc::EpubDoc;
    use std::collections::HashMap;

    let mut doc = EpubDoc::new(path).map_err(|e| {
        i18n::tr_f(
            language,
            "file_handler.epub_read_error",
            &[("err", &e.to_string())],
        )
    })?;
    let mut full_text = String::new();
    let mut placements: HashMap<String, EpubResourcePlacement> = HashMap::new();

    if let Some(title_item) = doc.mdata("title") {
        full_text.push_str(&title_item.value);
        full_text.push_str("\n\n");
    }

    let spine = doc.spine.clone();
    for item in spine {
        let resource_path = doc
            .resources
            .get(&item.idref)
            .map(|resource| resource.path.clone());
        if let Some((content, mime)) = doc.get_resource(&item.idref)
            && (mime.contains("xhtml") || mime.contains("html") || mime.contains("xml"))
        {
            let text = String::from_utf8(content.clone())
                .unwrap_or_else(|_| String::from_utf8_lossy(&content).to_string());
            let (cleaned, anchors) = html_to_text_with_anchors(&text);
            let (filtered, filtered_anchors) = filter_epub_text_with_anchors(&cleaned, &anchors);
            if filtered.trim().is_empty() {
                continue;
            }

            let text_start = full_text.len();
            full_text.push_str(&filtered);
            full_text.push('\n');

            if let Some(resource_path) = resource_path {
                placements.insert(
                    normalize_epub_internal_path(&percent_decode_epub_component(
                        &resource_path.to_string_lossy(),
                    )),
                    EpubResourcePlacement {
                        text_start,
                        text_len: filtered.len(),
                        anchors: filtered_anchors,
                    },
                );
            }
        }
    }

    if full_text.trim().is_empty() {
        return Err(i18n::tr(language, "file_handler.epub_no_text"));
    }

    let epub3_index = extract_epub3_navigation(&mut doc);
    let mut index = resolve_epub_index_entries(&epub3_index, &placements, &full_text);
    if index.is_empty() {
        let ncx_index = convert_ncx_navigation(&doc.toc);
        index = resolve_epub_index_entries(&ncx_index, &placements, &full_text);
    }

    Ok(EpubDocumentContent {
        text: full_text,
        index,
    })
}

fn convert_ncx_navigation(points: &[epub::doc::NavPoint]) -> Vec<RawEpubIndexEntry> {
    points
        .iter()
        .filter_map(|point| {
            let title = normalize_epub_index_label(&point.label);
            if title.is_empty() {
                return None;
            }
            Some(RawEpubIndexEntry {
                title,
                target: point.content.to_string_lossy().into_owned(),
                children: convert_ncx_navigation(&point.children),
            })
        })
        .collect()
}

fn extract_epub3_navigation<R: std::io::Read + std::io::Seek>(
    doc: &mut epub::doc::EpubDoc<R>,
) -> Vec<RawEpubIndexEntry> {
    use scraper::{Html, Selector};

    let Some(nav_id) = doc.get_nav_id() else {
        return Vec::new();
    };
    let Some(nav_path) = doc
        .resources
        .get(&nav_id)
        .map(|resource| resource.path.clone())
    else {
        return Vec::new();
    };
    let Some((content, _mime)) = doc.get_resource(&nav_id) else {
        return Vec::new();
    };
    let html = String::from_utf8(content)
        .unwrap_or_else(|bytes| String::from_utf8_lossy(bytes.as_bytes()).into_owned());
    let document = Html::parse_document(&html);
    let Ok(nav_selector) = Selector::parse("nav") else {
        return Vec::new();
    };

    let navigation_elements = document.select(&nav_selector).collect::<Vec<_>>();
    for nav in &navigation_elements {
        let nav_type = nav
            .value()
            .attr("epub:type")
            .or_else(|| nav.value().attr("type"))
            .unwrap_or_default();
        let role = nav.value().attr("role").unwrap_or_default();
        if nav_type
            .split_ascii_whitespace()
            .any(|value| value == "toc")
            || role.eq_ignore_ascii_case("doc-toc")
        {
            return parse_epub3_nav_element(*nav, &nav_path);
        }
    }

    for nav in &navigation_elements {
        let marker = format!(
            "{} {} {}",
            nav.value().attr("id").unwrap_or_default(),
            nav.value().attr("class").unwrap_or_default(),
            nav.value().attr("aria-label").unwrap_or_default(),
        )
        .to_ascii_lowercase();
        if marker
            .split(|ch: char| !ch.is_ascii_alphanumeric())
            .any(|part| matches!(part, "toc" | "contents" | "index" | "indice"))
        {
            return parse_epub3_nav_element(*nav, &nav_path);
        }
    }

    if let [nav] = navigation_elements.as_slice() {
        return parse_epub3_nav_element(*nav, &nav_path);
    }
    Vec::new()
}

fn parse_epub3_nav_element(
    nav: scraper::ElementRef<'_>,
    nav_path: &std::path::Path,
) -> Vec<RawEpubIndexEntry> {
    use scraper::ElementRef;

    let root_list = nav
        .children()
        .filter_map(ElementRef::wrap)
        .find(|element| matches!(element.value().name(), "ol" | "ul"))
        .or_else(|| {
            nav.descendants()
                .filter_map(ElementRef::wrap)
                .find(|element| matches!(element.value().name(), "ol" | "ul"))
        });
    root_list
        .map(|list| parse_epub3_list(list, nav_path))
        .unwrap_or_default()
}

fn parse_epub3_list(
    list: scraper::ElementRef<'_>,
    nav_path: &std::path::Path,
) -> Vec<RawEpubIndexEntry> {
    use scraper::ElementRef;

    let mut entries = Vec::new();
    for item in list
        .children()
        .filter_map(ElementRef::wrap)
        .filter(|element| element.value().name() == "li")
    {
        let nested_list = item
            .children()
            .filter_map(ElementRef::wrap)
            .find(|element| matches!(element.value().name(), "ol" | "ul"));
        let children = nested_list
            .map(|child_list| parse_epub3_list(child_list, nav_path))
            .unwrap_or_default();
        let link = first_epub3_element_before_nested_list(item, &["a"]);
        let label_element =
            link.or_else(|| first_epub3_element_before_nested_list(item, &["span", "div", "p"]));
        let title = label_element
            .map(|element| {
                normalize_epub_index_label(&element.text().collect::<Vec<_>>().join(" "))
            })
            .unwrap_or_default();
        if title.is_empty() {
            entries.extend(children);
            continue;
        }

        let target = link
            .and_then(|element| element.value().attr("href"))
            .map(|href| resolve_epub_relative_target(nav_path, href))
            .or(children.first().map(|child| child.target.clone()));
        let Some(target) = target else {
            entries.extend(children);
            continue;
        };

        entries.push(RawEpubIndexEntry {
            title,
            target,
            children,
        });
    }
    entries
}

fn first_epub3_element_before_nested_list<'a>(
    item: scraper::ElementRef<'a>,
    names: &[&str],
) -> Option<scraper::ElementRef<'a>> {
    use scraper::ElementRef;

    for child in item.children().filter_map(ElementRef::wrap) {
        let name = child.value().name();
        if matches!(name, "ol" | "ul") {
            continue;
        }
        if names.contains(&name) {
            return Some(child);
        }
        if let Some(element) = first_epub3_element_before_nested_list(child, names) {
            return Some(element);
        }
    }
    None
}

fn resolve_epub_relative_target(nav_path: &std::path::Path, href: &str) -> String {
    let (path_part, fragment) = href.split_once('#').unwrap_or((href, ""));
    let path_part = path_part.split('?').next().unwrap_or(path_part);
    let decoded_path = percent_decode_epub_component(path_part);
    let base = nav_path.parent().unwrap_or(std::path::Path::new(""));
    let joined = if decoded_path.trim().is_empty() {
        nav_path.to_path_buf()
    } else {
        base.join(decoded_path)
    };
    let normalized =
        normalize_epub_internal_path(&percent_decode_epub_component(&joined.to_string_lossy()));
    if fragment.is_empty() {
        normalized
    } else {
        format!("{}#{}", normalized, percent_decode_epub_component(fragment))
    }
}

fn resolve_epub_index_entries(
    entries: &[RawEpubIndexEntry],
    placements: &std::collections::HashMap<String, EpubResourcePlacement>,
    full_text: &str,
) -> Vec<EpubIndexEntry> {
    let mut resolved = Vec::new();
    for entry in entries {
        let children = resolve_epub_index_entries(&entry.children, placements, full_text);
        let target_utf16 = resolve_epub_target(&entry.target, &entry.title, placements, full_text)
            .or(children.first().map(|child| child.target_utf16));
        if let Some(target_utf16) = target_utf16 {
            resolved.push(EpubIndexEntry {
                title: entry.title.clone(),
                target_utf16,
                children,
            });
        } else {
            resolved.extend(children);
        }
    }
    resolved
}

fn resolve_epub_target(
    target: &str,
    title: &str,
    placements: &std::collections::HashMap<String, EpubResourcePlacement>,
    full_text: &str,
) -> Option<i32> {
    let (path_part, fragment) = target.split_once('#').unwrap_or((target, ""));
    let path_part = path_part.split('?').next().unwrap_or(path_part);
    let normalized_path = normalize_epub_internal_path(&percent_decode_epub_component(path_part));
    let placement = placements.get(&normalized_path).or_else(|| {
        placements.iter().find_map(|(path, placement)| {
            if path.ends_with(&format!("/{}", normalized_path))
                || normalized_path.ends_with(&format!("/{}", path))
                || path.eq_ignore_ascii_case(&normalized_path)
            {
                Some(placement)
            } else {
                None
            }
        })
    })?;

    let decoded_fragment = percent_decode_epub_component(fragment);
    let local_offset = if decoded_fragment.is_empty() {
        0
    } else {
        placement
            .anchors
            .get(&decoded_fragment)
            .copied()
            .or_else(|| {
                placement.anchors.iter().find_map(|(name, offset)| {
                    name.eq_ignore_ascii_case(&decoded_fragment)
                        .then_some(*offset)
                })
            })
            .unwrap_or(0)
    };
    let raw_target = placement.text_start.saturating_add(local_offset);
    let aligned_target = align_epub_target_to_title(full_text, placement, raw_target, title);
    Some(byte_offset_to_editor_utf16(full_text, aligned_target))
}

fn align_epub_target_to_title(
    full_text: &str,
    placement: &EpubResourcePlacement,
    raw_target: usize,
    title: &str,
) -> usize {
    const TITLE_SEARCH_RADIUS_BYTES: usize = 4096;
    let normalized_title = normalize_epub_index_label(title);
    if normalized_title.is_empty() {
        return raw_target;
    }

    let resource_start = placement.text_start.min(full_text.len());
    let resource_end = placement
        .text_start
        .saturating_add(placement.text_len)
        .min(full_text.len());
    if resource_start >= resource_end {
        return raw_target;
    }
    let search_start = raw_target
        .saturating_sub(TITLE_SEARCH_RADIUS_BYTES)
        .max(resource_start);
    let search_end = raw_target
        .saturating_add(TITLE_SEARCH_RADIUS_BYTES)
        .min(resource_end);

    let mut best_match = None;
    let mut line_start = resource_start;
    for segment in full_text[resource_start..resource_end].split_inclusive('\n') {
        let line_end = line_start.saturating_add(segment.len());
        if line_end >= search_start && line_start <= search_end {
            let line = segment.strip_suffix('\n').unwrap_or(segment).trim();
            if normalize_epub_index_label(line).eq_ignore_ascii_case(&normalized_title) {
                let leading = segment.find(line).unwrap_or(0);
                let candidate = line_start.saturating_add(leading);
                let distance = candidate.abs_diff(raw_target);
                if best_match
                    .as_ref()
                    .is_none_or(|(_, best_distance)| distance < *best_distance)
                {
                    best_match = Some((candidate, distance));
                }
            }
        }
        line_start = line_end;
    }
    best_match.map(|(offset, _)| offset).unwrap_or(raw_target)
}

pub(crate) fn byte_offset_to_editor_utf16(text: &str, byte_offset: usize) -> i32 {
    let mut safe_offset = byte_offset.min(text.len());
    while safe_offset > 0 && !text.is_char_boundary(safe_offset) {
        safe_offset -= 1;
    }
    let prefix = &text[..safe_offset];
    // RichEdit stores each paragraph break as one character even though
    // SetWindowText receives CRLF. EM_EXSETSEL therefore expects the original
    // single-newline UTF-16 offset, without an extra unit for the inserted CR.
    prefix.encode_utf16().count().min(i32::MAX as usize) as i32
}

pub(crate) fn normalize_epub_index_label(label: &str) -> String {
    label.split_whitespace().collect::<Vec<_>>().join(" ")
}

pub(crate) fn normalize_epub_internal_path(path: &str) -> String {
    let mut parts: Vec<&str> = Vec::new();
    let replaced = path.replace('\\', "/");
    for part in replaced.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                parts.truncate(parts.len().saturating_sub(1));
            }
            _ => parts.push(part),
        }
    }
    parts.join("/")
}

pub(crate) fn percent_decode_epub_component(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' && index + 2 < bytes.len() {
            let high = hex_value(bytes[index + 1]);
            let low = hex_value(bytes[index + 2]);
            if let (Some(high), Some(low)) = (high, low) {
                decoded.push((high << 4) | low);
                index += 3;
                continue;
            }
        }
        decoded.push(bytes[index]);
        index += 1;
    }
    String::from_utf8(decoded)
        .unwrap_or_else(|bytes| String::from_utf8_lossy(bytes.as_bytes()).into_owned())
}

fn hex_value(value: u8) -> Option<u8> {
    match value {
        b'0'..=b'9' => Some(value - b'0'),
        b'a'..=b'f' => Some(value - b'a' + 10),
        b'A'..=b'F' => Some(value - b'A' + 10),
        _ => None,
    }
}

fn filter_epub_text_with_anchors(
    cleaned: &str,
    anchors: &std::collections::HashMap<String, usize>,
) -> (String, std::collections::HashMap<String, usize>) {
    let mut output = String::new();
    let mut kept_ranges: Vec<(usize, usize, usize)> = Vec::new();
    let mut source_start = 0usize;

    for segment in cleaned.split_inclusive('\n') {
        let line = segment.strip_suffix('\n').unwrap_or(segment);
        let explicit_break = line.ends_with(SONARPAD_EXPLICIT_BREAK_MARKER);
        let visible_line = line
            .strip_suffix(SONARPAD_EXPLICIT_BREAK_MARKER)
            .unwrap_or(line);
        let trimmed = visible_line.trim();
        if !trimmed.is_empty()
            && !is_epub_metadata_noise_line(trimmed)
            && !(trimmed.starts_with("part") && trimmed.len() <= 12)
        {
            let leading = visible_line.find(trimmed).unwrap_or(0);
            let text_start = source_start.saturating_add(leading);
            let text_end = text_start.saturating_add(trimmed.len());
            let destination_start = output.len();
            output.push_str(trimmed);
            output.push('\n');
            kept_ranges.push((text_start, text_end, destination_start));
        } else if explicit_break {
            let marker_start = source_start.saturating_add(visible_line.len());
            let destination_start = output.len();
            output.push('\n');
            kept_ranges.push((
                marker_start,
                marker_start.saturating_add(SONARPAD_EXPLICIT_BREAK_MARKER.len_utf8()),
                destination_start,
            ));
        }
        source_start = source_start.saturating_add(segment.len());
    }

    if source_start < cleaned.len() {
        let line = &cleaned[source_start..];
        let explicit_break = line.ends_with(SONARPAD_EXPLICIT_BREAK_MARKER);
        let visible_line = line
            .strip_suffix(SONARPAD_EXPLICIT_BREAK_MARKER)
            .unwrap_or(line);
        let trimmed = visible_line.trim();
        if !trimmed.is_empty()
            && !is_epub_metadata_noise_line(trimmed)
            && !(trimmed.starts_with("part") && trimmed.len() <= 12)
        {
            let leading = visible_line.find(trimmed).unwrap_or(0);
            let text_start = source_start.saturating_add(leading);
            let text_end = text_start.saturating_add(trimmed.len());
            let destination_start = output.len();
            output.push_str(trimmed);
            output.push('\n');
            kept_ranges.push((text_start, text_end, destination_start));
        } else if explicit_break {
            let marker_start = source_start.saturating_add(visible_line.len());
            let destination_start = output.len();
            output.push('\n');
            kept_ranges.push((
                marker_start,
                marker_start.saturating_add(SONARPAD_EXPLICIT_BREAK_MARKER.len_utf8()),
                destination_start,
            ));
        }
    }

    let mut mapped_anchors = std::collections::HashMap::new();
    for (name, source_offset) in anchors {
        let mapped = kept_ranges
            .iter()
            .find_map(|(range_start, range_end, destination_start)| {
                if source_offset <= range_end {
                    let relative = source_offset.saturating_sub(*range_start);
                    Some(destination_start.saturating_add(relative.min(range_end - range_start)))
                } else {
                    None
                }
            })
            .unwrap_or(output.len());
        mapped_anchors.insert(name.clone(), mapped);
    }

    (output, mapped_anchors)
}

pub fn read_epub_chapters(path: &Path, language: Language) -> Result<Vec<String>, String> {
    use epub::doc::EpubDoc;
    let mut doc = EpubDoc::new(path).map_err(|e| {
        i18n::tr_f(
            language,
            "file_handler.epub_read_error",
            &[("err", &e.to_string())],
        )
    })?;
    let mut chapters = Vec::new();

    let spine = doc.spine.clone();
    for item in spine {
        if let Some((content, mime)) = doc.get_resource(&item.idref)
            && (mime.contains("xhtml") || mime.contains("html") || mime.contains("xml"))
        {
            let text = String::from_utf8(content.clone())
                .unwrap_or_else(|_| String::from_utf8_lossy(&content).to_string());

            let cleaned = html_to_text(&text);
            let mut lines: Vec<String> = Vec::new();
            for line in cleaned.lines() {
                let trimmed = line.trim();
                if trimmed.is_empty()
                    || is_epub_metadata_noise_line(trimmed)
                    || (trimmed.starts_with("part") && trimmed.len() <= 12)
                {
                    continue;
                }
                lines.push(trimmed.to_string());
            }
            if lines.len() >= 2 {
                let first = lines[0].trim();
                let second = lines[1].trim();
                if !first.is_empty() && first == second {
                    lines.remove(1);
                }
            }
            let mut chapter_text = lines.join("\n");
            if chapter_text.ends_with('\n') {
                chapter_text.pop();
            }
            if !chapter_text.trim().is_empty() {
                chapters.push(chapter_text);
            }
        }
    }

    if chapters.is_empty() {
        return Err(i18n::tr(language, "file_handler.epub_no_text"));
    }

    Ok(chapters)
}

fn is_epub_metadata_noise_line(line: &str) -> bool {
    let normalized = line.split_whitespace().collect::<Vec<_>>().join(" ");
    normalized.eq_ignore_ascii_case("epub r1.0")
        || normalized.eq_ignore_ascii_case("epub base r2.1")
}

pub fn read_html_text(path: &Path, language: Language) -> Result<(String, TextEncoding), String> {
    let bytes = std::fs::read(path)
        .map_err(|err| crate::settings::error_open_file_message(language, err))?;
    let (text, encoding) = decode_text(&bytes, language)?;
    let cleaned = html_to_text(&text);
    Ok((cleaned, encoding))
}

const SONARPAD_LINE_BREAK_CLASS: &str = "sonarpad-preserve-line-break";
const SONARPAD_EXPLICIT_BREAK_MARKER: char = '\u{E000}';

fn html_to_text(html: &str) -> String {
    html_to_text_with_anchors(html)
        .0
        .replace(SONARPAD_EXPLICIT_BREAK_MARKER, "")
}

pub(crate) fn html_to_text_with_anchors(
    html: &str,
) -> (String, std::collections::HashMap<String, usize>) {
    let mut out = String::new();
    let mut anchors = std::collections::HashMap::new();
    let mut inside = false;
    let mut tag = String::new();
    let mut last_newline = false;
    let mut skip_stack: Vec<String> = Vec::new();
    let mut in_comment = false;
    let mut entity = String::new();
    let mut in_entity = false;

    for ch in html.chars() {
        if in_comment {
            tag.push(ch);
            if tag.ends_with("-->") {
                in_comment = false;
                tag.clear();
            }
            continue;
        }

        if inside {
            if ch == '>' {
                inside = false;
                let tag_trimmed = tag.trim();
                if tag_trimmed.starts_with("!--") {
                    if !tag_trimmed.ends_with("--") {
                        in_comment = true;
                    }
                    tag.clear();
                    continue;
                }

                let tag_name = tag_trimmed
                    .trim()
                    .trim_start_matches('/')
                    .split_whitespace()
                    .next()
                    .unwrap_or("")
                    .trim_end_matches('/')
                    .to_ascii_lowercase();
                let is_closing = tag_trimmed.starts_with('/');

                if matches!(tag_name.as_str(), "head" | "style" | "script" | "title") {
                    if is_closing {
                        if let Some(pos) = skip_stack.iter().rposition(|value| value == &tag_name) {
                            skip_stack.truncate(pos);
                        }
                    } else {
                        skip_stack.push(tag_name.clone());
                    }
                    tag.clear();
                    continue;
                }
                let explicit_sonarpad_break = tag_name == "br"
                    && html_tag_attribute(tag_trimmed, "class").is_some_and(|classes| {
                        classes
                            .split_ascii_whitespace()
                            .any(|class| class == SONARPAD_LINE_BREAK_CLASS)
                    });
                if explicit_sonarpad_break && skip_stack.is_empty() && !out.is_empty() {
                    out.push(SONARPAD_EXPLICIT_BREAK_MARKER);
                    out.push('\n');
                    last_newline = true;
                } else if matches!(
                    tag_name.as_str(),
                    "br" | "p"
                        | "div"
                        | "li"
                        | "tr"
                        | "hr"
                        | "ul"
                        | "ol"
                        | "table"
                        | "h1"
                        | "h2"
                        | "h3"
                        | "h4"
                        | "h5"
                        | "h6"
                ) && skip_stack.is_empty()
                    && !last_newline
                    && !out.is_empty()
                {
                    out.push('\n');
                    last_newline = true;
                }

                if !is_closing && skip_stack.is_empty() {
                    for attribute in ["id", "xml:id", "name"] {
                        if let Some(value) = html_tag_attribute(tag_trimmed, attribute)
                            && !value.is_empty()
                        {
                            anchors.entry(value).or_insert(out.len());
                        }
                    }
                }
                tag.clear();
            } else {
                tag.push(ch);
            }
            continue;
        }
        if ch == '<' {
            if in_entity {
                append_html_entity(&mut out, &entity);
                entity.clear();
                in_entity = false;
            }
            inside = true;
            continue;
        }
        if !skip_stack.is_empty() {
            continue;
        }
        if in_entity {
            if ch == ';' {
                append_html_entity(&mut out, &entity);
                entity.clear();
                in_entity = false;
                last_newline = out.ends_with('\n');
            } else if entity.len() < 16 && !ch.is_whitespace() {
                entity.push(ch);
            } else {
                out.push('&');
                out.push_str(&entity);
                out.push(ch);
                entity.clear();
                in_entity = false;
                last_newline = ch == '\n';
            }
            continue;
        }
        if ch == '&' {
            in_entity = true;
            entity.clear();
            continue;
        }
        out.push(ch);
        last_newline = ch == '\n';
    }

    if in_entity {
        out.push('&');
        out.push_str(&entity);
    }

    (out, anchors)
}

fn html_tag_attribute(tag: &str, requested_name: &str) -> Option<String> {
    let bytes = tag.as_bytes();
    let mut index = 0usize;
    while index < bytes.len() && !bytes[index].is_ascii_whitespace() {
        index += 1;
    }
    while index < bytes.len() {
        while index < bytes.len() && (bytes[index].is_ascii_whitespace() || bytes[index] == b'/') {
            index += 1;
        }
        let name_start = index;
        while index < bytes.len()
            && !bytes[index].is_ascii_whitespace()
            && bytes[index] != b'='
            && bytes[index] != b'/'
        {
            index += 1;
        }
        if name_start == index {
            break;
        }
        let name = &tag[name_start..index];
        while index < bytes.len() && bytes[index].is_ascii_whitespace() {
            index += 1;
        }
        if index >= bytes.len() || bytes[index] != b'=' {
            continue;
        }
        index += 1;
        while index < bytes.len() && bytes[index].is_ascii_whitespace() {
            index += 1;
        }
        if index >= bytes.len() {
            break;
        }
        let (value_start, value_end) = if bytes[index] == b'\'' || bytes[index] == b'"' {
            let quote = bytes[index];
            index += 1;
            let start = index;
            while index < bytes.len() && bytes[index] != quote {
                index += 1;
            }
            let end = index;
            if index < bytes.len() {
                index += 1;
            }
            (start, end)
        } else {
            let start = index;
            while index < bytes.len() && !bytes[index].is_ascii_whitespace() && bytes[index] != b'/'
            {
                index += 1;
            }
            (start, index)
        };
        if name.eq_ignore_ascii_case(requested_name) {
            return Some(decode_basic_html_entities(&tag[value_start..value_end]));
        }
    }
    None
}

fn append_html_entity(output: &mut String, entity: &str) {
    match entity {
        "nbsp" => output.push(' '),
        "lt" => output.push('<'),
        "gt" => output.push('>'),
        "amp" => output.push('&'),
        "quot" => output.push('"'),
        "apos" => output.push('\''),
        value if value.starts_with("#x") || value.starts_with("#X") => {
            if let Ok(number) = u32::from_str_radix(&value[2..], 16)
                && let Some(ch) = char::from_u32(number)
            {
                output.push(ch);
                return;
            }
            output.push('&');
            output.push_str(entity);
            output.push(';');
        }
        value if value.starts_with('#') => {
            if let Ok(number) = value[1..].parse::<u32>()
                && let Some(ch) = char::from_u32(number)
            {
                output.push(ch);
                return;
            }
            output.push('&');
            output.push_str(entity);
            output.push(';');
        }
        _ => {
            output.push('&');
            output.push_str(entity);
            output.push(';');
        }
    }
}

fn decode_basic_html_entities(value: &str) -> String {
    let mut output = String::new();
    let mut entity = String::new();
    let mut in_entity = false;
    for ch in value.chars() {
        if in_entity {
            if ch == ';' {
                append_html_entity(&mut output, &entity);
                entity.clear();
                in_entity = false;
            } else {
                entity.push(ch);
            }
        } else if ch == '&' {
            in_entity = true;
        } else {
            output.push(ch);
        }
    }
    if in_entity {
        output.push('&');
        output.push_str(&entity);
    }
    output
}

#[cfg(test)]
mod tests {
    use super::{
        EpubResourcePlacement, Language, WINDOWS_1250, WINDOWS_1252, align_epub_target_to_title,
        byte_offset_to_editor_utf16, choose_ansi_decoding, decode_ansi_best_effort,
        embedded_nul_count, filter_epub_text_with_anchors, html_to_text, html_to_text_with_anchors,
        is_epub_metadata_noise_line, strip_embedded_nuls,
    };

    #[test]
    fn marked_epub_breaks_survive_filtering_consecutively() {
        let html = "<p>Prima<br class=\"sonarpad-preserve-line-break\"/><br class=\"sonarpad-preserve-line-break\"/><br class=\"sonarpad-preserve-line-break\"/><br class=\"sonarpad-preserve-line-break\"/>Seconda</p>";
        let (cleaned, anchors) = html_to_text_with_anchors(html);
        let (filtered, _) = filter_epub_text_with_anchors(&cleaned, &anchors);
        assert_eq!(filtered, "Prima\n\n\n\nSeconda\n");
    }

    #[test]
    #[ignore = "Requires a private PDF and password supplied through the environment"]
    fn protected_pdf_password_roundtrip() {
        let path =
            std::path::PathBuf::from(std::env::var("SONARPAD_TEST_PDF_PATH").expect("PDF path"));
        let password = std::env::var("SONARPAD_TEST_PDF_PASSWORD").expect("PDF password");
        let before = std::fs::read(&path).expect("read encrypted PDF");
        assert!(matches!(
            super::read_pdf_text_with_status(&path, Language::English),
            Ok(super::PdfTextResult::PasswordRequired { incorrect: false })
        ));
        assert!(matches!(
            super::read_pdf_text_with_password(
                &path,
                Language::English,
                Some("incorrect-test-password")
            ),
            Ok(super::PdfTextResult::PasswordRequired { incorrect: true })
        ));
        assert!(matches!(
            super::read_pdf_text_with_password(&path, Language::English, Some(&password)),
            Ok(super::PdfTextResult::Text(text)) if !text.trim().is_empty()
        ));
        let directory =
            std::env::temp_dir().join(format!("sonarpad-pdf-copy-{}", std::process::id()));
        std::fs::create_dir_all(&directory).unwrap();
        let output = directory.join("copy.pdf");
        assert!(
            super::save_pdf_without_password(&path, &path, &password, Language::English).is_err()
        );
        std::fs::write(&output, b"existing destination").unwrap();
        assert!(
            super::save_pdf_without_password(
                &path,
                &output,
                "incorrect-test-password",
                Language::English
            )
            .is_err()
        );
        assert!(std::fs::read(&output).unwrap() == b"existing destination");
        super::save_pdf_without_password(&path, &output, &password, Language::English).unwrap();
        let copy = super::LoDocument::load(&output).unwrap();
        assert!(!copy.is_encrypted());
        assert!(copy.trailer.get(b"Encrypt").is_err());
        let visual_check = || {
            use pdfium_render::prelude::*;
            let bindings =
                Pdfium::bind_to_library(crate::settings::settings_dir().join("pdfium.dll"))
                    .unwrap();
            let pdfium = Pdfium::new(bindings);
            let original = pdfium.load_pdf_from_file(&path, Some(&password)).unwrap();
            let exported = pdfium.load_pdf_from_file(&output, None).unwrap();
            assert_eq!(original.pages().len(), exported.pages().len());
            for (a, b) in original.pages().iter().zip(exported.pages().iter()) {
                assert_eq!(a.width(), b.width());
                assert_eq!(a.height(), b.height());
                assert!(a.text().unwrap().all() == b.text().unwrap().all());
                let config = PdfRenderConfig::new().set_target_width(1000);
                assert!(
                    a.render_with_config(&config).unwrap().as_raw_bytes()
                        == b.render_with_config(&config).unwrap().as_raw_bytes()
                );
            }
        };
        let checked = std::panic::catch_unwind(visual_check);
        std::fs::remove_file(&output).unwrap();
        std::fs::remove_dir(&directory).unwrap();
        assert!(checked.is_ok(), "PDF content or rendering changed");
        assert!(std::fs::read(&path).expect("read unchanged encrypted PDF") == before);
    }

    #[test]
    fn japanese_pdf_lines_are_not_flattened() {
        let input = "Ai: こんにちは\nJean: こんばんは\nThird line";
        let result = super::normalize_pdf_paragraphs(input);
        assert_eq!(result, input);
    }

    #[test]
    fn latin_pdf_keeps_legacy_paragraph_normalization() {
        let input = "A moderately long example line\nSecond line continues that thought";
        assert_eq!(
            super::normalize_pdf_paragraphs(input),
            "A moderately long example line Second line continues that thought"
        );
    }

    #[test]
    fn pdf_embedded_nuls_are_removed_without_truncating_following_text() {
        let cleaned = strip_embedded_nuls("testo prima\0testo dopo".to_string(), "unit test");

        assert_eq!(cleaned, "testo primatesto dopo");
        assert_eq!(embedded_nul_count(&cleaned), 0);
    }

    #[test]
    fn html_to_text_keeps_text_after_inline_comment() {
        let html = "<html><!-- note --><body><p>Alpha</p><p>Beta</p></body></html>";
        let out = html_to_text(html);
        assert!(out.contains("Alpha"));
        assert!(out.contains("Beta"));
    }

    #[test]
    fn html_to_text_handles_comment_with_gt_character() {
        let html = "<p>First</p><!-- 1 > 2 --><p>Second</p>";
        let out = html_to_text(html);
        assert!(out.contains("First"));
        assert!(out.contains("Second"));
    }

    #[test]
    fn epub_metadata_noise_lines_are_detected() {
        assert!(is_epub_metadata_noise_line("ePub r1.0"));
        assert!(is_epub_metadata_noise_line("ePub base r2.1"));
        assert!(is_epub_metadata_noise_line("  EPUB   BASE   R2.1  "));
        assert!(!is_epub_metadata_noise_line("ePub base r2.2"));
        assert!(!is_epub_metadata_noise_line("Capitolo 1"));
    }

    #[test]
    fn epub_index_offset_counts_rich_edit_line_break_as_one_character() {
        let text = "Introduzione\nSeconda riga\nProemio\nTesto";
        let title_offset = text.find("Proemio").unwrap_or_default();

        assert_eq!(
            byte_offset_to_editor_utf16(text, title_offset),
            "Introduzione\nSeconda riga\n".encode_utf16().count() as i32
        );
    }

    #[test]
    fn epub_index_offset_remains_utf16_safe_with_unicode_before_title() {
        let text = "È un’introduzione 😀\nProemio";
        let title_offset = text.find("Proemio").unwrap_or_default();

        assert_eq!(
            byte_offset_to_editor_utf16(text, title_offset),
            "È un’introduzione 😀\n".encode_utf16().count() as i32
        );
    }

    #[test]
    fn epub_index_target_moves_back_to_exact_title_line() {
        let text = "Proemio\nPrima riga del testo.\nSeconda riga.\n";
        let raw_target = text.find("Seconda riga").unwrap_or_default();
        let placement = EpubResourcePlacement {
            text_start: 0,
            text_len: text.len(),
            anchors: std::collections::HashMap::new(),
        };

        assert_eq!(
            align_epub_target_to_title(text, &placement, raw_target, "Proemio"),
            0
        );
    }

    #[test]
    fn epub_index_target_does_not_match_title_outside_resource() {
        let text = "Proemio\nTesto precedente.\nCapitolo reale\nCorpo.\n";
        let resource_start = text.find("Capitolo reale").unwrap_or_default();
        let raw_target = text.find("Corpo").unwrap_or_default();
        let placement = EpubResourcePlacement {
            text_start: resource_start,
            text_len: text.len().saturating_sub(resource_start),
            anchors: std::collections::HashMap::new(),
        };

        assert_eq!(
            align_epub_target_to_title(text, &placement, raw_target, "Proemio"),
            raw_target
        );
    }

    #[test]
    fn choose_ansi_prefers_cp1252_for_italian_when_acp_looks_cp1250() {
        let source = "Luana si asciugò il sudore e arrivò all’altezza.";
        let (encoded, _, _) = WINDOWS_1252.encode(source);
        let bytes = encoded.into_owned();

        let (cp1250_text, _, _) = WINDOWS_1250.decode(&bytes);
        let (cp1252_text, _, _) = WINDOWS_1252.decode(&bytes);
        let chosen = choose_ansi_decoding(
            Language::Italian,
            &cp1250_text,
            &cp1252_text,
            Some(&cp1250_text),
        );

        assert_eq!(chosen, cp1252_text);
        assert!(chosen.contains("asciugò"));
        assert!(chosen.contains("arrivò"));
    }

    #[test]
    fn choose_ansi_prefers_cp1250_for_czech_when_cp1250_is_clear_winner() {
        let source = "Příliš žluťoučký kůň úpěl ďábelské ódy.";
        let (encoded, _, _) = WINDOWS_1250.encode(source);
        let bytes = encoded.into_owned();

        let (cp1250_text, _, _) = WINDOWS_1250.decode(&bytes);
        let (cp1252_text, _, _) = WINDOWS_1252.decode(&bytes);
        let chosen = choose_ansi_decoding(
            Language::Czech,
            &cp1250_text,
            &cp1252_text,
            Some(&cp1252_text),
        );

        assert_eq!(chosen, cp1250_text);
        assert!(chosen.contains("Příliš"));
        assert!(chosen.contains("kůň"));
    }

    #[test]
    fn decode_ansi_prefers_shift_jis_for_japanese_mojibake() {
        let source = "これは日本語のテストファイルです。";
        let shift_jis =
            encoding_rs::Encoding::for_label(b"shift_jis").expect("shift_jis encoding available");
        let (encoded, _, _) = shift_jis.encode(source);
        let bytes = encoded.into_owned();

        let decoded = decode_ansi_best_effort(&bytes, Language::Italian);
        assert_eq!(decoded, source);
    }
}

// --- DOC Parsing ---

pub fn read_doc_text(path: &Path, language: Language) -> Result<String, String> {
    let file = std::fs::File::open(path).map_err(|e| {
        i18n::tr_f(
            language,
            "file_handler.doc_open_error",
            &[("err", &e.to_string())],
        )
    })?;
    match CompoundFile::open(&file) {
        Ok(mut comp) => {
            let buffer = {
                let mut stream = comp
                    .open_stream("WordDocument")
                    .map_err(|_| i18n::tr(language, "file_handler.doc_stream_missing"))?;
                let mut buffer = Vec::new();
                stream.read_to_end(&mut buffer).map_err(|e| {
                    i18n::tr_f(
                        language,
                        "file_handler.doc_stream_read_error",
                        &[("err", &e.to_string())],
                    )
                })?;
                buffer
            };

            let mut table_bytes = Vec::new();
            if let Ok(mut table_stream) = comp.open_stream("1Table") {
                crate::log_if_err!(table_stream.read_to_end(&mut table_bytes));
            } else if let Ok(mut table_stream) = comp.open_stream("0Table") {
                crate::log_if_err!(table_stream.read_to_end(&mut table_bytes));
            }

            if !table_bytes.is_empty()
                && let Some(text) = extract_doc_text_piece_table(&buffer, &table_bytes)
            {
                return Ok(clean_doc_text(text));
            }

            let text_utf16 = extract_utf16_strings(&buffer);
            let text_ascii = extract_ascii_strings(&buffer);

            if text_utf16.len() > 100 {
                return Ok(clean_doc_text(text_utf16));
            }
            if !text_ascii.is_empty() {
                return Ok(clean_doc_text(text_ascii));
            }
            Ok(clean_doc_text(text_utf16))
        }
        Err(_) => {
            let bytes = std::fs::read(path).map_err(|e| {
                i18n::tr_f(
                    language,
                    "file_handler.file_read_error",
                    &[("err", &e.to_string())],
                )
            })?;
            if looks_like_rtf(&bytes) {
                return Ok(extract_rtf_text(&bytes));
            }
            if let Ok(text) = read_docx_text(path, language) {
                return Ok(clean_doc_text(text));
            }
            let text_utf16 = extract_utf16_strings(&bytes);
            if text_utf16.len() > 100 {
                return Ok(clean_doc_text(text_utf16));
            }
            let text_ascii = extract_ascii_strings(&bytes);
            if !text_ascii.is_empty() {
                return Ok(clean_doc_text(text_ascii));
            }
            if !text_utf16.is_empty() {
                return Ok(clean_doc_text(text_utf16));
            }
            Err(i18n::tr(language, "file_handler.file_read_unknown"))
        }
    }
}

pub fn looks_like_rtf(bytes: &[u8]) -> bool {
    let mut start = 0usize;
    if bytes.starts_with(&[0xEF, 0xBB, 0xBF]) {
        start = 3;
    }
    while start < bytes.len() && bytes[start].is_ascii_whitespace() {
        start += 1;
    }
    bytes
        .get(start..start + 5)
        .map(|s| s == b"{\\rtf")
        .unwrap_or(false)
}

struct DocPiece {
    offset: usize,
    cp_len: usize,
    compressed: bool,
}

fn extract_doc_text_piece_table(word: &[u8], table: &[u8]) -> Option<String> {
    let pieces = find_piece_table(table)?;
    let mut out = String::new();
    for piece in pieces {
        if piece.cp_len == 0 {
            continue;
        }
        if piece.compressed {
            let end = piece.offset.saturating_add(piece.cp_len);
            if end > word.len() {
                continue;
            }
            let (decoded, _, _) = WINDOWS_1252.decode(&word[piece.offset..end]);
            out.push_str(&decoded);
        } else {
            let byte_len = piece.cp_len.saturating_mul(2);
            let end = piece.offset.saturating_add(byte_len);
            if end > word.len() {
                continue;
            }
            let mut utf16 = Vec::with_capacity(byte_len / 2);
            for chunk in word[piece.offset..end].as_chunks::<2>().0 {
                utf16.push(u16::from_le_bytes([chunk[0], chunk[1]]));
            }
            out.push_str(&String::from_utf16_lossy(&utf16));
        }
    }
    if out.is_empty() {
        return None;
    }
    Some(out.replace('\r', "\n"))
}

fn find_piece_table(table: &[u8]) -> Option<Vec<DocPiece>> {
    let mut best: Option<Vec<DocPiece>> = None;
    let mut i = 0usize;
    while i + 5 <= table.len() {
        if table[i] != 0x02 {
            i += 1;
            continue;
        }
        let lcb = table
            .get(i + 1..i + 5)
            .and_then(|slice| slice.try_into().ok())
            .map(u32::from_le_bytes)? as usize;
        let start = i + 5;
        let end = start.saturating_add(lcb);
        if lcb < 4 || end > table.len() {
            i += 1;
            continue;
        }
        if let Some(pieces) = parse_plc_pcd(&table[start..end])
            && best
                .as_ref()
                .map(|b| pieces.len() > b.len())
                .unwrap_or(true)
        {
            best = Some(pieces);
        }
        i += 1;
    }
    best
}

fn parse_plc_pcd(data: &[u8]) -> Option<Vec<DocPiece>> {
    if data.len() < 4 {
        return None;
    }
    let remaining = data.len().saturating_sub(4);
    if !remaining.is_multiple_of(12) {
        return None;
    }
    let piece_count = remaining / 12;
    if piece_count == 0 {
        return None;
    }
    let cp_count = piece_count + 1;
    let mut cps = Vec::with_capacity(cp_count);
    for idx in 0..cp_count {
        let value = data
            .get(idx * 4..idx * 4 + 4)
            .and_then(|slice| slice.try_into().ok())
            .map(u32::from_le_bytes)?;
        cps.push(value);
    }
    if cps.windows(2).any(|w| w[1] < w[0]) {
        return None;
    }
    let mut pieces = Vec::with_capacity(piece_count);
    let pcd_start = cp_count * 4;
    for idx in 0..piece_count {
        let off = pcd_start + idx * 8;
        if off + 8 > data.len() {
            return None;
        }
        let fc_raw = data
            .get(off + 2..off + 6)
            .and_then(|slice| slice.try_into().ok())
            .map(u32::from_le_bytes)?;
        let compressed = (fc_raw & 1) == 1;
        let fc = fc_raw & 0xFFFFFFFE;
        let offset = if compressed {
            (fc as usize) / 2
        } else {
            fc as usize
        };
        pieces.push(DocPiece {
            offset,
            cp_len: (cps[idx + 1].saturating_sub(cps[idx])) as usize,
            compressed,
        });
    }
    Some(pieces)
}

fn clean_doc_text(text: String) -> String {
    let mut cleaned = String::new();
    for line in text.lines() {
        let trimmed = line.trim_matches(|c: char| c.is_whitespace() || c.is_control());
        if trimmed.is_empty() || is_likely_garbage(trimmed) || trimmed.contains("11252") {
            continue;
        }
        cleaned.push_str(line);
        cleaned.push('\n');
    }
    cleaned
}

fn extract_utf16_strings(buffer: &[u8]) -> String {
    let mut text = String::new();
    let mut current_seq = Vec::new();
    for chunk in buffer.as_chunks::<2>().0 {
        let unit = u16::from_le_bytes([chunk[0], chunk[1]]);
        if (unit >= 32 && unit != 0xFFFF) || unit == 10 || unit == 13 || unit == 9 {
            current_seq.push(unit);
            if current_seq.len() > 10000 {
                let s = String::from_utf16_lossy(&current_seq);
                if !is_likely_garbage(&s) {
                    text.push_str(&s);
                    text.push('\n');
                }
                current_seq.clear();
            }
        } else {
            if current_seq.len() > 5 {
                let s = String::from_utf16_lossy(&current_seq);
                if !is_likely_garbage(&s) {
                    text.push_str(&s);
                    text.push('\n');
                }
            }
            current_seq.clear();
        }
    }
    if current_seq.len() > 5 {
        let s = String::from_utf16_lossy(&current_seq);
        if !is_likely_garbage(&s) {
            text.push_str(&s);
        }
    }
    text
}

fn extract_ascii_strings(buffer: &[u8]) -> String {
    let mut text = String::new();
    let mut current_seq = Vec::new();
    for &byte in buffer {
        if (32..=126).contains(&byte) || byte == 10 || byte == 13 || byte == 9 {
            current_seq.push(byte);
            if current_seq.len() > 10000 {
                if let Ok(s) = String::from_utf8(current_seq.clone())
                    && !is_likely_garbage(&s)
                {
                    text.push_str(&s);
                    text.push('\n');
                }
                current_seq.clear();
            }
        } else {
            if current_seq.len() > 5
                && let Ok(s) = String::from_utf8(current_seq.clone())
                && !is_likely_garbage(&s)
            {
                text.push_str(&s);
                text.push('\n');
            }
            current_seq.clear();
        }
    }
    text
}

fn is_likely_garbage(s: &str) -> bool {
    let trimmed = s.trim_matches(|c: char| c.is_whitespace() || c.is_control());
    if s.contains("1125211")
        || s.contains("11252")
        || s.contains("Arial;")
        || s.contains("Times New Roman;")
        || s.contains("Courier New;")
    {
        return true;
    }
    if trimmed.starts_with('*') && trimmed.chars().nth(1).is_some_and(|c| c.is_ascii_digit()) {
        return true;
    }
    if s.contains("|") && trimmed.chars().take(5).all(|c| c.is_ascii_digit()) {
        return true;
    }
    if s.contains("'01") || s.contains("'02") || s.contains("'03") {
        return true;
    }
    let letter_count = s.chars().filter(|c| c.is_alphabetic()).count();
    let digit_count = s.chars().filter(|c| c.is_ascii_digit()).count();
    let symbol_count = s
        .chars()
        .filter(|c| !c.is_alphanumeric() && !c.is_whitespace())
        .count();
    if letter_count == 0 {
        return true;
    }
    if (digit_count + symbol_count) * 2 > letter_count {
        return true;
    }
    let mut max_digit_run = 0;
    let mut current_digit_run = 0;
    for c in s.chars() {
        if c.is_ascii_digit() {
            current_digit_run += 1;
        } else {
            max_digit_run = max_digit_run.max(current_digit_run);
            current_digit_run = 0;
        }
    }
    max_digit_run = max_digit_run.max(current_digit_run);
    if max_digit_run > 4 {
        return true;
    }
    false
}

// --- RTF Parsing ---

pub fn extract_rtf_text(bytes: &[u8]) -> String {
    fn is_skip_destination(keyword: &str) -> bool {
        matches!(
            keyword,
            "fonttbl"
                | "colortbl"
                | "stylesheet"
                | "info"
                | "pict"
                | "object"
                | "filetbl"
                | "datastore"
                | "themedata"
                | "header"
                | "headerl"
                | "headerr"
                | "headerf"
                | "footer"
                | "footerl"
                | "footerr"
                | "footerf"
                | "generator"
                | "xmlopen"
                | "xmlattrname"
                | "xmlattrvalue"
        )
    }
    fn hex_val(b: u8) -> Option<u8> {
        match b {
            b'0'..=b'9' => Some(b - b'0'),
            b'a'..=b'f' => Some(b - b'a' + 10),
            b'A'..=b'F' => Some(b - b'A' + 10),
            _ => None,
        }
    }
    fn emit_char(out: &mut String, skip_output: &mut usize, in_skip: bool, ch: char) {
        if *skip_output > 0 {
            *skip_output -= 1;
            return;
        }
        if in_skip {
            return;
        }
        match ch {
            '\r' | '\0' => {}
            '\n' => out.push('\n'),
            _ => out.push(ch),
        }
    }
    fn emit_str(out: &mut String, skip_output: &mut usize, in_skip: bool, s: &str) {
        for ch in s.chars() {
            emit_char(out, skip_output, in_skip, ch);
        }
    }
    fn encoding_from_codepage(codepage: i32) -> Option<&'static Encoding> {
        let label = if codepage == 65001 {
            "utf-8".to_string()
        } else {
            format!("windows-{}", codepage)
        };
        Encoding::for_label(label.as_bytes())
    }
    let mut out = String::new();
    let mut i = 0usize;
    let mut group_stack = vec![false];
    let mut uc_skip = 1usize;
    let mut skip_output = 0usize;
    let mut encoding: &'static Encoding = WINDOWS_1252;
    while i < bytes.len() {
        match bytes[i] {
            b'{' => {
                group_stack.push(*group_stack.last().unwrap_or(&false));
                i += 1;
            }
            b'}' => {
                if group_stack.len() > 1 {
                    group_stack.pop();
                }
                i += 1;
            }
            b'\\' => {
                i += 1;
                if i >= bytes.len() {
                    break;
                }
                match bytes[i] {
                    b'\\' | b'{' | b'}' => {
                        emit_char(
                            &mut out,
                            &mut skip_output,
                            *group_stack.last().unwrap_or(&false),
                            bytes[i] as char,
                        );
                        i += 1;
                    }
                    b'~' => {
                        emit_char(
                            &mut out,
                            &mut skip_output,
                            *group_stack.last().unwrap_or(&false),
                            ' ',
                        );
                        i += 1;
                    }
                    b'-' | b'_' => {
                        emit_char(
                            &mut out,
                            &mut skip_output,
                            *group_stack.last().unwrap_or(&false),
                            '-',
                        );
                        i += 1;
                    }
                    b'*' => {
                        if let Some(last) = group_stack.last_mut() {
                            *last = true;
                        }
                        i += 1;
                    }
                    b'\'' if i + 2 < bytes.len() => {
                        let h1 = bytes[i + 1];
                        let h2 = bytes[i + 2];
                        if let (Some(n1), Some(n2)) = (hex_val(h1), hex_val(h2)) {
                            let byte = (n1 << 4) | n2;
                            let buf = [byte];
                            let (decoded, _, _) = encoding.decode(&buf);
                            emit_str(
                                &mut out,
                                &mut skip_output,
                                *group_stack.last().unwrap_or(&false),
                                &decoded,
                            );
                            i += 3;
                        } else {
                            i += 1;
                        }
                    }
                    b'\'' => {
                        i += 1;
                    }
                    b if b.is_ascii_alphabetic() => {
                        let start = i;
                        while i < bytes.len() && bytes[i].is_ascii_alphabetic() {
                            i += 1;
                        }
                        let keyword = std::str::from_utf8(&bytes[start..i]).unwrap_or("");
                        let mut sign = 1i32;
                        if i < bytes.len() && bytes[i] == b'-' {
                            sign = -1;
                            i += 1;
                        }
                        let mut value = 0i32;
                        let mut has_digit = false;
                        while i < bytes.len() && bytes[i].is_ascii_digit() {
                            has_digit = true;
                            value = value * 10 + (bytes[i] - b'0') as i32;
                            i += 1;
                        }
                        let num = if has_digit { Some(value * sign) } else { None };
                        if i < bytes.len() && bytes[i] == b' ' {
                            i += 1;
                        }
                        match keyword {
                            "par" | "line" => emit_char(
                                &mut out,
                                &mut skip_output,
                                *group_stack.last().unwrap_or(&false),
                                '\n',
                            ),
                            "tab" => emit_char(
                                &mut out,
                                &mut skip_output,
                                *group_stack.last().unwrap_or(&false),
                                '\t',
                            ),
                            "emdash" => emit_str(
                                &mut out,
                                &mut skip_output,
                                *group_stack.last().unwrap_or(&false),
                                "--",
                            ),
                            "endash" => emit_char(
                                &mut out,
                                &mut skip_output,
                                *group_stack.last().unwrap_or(&false),
                                '-',
                            ),
                            "bullet" => emit_char(
                                &mut out,
                                &mut skip_output,
                                *group_stack.last().unwrap_or(&false),
                                '*',
                            ),
                            "u" => {
                                if let Some(n) = num {
                                    let mut code = n;
                                    if code < 0 {
                                        code += 65536;
                                    }
                                    if let Some(ch) = char::from_u32(code as u32) {
                                        emit_char(
                                            &mut out,
                                            &mut skip_output,
                                            *group_stack.last().unwrap_or(&false),
                                            ch,
                                        );
                                    }
                                    skip_output = uc_skip;
                                }
                            }
                            "uc" => {
                                if let Some(n) = num
                                    && n >= 0
                                {
                                    uc_skip = n as usize;
                                }
                            }
                            "ansicpg" => {
                                if let Some(n) = num
                                    && let Some(enc) = encoding_from_codepage(n)
                                {
                                    encoding = enc;
                                }
                            }
                            _ => {
                                if is_skip_destination(keyword)
                                    && let Some(last) = group_stack.last_mut()
                                {
                                    *last = true;
                                }
                            }
                        }
                    }
                    _ => {
                        i += 1;
                    }
                }
            }
            b'\r' | b'\n' => {
                i += 1;
            }
            b => {
                if b >= 0x80 {
                    let buf = [b];
                    let (decoded, _, _) = encoding.decode(&buf);
                    emit_str(
                        &mut out,
                        &mut skip_output,
                        *group_stack.last().unwrap_or(&false),
                        &decoded,
                    );
                } else {
                    emit_char(
                        &mut out,
                        &mut skip_output,
                        *group_stack.last().unwrap_or(&false),
                        b as char,
                    );
                }
                i += 1;
            }
        }
    }
    out
}

// --- Spreadsheet Parsing ---

pub fn read_spreadsheet_text(path: &Path, language: Language) -> Result<String, String> {
    let mut workbook = open_workbook_auto(path).map_err(|err| {
        i18n::tr_f(
            language,
            "file_handler.excel_open_error",
            &[("err", &err.to_string())],
        )
    })?;
    let mut out = String::new();
    if let Some(Ok(range)) = workbook.worksheet_range_at(0) {
        for row in range.rows() {
            let mut first = true;
            for cell in row {
                if !first {
                    out.push('\t');
                }
                first = false;
                match cell {
                    CalamineData::Empty => {}
                    CalamineData::String(s) => out.push_str(s),
                    CalamineData::Float(f) => out.push_str(&f.to_string()),
                    CalamineData::Int(i) => out.push_str(&i.to_string()),
                    CalamineData::Bool(b) => out.push_str(&b.to_string()),
                    CalamineData::Error(e) => out.push_str(&format!("{:?}", e)),
                    CalamineData::DateTime(f) => out.push_str(&f.to_string()),
                    CalamineData::DateTimeIso(s) | CalamineData::DurationIso(s) => out.push_str(s),
                }
            }
            out.push('\n');
        }
    } else {
        return Err(i18n::tr(language, "file_handler.excel_no_sheet"));
    }
    Ok(out)
}

// --- DOCX Parsing & Writing ---

pub fn read_docx_text(path: &Path, language: Language) -> Result<String, String> {
    let bytes = std::fs::read(path).map_err(|err| {
        i18n::tr_f(
            language,
            "file_handler.file_open_error",
            &[("err", &err.to_string())],
        )
    })?;
    let docx = read_docx(&bytes).map_err(|err| {
        i18n::tr_f(
            language,
            "file_handler.docx_read_error",
            &[("err", &err.to_string())],
        )
    })?;
    Ok(extract_docx_text(&docx))
}

fn extract_docx_text(docx: &Docx) -> String {
    let mut out = String::new();
    for child in &docx.document.children {
        append_document_child_text(&mut out, child);
    }
    if out.ends_with('\n') {
        out.pop();
    }
    out
}

fn append_document_child_text(out: &mut String, child: &DocumentChild) {
    match child {
        DocumentChild::Paragraph(p) => {
            append_paragraph_text(out, p);
            out.push('\n');
        }
        DocumentChild::Table(t) => {
            append_table_text(out, t);
        }
        _ => {}
    }
}
fn append_paragraph_text(out: &mut String, paragraph: &Paragraph) {
    for child in &paragraph.children {
        append_paragraph_child_text(out, child);
    }
}
fn append_paragraph_child_text(out: &mut String, child: &ParagraphChild) {
    match child {
        ParagraphChild::Run(run) => {
            append_run_text(out, run);
        }
        ParagraphChild::Hyperlink(link) => {
            for child in &link.children {
                append_paragraph_child_text(out, child);
            }
        }
        _ => {}
    }
}
fn append_run_text(out: &mut String, run: &Run) {
    for child in &run.children {
        match child {
            RunChild::Text(t) => {
                out.push_str(&t.text);
            }
            RunChild::Tab(_) => {
                out.push('\t');
            }
            _ => {}
        }
    }
}
fn append_table_text(out: &mut String, table: &Table) {
    for row in &table.rows {
        let docx_rs::TableChild::TableRow(row) = row;
        let mut first_cell = true;
        for cell in &row.cells {
            let docx_rs::TableRowChild::TableCell(cell) = cell;
            if !first_cell {
                out.push('\t');
            }
            first_cell = false;
            let cell_text = extract_table_cell_text(cell);
            out.push_str(&cell_text);
        }
        out.push('\n');
    }
}

fn extract_table_cell_text(cell: &docx_rs::TableCell) -> String {
    let mut out = String::new();
    for content in &cell.children {
        match content {
            TableCellContent::Paragraph(p) => {
                append_paragraph_text(&mut out, p);
                out.push('\n');
            }
            TableCellContent::Table(t) => {
                append_table_text(&mut out, t);
            }
            _ => {}
        }
    }
    if out.ends_with('\n') {
        out.pop();
    }
    out
}

pub fn write_docx_text(path: &Path, text: &str, language: Language) -> Result<(), String> {
    let file = std::fs::File::create(path).map_err(|err| {
        i18n::tr_f(
            language,
            "file_handler.file_save_error",
            &[("err", &err.to_string())],
        )
    })?;
    let mut docx = Docx::new();
    for line in text.split('\n') {
        let line = line.strip_suffix('\r').unwrap_or(line);
        let paragraph = if line.is_empty() {
            Paragraph::new()
        } else {
            Paragraph::new().add_run(Run::new().add_text(line))
        };
        docx = docx.add_paragraph(paragraph);
    }
    docx.build().pack(file).map_err(|err| {
        i18n::tr_f(
            language,
            "file_handler.docx_save_error",
            &[("err", &err.to_string())],
        )
    })?;
    Ok(())
}

// --- PDF Parsing ---

pub enum PdfTextResult {
    Text(String),
    NoText,
    PasswordRequired { incorrect: bool },
}

const PDF_PASSWORD_REQUIRED: &str = "PDF_PASSWORD_REQUIRED";

const PDF_FORM_SECTION_TITLE: &str = "=== SONARPAD PDF FORM FIELDS ===";
const PDF_FORM_SECTION_BEGIN: &str = "\n\n=== SONARPAD PDF FORM FIELDS ===\n";
const PDF_FORM_SECTION_END: &str = "=== END SONARPAD PDF FORM FIELDS ===\n";
const PDF_FIELD_BEGIN_PREFIX: &str = "[[PDF_FIELD:";
const PDF_FIELD_BEGIN_SUFFIX: &str = "]]";
const PDF_FIELD_END_PREFIX: &str = "[[/PDF_FIELD:";
const PDF_FIELD_END_SUFFIX: &str = "]]";

#[derive(Debug, Clone)]
struct PdfFormField {
    name: String,
    field_type: String,
    value: String,
}

static PDF_EXTRACT_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

pub fn read_pdf_text_with_status(path: &Path, language: Language) -> Result<PdfTextResult, String> {
    read_pdf_text_with_password(path, language, None)
}

pub fn read_pdf_text_with_password(
    path: &Path,
    language: Language,
    password: Option<&str>,
) -> Result<PdfTextResult, String> {
    let start = std::time::Instant::now();
    let lock = PDF_EXTRACT_LOCK.get_or_init(|| Mutex::new(()));
    let guard = match lock.lock() {
        Ok(guard) => guard,
        Err(poisoned) => {
            crate::log_debug("PDF: Extraction lock poisoned; continuing.");
            poisoned.into_inner()
        }
    };
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        crate::sentry_integration::with_suppressed_panic_reporting(|| {
            let _guard = guard;
            crate::log_debug(&format!("PDF: Starting extraction for {:?}", path));
            let text = if password.is_some() {
                extract_pdf_text_pdfium_with_password(path, password)?
            } else {
                extract_pdf_text_with_fallback(path, language)?
            };
            crate::log_debug(&format!(
                "PDF: Raw extraction completed in {:?}, length={}",
                start.elapsed(),
                text.len()
            ));

            let norm_start = std::time::Instant::now();
            let normalized = if text.trim().is_empty() {
                crate::log_debug("PDF: No text extracted; checking form fields.");
                String::new()
            } else {
                normalize_pdf_paragraphs(&text)
            };
            let normalized = append_pdf_form_fields_if_any(path, normalized, language, password);
            let normalized = strip_embedded_nuls(normalized, "normalized PDF output");
            crate::log_debug(&format!(
                "PDF: Normalization completed in {:?}, final length={}",
                norm_start.elapsed(),
                normalized.len()
            ));

            if normalized.trim().is_empty() {
                Ok(PdfTextResult::NoText)
            } else {
                Ok(PdfTextResult::Text(normalized))
            }
        })
    }));

    match result {
        Ok(Err(error)) if error == PDF_PASSWORD_REQUIRED => Ok(PdfTextResult::PasswordRequired {
            incorrect: password.is_some(),
        }),
        Ok(result) => result,
        Err(panic) => {
            let panic_msg = panic_payload_to_string(&panic);
            crate::log_debug(&format!("PDF: Panic during extraction: {}", panic_msg));
            match extract_pdf_text_pdfium_with_password(path, password) {
                Ok(text) => {
                    crate::log_debug("PDF: pdfium extraction succeeded after panic.");
                    let normalized = if text.trim().is_empty() {
                        String::new()
                    } else {
                        normalize_pdf_paragraphs(&text)
                    };
                    let normalized =
                        append_pdf_form_fields_if_any(path, normalized, language, password);
                    let normalized =
                        strip_embedded_nuls(normalized, "PDFium panic fallback output");
                    if normalized.trim().is_empty() {
                        Ok(PdfTextResult::NoText)
                    } else {
                        Ok(PdfTextResult::Text(normalized))
                    }
                }
                Err(pdfium_err) if pdfium_err == PDF_PASSWORD_REQUIRED => {
                    Ok(PdfTextResult::PasswordRequired {
                        incorrect: password.is_some(),
                    })
                }
                Err(pdfium_err) => {
                    crate::log_debug(&format!(
                        "PDF: pdfium extraction failed after panic: {}",
                        pdfium_err
                    ));
                    Err(i18n::tr_f(
                        language,
                        "file_handler.pdf_read_error",
                        &[("err", "PDF extraction crashed unexpectedly")],
                    ))
                }
            }
        }
    }
}

pub fn read_pdf_text(path: &Path, language: Language) -> Result<String, String> {
    match read_pdf_text_with_status(path, language)? {
        PdfTextResult::Text(text) => Ok(text),
        PdfTextResult::NoText => Ok(i18n::tr(language, "file_handler.pdf_no_text")),
        PdfTextResult::PasswordRequired { .. } => {
            Err(i18n::tr(language, "app.pdf_password_prompt"))
        }
    }
}

fn append_pdf_form_fields_if_any(
    path: &Path,
    mut text: String,
    language: Language,
    password: Option<&str>,
) -> String {
    match read_pdf_form_fields(path, password) {
        Ok(fields) if !fields.is_empty() => {
            crate::log_debug(&format!(
                "PDF form: found {} AcroForm field(s) in {}",
                fields.len(),
                path.display()
            ));
            text.push_str(PDF_FORM_SECTION_BEGIN);
            text.push_str(pdf_form_instruction(language));
            for field in fields {
                text.push_str(&format!(
                    "Campo: {} ({})
{}{}{}
{}
{}{}{}

",
                    field.name,
                    field.field_type,
                    PDF_FIELD_BEGIN_PREFIX,
                    field.name,
                    PDF_FIELD_BEGIN_SUFFIX,
                    field.value,
                    PDF_FIELD_END_PREFIX,
                    field.name,
                    PDF_FIELD_END_SUFFIX
                ));
            }
            text.push_str(PDF_FORM_SECTION_END);
            text
        }
        Ok(_) => text,
        Err(err) => {
            crate::log_debug(&format!("PDF form: unable to read form fields: {err}"));
            text
        }
    }
}

fn pdf_form_instruction(language: Language) -> &'static str {
    match language {
        Language::Italian => {
            "Compila i valori tra i marcatori PDF_FIELD e poi salva il PDF. Non modificare i nomi dei campi tra parentesi quadre.\n\n"
        }
        _ => {
            "Fill in the values between the PDF_FIELD markers, then save the PDF. Do not change the field names inside the square brackets.\n\n"
        }
    }
}

pub fn text_has_pdf_form_values(text: &str) -> bool {
    text.contains(PDF_FORM_SECTION_TITLE) && text.contains(PDF_FIELD_BEGIN_PREFIX)
}

pub fn write_pdf_form_values_from_text(
    source_path: &Path,
    output_path: &Path,
    text: &str,
    language: Language,
) -> Result<bool, String> {
    let values = parse_pdf_form_values(text);
    if values.is_empty() {
        return Ok(false);
    }
    fill_pdf_form_fields(source_path, output_path, &values, language)
}

fn parse_pdf_form_values(text: &str) -> Vec<(String, String)> {
    let Some(section_title_start) = text.find(PDF_FORM_SECTION_TITLE) else {
        return Vec::new();
    };
    let after_title = &text[section_title_start + PDF_FORM_SECTION_TITLE.len()..];
    let section = after_title
        .strip_prefix("\r\n")
        .or_else(|| after_title.strip_prefix('\n'))
        .unwrap_or(after_title);
    let section = match section.find(PDF_FORM_SECTION_END) {
        Some(end) => &section[..end],
        None => section,
    };

    let mut values = Vec::new();
    let mut rest = section;
    while let Some(begin_pos) = rest.find(PDF_FIELD_BEGIN_PREFIX) {
        let after_begin_prefix = &rest[begin_pos + PDF_FIELD_BEGIN_PREFIX.len()..];
        let Some(name_end) = after_begin_prefix.find(PDF_FIELD_BEGIN_SUFFIX) else {
            break;
        };
        let name = after_begin_prefix[..name_end].trim().to_string();
        let next_start = name_end + PDF_FIELD_BEGIN_SUFFIX.len();
        if name.is_empty() {
            rest = &after_begin_prefix[next_start..];
            continue;
        }
        let after_begin = &after_begin_prefix[next_start..];
        let end_marker = format!("{}{}{}", PDF_FIELD_END_PREFIX, name, PDF_FIELD_END_SUFFIX);
        let Some(value_end) = after_begin.find(&end_marker) else {
            break;
        };
        let value = after_begin[..value_end]
            .trim_matches(|ch| ch == '\r' || ch == '\n')
            .to_string();
        values.push((name, value));
        rest = &after_begin[value_end + end_marker.len()..];
    }
    values
}

fn fill_pdf_form_fields(
    source_path: &Path,
    output_path: &Path,
    values: &[(String, String)],
    language: Language,
) -> Result<bool, String> {
    if values.is_empty() || !source_path.exists() {
        return Ok(false);
    }

    let mut doc = LoDocument::load(source_path).map_err(|err| {
        i18n::tr_f(
            language,
            "file_handler.file_save_error",
            &[("err", &format!("PDF form load failed: {err}"))],
        )
    })?;

    let acro_form_obj = {
        let catalog = doc.catalog().map_err(|err| {
            i18n::tr_f(
                language,
                "file_handler.file_save_error",
                &[("err", &format!("PDF form catalog failed: {err}"))],
            )
        })?;
        match catalog.get(b"AcroForm") {
            Ok(obj) => obj.clone(),
            Err(_) => return Ok(false),
        }
    };
    let acro_form_id = match acro_form_obj {
        LoObject::Reference(id) => id,
        _ => return Ok(false),
    };

    if let Ok(acro_form_dict) = doc.get_dictionary_mut(acro_form_id) {
        acro_form_dict.set("NeedAppearances", LoObject::Boolean(true));
    }

    let fields = match doc
        .get_dictionary(acro_form_id)
        .ok()
        .and_then(|dict| dict.get(b"Fields").ok())
    {
        Some(LoObject::Array(fields)) => fields.clone(),
        _ => return Ok(false),
    };

    let mut changed = 0usize;
    for field in fields {
        changed += fill_pdf_form_field_recursive(&mut doc, &field, None, values);
    }

    if changed == 0 {
        return Ok(false);
    }

    save_pdf_document_atomically(&mut doc, output_path, language)?;
    crate::log_debug(&format!(
        "PDF form: saved {changed} field(s) to {}",
        output_path.display()
    ));
    Ok(true)
}

fn fill_pdf_form_field_recursive(
    doc: &mut LoDocument,
    obj: &LoObject,
    parent_name: Option<&str>,
    values: &[(String, String)],
) -> usize {
    let field_id = match obj {
        LoObject::Reference(id) => Some(*id),
        _ => None,
    };
    let dict_clone = match field_id {
        Some(id) => match doc.get_dictionary(id) {
            Ok(dict) => dict.clone(),
            Err(_) => return 0,
        },
        None => match obj {
            LoObject::Dictionary(dict) => dict.clone(),
            _ => return 0,
        },
    };

    let own_name = pdf_dict_text(&dict_clone, b"T");
    let full_name = match (parent_name, own_name.as_deref()) {
        (Some(parent), Some(name)) if !parent.is_empty() && !name.is_empty() => {
            Some(format!("{parent}.{name}"))
        }
        (_, Some(name)) if !name.is_empty() => Some(name.to_string()),
        (Some(parent), _) if !parent.is_empty() => Some(parent.to_string()),
        _ => None,
    };

    let kids = match dict_clone.get(b"Kids") {
        Ok(LoObject::Array(kids)) => kids.clone(),
        _ => Vec::new(),
    };

    let mut changed = 0usize;
    for kid in kids {
        changed += fill_pdf_form_field_recursive(doc, &kid, full_name.as_deref(), values);
    }

    let Some(name) = full_name else {
        return changed;
    };
    let Some((_, value)) = values.iter().find(|(field_name, _)| field_name == &name) else {
        return changed;
    };

    let value_obj = LoObject::String(value.as_bytes().to_vec(), StringFormat::Literal);
    if let Some(id) = field_id
        && let Ok(dict) = doc.get_dictionary_mut(id)
    {
        dict.set("V", value_obj.clone());
        dict.set("DV", value_obj);
        dict.remove(b"AP");
        changed += 1;
    }
    changed
}

pub fn save_pdf_without_password(
    source: &Path,
    output: &Path,
    password: &str,
    language: Language,
) -> Result<(), String> {
    let source_path = source.canonicalize().map_err(|e| e.to_string())?;
    if output.canonicalize().is_ok_and(|path| path == source_path) {
        return Err(i18n::tr(language, "app.pdf_copy_different_path"));
    }
    // Use the same PDFium engine that successfully authenticated the original.
    // Saving with FPDF_REMOVE_SECURITY retains PDF objects; it does not render or
    // recreate pages from the extracted editor text.
    use pdfium_render::prelude::FPDF_FILEWRITE;
    #[repr(C)]
    struct PdfCopyWriter {
        interface: FPDF_FILEWRITE,
        data: Vec<u8>,
    }
    unsafe extern "C" fn write_block(
        this: *mut FPDF_FILEWRITE,
        data: *const std::ffi::c_void,
        size: std::os::raw::c_ulong,
    ) -> std::os::raw::c_int {
        // PDFium passes the address of our first repr(C) field synchronously.
        let writer = unsafe { &mut *this.cast::<PdfCopyWriter>() };
        let bytes = unsafe { std::slice::from_raw_parts(data.cast::<u8>(), size as usize) };
        writer.data.extend_from_slice(bytes);
        1
    }
    let bindings = Pdfium::bind_to_library(Pdfium::pdfium_platform_library_name_at_path(
        &crate::settings::settings_dir(),
    ))
    .or_else(|_| Pdfium::bind_to_system_library())
    .map_err(|error| error.to_string())?;
    let pdfium = Pdfium::new(bindings);
    let bytes = std::fs::read(source).map_err(|error| error.to_string())?;
    let document = pdfium
        .bindings()
        .FPDF_LoadMemDocument64(&bytes, Some(password));
    if document.is_null() {
        return Err(i18n::tr(language, "app.pdf_password_incorrect"));
    }
    let mut writer = PdfCopyWriter {
        interface: FPDF_FILEWRITE {
            version: 1,
            WriteBlock: Some(write_block),
        },
        data: Vec::new(),
    };
    const FPDF_REMOVE_SECURITY: u32 = 3;
    let saved =
        pdfium
            .bindings()
            .FPDF_SaveAsCopy(document, &mut writer.interface, FPDF_REMOVE_SECURITY);
    pdfium.bindings().FPDF_CloseDocument(document);
    if saved == 0 {
        return Err(i18n::tr_f(
            language,
            "file_handler.file_save_error",
            &[("err", "PDFium save failed")],
        ));
    }
    // Validate before replacing any destination file.
    let verified = pdfium
        .load_pdf_from_byte_slice(&writer.data, None)
        .map_err(|error| error.to_string())?;
    if verified.pages().is_empty() {
        return Err(i18n::tr_f(
            language,
            "file_handler.file_save_error",
            &[("err", "PDF contains no pages")],
        ));
    }
    save_pdf_atomically(output, language, |file| {
        std::io::Write::write_all(file, &writer.data)
    })
}

fn save_pdf_document_atomically(
    doc: &mut LoDocument,
    output_path: &Path,
    language: Language,
) -> Result<(), String> {
    save_pdf_atomically(output_path, language, |file| doc.save_to(file))
}

fn save_pdf_atomically(
    output_path: &Path,
    language: Language,
    write: impl FnOnce(&mut std::fs::File) -> std::io::Result<()>,
) -> Result<(), String> {
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let tmp_path = output_path.with_extension(format!("pdf.{}.{nonce}.tmp", std::process::id()));
    let mut created = false;
    let result = (|| -> std::io::Result<()> {
        {
            let mut file = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&tmp_path)?;
            created = true;
            write(&mut file)?;
            file.sync_all()?;
        }
        // Replacing the directory entry also avoids writing through a hard link.
        std::fs::rename(&tmp_path, output_path)
    })();
    if created && tmp_path.exists() {
        crate::log_if_err!(std::fs::remove_file(&tmp_path));
    }
    result.map_err(|err| {
        i18n::tr_f(
            language,
            "file_handler.file_save_error",
            &[("err", &err.to_string())],
        )
    })
}

fn read_pdf_form_fields(path: &Path, password: Option<&str>) -> Result<Vec<PdfFormField>, String> {
    let mut doc = LoDocument::load(path).map_err(|err| format!("PDF form load failed: {err}"))?;
    if doc.is_encrypted() {
        doc.decrypt(password.unwrap_or(""))
            .map_err(|_| "PDF form authentication failed".to_string())?;
    }
    let catalog = doc
        .catalog()
        .map_err(|err| format!("PDF form catalog failed: {err}"))?;
    let acro_form = match catalog.get(b"AcroForm") {
        Ok(obj) => obj,
        Err(_) => return Ok(Vec::new()),
    };
    let Some(acro_dict) = resolve_pdf_dict(&doc, acro_form) else {
        return Ok(Vec::new());
    };
    let fields_obj = match acro_dict.get(b"Fields") {
        Ok(obj) => obj,
        Err(_) => return Ok(Vec::new()),
    };
    let LoObject::Array(fields) = fields_obj else {
        return Ok(Vec::new());
    };

    let mut output = Vec::new();
    for field in fields {
        collect_pdf_form_fields(&doc, field, None, None, &mut output);
    }
    Ok(output)
}

fn collect_pdf_form_fields(
    doc: &LoDocument,
    obj: &LoObject,
    parent_name: Option<&str>,
    inherited_type: Option<&str>,
    output: &mut Vec<PdfFormField>,
) {
    let Some(dict) = resolve_pdf_dict(doc, obj) else {
        return;
    };

    let own_name = pdf_dict_text(dict, b"T");
    let full_name = match (parent_name, own_name.as_deref()) {
        (Some(parent), Some(name)) if !parent.is_empty() && !name.is_empty() => {
            Some(format!("{parent}.{name}"))
        }
        (_, Some(name)) if !name.is_empty() => Some(name.to_string()),
        (Some(parent), _) if !parent.is_empty() => Some(parent.to_string()),
        _ => None,
    };

    let own_type = pdf_dict_name(dict, b"FT");
    let field_type = own_type.as_deref().or(inherited_type);

    if let Ok(LoObject::Array(kids)) = dict.get(b"Kids") {
        for kid in kids {
            collect_pdf_form_fields(doc, kid, full_name.as_deref(), field_type, output);
        }
    }

    let Some(name) = full_name else {
        return;
    };
    let Some(field_type) = field_type else {
        return;
    };
    if field_type.eq_ignore_ascii_case("Sig") {
        return;
    }
    if output.iter().any(|field| field.name == name) {
        return;
    }

    output.push(PdfFormField {
        name,
        field_type: field_type.to_string(),
        value: pdf_dict_text(dict, b"V").unwrap_or_default(),
    });
}

fn resolve_pdf_dict<'a>(doc: &'a LoDocument, obj: &'a LoObject) -> Option<&'a LoDictionary> {
    match obj {
        LoObject::Dictionary(dict) => Some(dict),
        LoObject::Reference(id) => match doc.get_object(*id).ok()? {
            LoObject::Dictionary(dict) => Some(dict),
            _ => None,
        },
        _ => None,
    }
}

fn pdf_dict_text(dict: &LoDictionary, key: &[u8]) -> Option<String> {
    pdf_object_text(dict.get(key).ok()?)
}

fn pdf_dict_name(dict: &LoDictionary, key: &[u8]) -> Option<String> {
    match dict.get(key).ok()? {
        LoObject::Name(bytes) => Some(String::from_utf8_lossy(bytes).into_owned()),
        other => pdf_object_text(other),
    }
}

fn pdf_object_text(obj: &LoObject) -> Option<String> {
    match obj {
        LoObject::String(bytes, _) => Some(String::from_utf8_lossy(bytes).into_owned()),
        LoObject::Name(bytes) => Some(String::from_utf8_lossy(bytes).into_owned()),
        LoObject::Integer(value) => Some(value.to_string()),
        LoObject::Real(value) => Some(value.to_string()),
        LoObject::Boolean(value) => Some(value.to_string()),
        _ => None,
    }
}

fn embedded_nul_count(text: &str) -> usize {
    text.matches('\0').count()
}

fn strip_embedded_nuls(text: String, source: &str) -> String {
    let count = embedded_nul_count(&text);
    if count == 0 {
        return text;
    }

    crate::log_debug(&format!(
        "PDF: Removed {count} embedded NUL characters from {source}."
    ));
    text.replace('\0', "")
}

fn extract_pdf_text_with_fallback(path: &Path, language: Language) -> Result<String, String> {
    let extract_result =
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| extract_text(path)));
    match extract_result {
        Ok(Ok(text)) => {
            let nul_count = embedded_nul_count(&text);
            if nul_count == 0 {
                return Ok(text);
            }

            crate::log_debug(&format!(
                "PDF: pdf_extract returned {nul_count} embedded NUL characters; retrying with PDFium."
            ));
            let sanitized_pdf_extract = strip_embedded_nuls(text, "pdf_extract fallback text");
            match extract_pdf_text_pdfium(path) {
                Ok(pdfium_text) if !pdfium_text.trim().is_empty() => {
                    crate::log_debug(
                        "PDF: PDFium extraction succeeded after embedded NUL detection.",
                    );
                    Ok(pdfium_text)
                }
                Ok(_) => {
                    crate::log_debug(
                        "PDF: PDFium returned no text after embedded NUL detection; using sanitized pdf_extract text.",
                    );
                    Ok(sanitized_pdf_extract)
                }
                Err(pdfium_err) if pdfium_err == PDF_PASSWORD_REQUIRED => Err(pdfium_err),
                Err(pdfium_err) => {
                    crate::log_debug(&format!(
                        "PDF: PDFium fallback after embedded NUL detection failed: {pdfium_err}; using sanitized pdf_extract text."
                    ));
                    Ok(sanitized_pdf_extract)
                }
            }
        }
        Ok(Err(err)) => {
            let err_str = err.to_string();
            crate::log_debug(&format!("PDF: pdf_extract failed: {}", err_str));
            match extract_pdf_text_pdfium(path) {
                Ok(text) => {
                    crate::log_debug("PDF: pdfium extraction succeeded after pdf_extract failure.");
                    Ok(text)
                }
                Err(pdfium_err) if pdfium_err == PDF_PASSWORD_REQUIRED => Err(pdfium_err),
                Err(pdfium_err) => {
                    crate::log_debug(&format!("PDF: pdfium extraction failed: {}", pdfium_err));
                    if is_pdf_parse_error(&err_str) {
                        Err(i18n::tr(language, "file_handler.pdf_parse_error"))
                    } else {
                        Err(i18n::tr_f(
                            language,
                            "file_handler.pdf_read_error",
                            &[("err", &err_str)],
                        ))
                    }
                }
            }
        }
        Err(panic) => {
            let panic_msg = panic_payload_to_string(&panic);
            crate::log_debug(&format!("PDF: pdf_extract panicked: {}", panic_msg));
            match extract_pdf_text_pdfium(path) {
                Ok(text) => {
                    crate::log_debug("PDF: pdfium extraction succeeded after pdf_extract panic.");
                    Ok(text)
                }
                Err(pdfium_err) if pdfium_err == PDF_PASSWORD_REQUIRED => Err(pdfium_err),
                Err(pdfium_err) => {
                    crate::log_debug(&format!("PDF: pdfium extraction failed: {}", pdfium_err));
                    Err(i18n::tr_f(
                        language,
                        "file_handler.pdf_read_error",
                        &[("err", "PDF extraction crashed unexpectedly")],
                    ))
                }
            }
        }
    }
}

fn extract_pdf_text_pdfium(path: &Path) -> Result<String, String> {
    extract_pdf_text_pdfium_with_password(path, None)
}

fn extract_pdf_text_pdfium_with_password(
    path: &Path,
    password: Option<&str>,
) -> Result<String, String> {
    let deps_dir = crate::settings::settings_dir();
    let pdfium_path = Pdfium::pdfium_platform_library_name_at_path(&deps_dir);
    if !pdfium_path.exists() {
        crate::log_debug(&format!(
            "PDF: pdfium.dll not found at {}",
            pdfium_path.display()
        ));
    }
    let bindings = Pdfium::bind_to_library(pdfium_path)
        .or_else(|_| Pdfium::bind_to_system_library())
        .map_err(|err| format!("pdfium bind failed: {err}"))?;
    let pdfium = Pdfium::new(bindings);
    let document = pdfium
        .load_pdf_from_file(path, password)
        .map_err(|err| match err {
            PdfiumError::PdfiumLibraryInternalError(PdfiumInternalError::PasswordError) => {
                PDF_PASSWORD_REQUIRED.to_string()
            }
            _ => format!("pdfium load failed: {err}"),
        })?;
    let mut out = String::new();
    for page in document.pages().iter() {
        let page_text = page
            .text()
            .map_err(|err| format!("pdfium page text failed: {err}"))?;
        let text = page_text.all();
        if !text.is_empty() {
            if !out.is_empty() {
                out.push('\n');
            }
            out.push_str(&text);
        }
    }
    Ok(strip_embedded_nuls(out, "PDFium output"))
}

fn panic_payload_to_string(panic: &Box<dyn std::any::Any + Send>) -> String {
    if let Some(msg) = panic.downcast_ref::<&str>() {
        (*msg).to_string()
    } else if let Some(msg) = panic.downcast_ref::<String>() {
        msg.clone()
    } else {
        "unknown panic payload".to_string()
    }
}

fn is_pdf_parse_error(err: &str) -> bool {
    err.contains("InvalidContentStream")
        || err.contains("operation.operands.len() == 6")
        || err.contains("expect repeat at least 1 times, found 0 times")
        || err.contains("missing unicode map and encoding")
        || err.contains("bad length of hexstring")
        || err.contains("Mismatch { message:")
        || err.contains("Parse(")
}

fn normalize_pdf_paragraphs(text: &str) -> String {
    // Japanese teaching material often puts a separate expression, dialogue
    // turn or table cell on each hard line. Preserve its line boundaries
    // rather than joining distinct exercises into long paragraphs. Ordinary
    // Latin-script PDFs retain their existing paragraph normalization.
    if text.chars().any(is_japanese_pdf_character) {
        return text
            .lines()
            .map(str::trim_end)
            .collect::<Vec<_>>()
            .join("\n");
    }
    let mut out = String::new();
    let mut current = String::new();
    let avg_len = average_pdf_line_len(text);
    let mut last_line = String::new();
    for raw_line in text.lines() {
        let line = raw_line.trim();
        if line.is_empty() {
            flush_pdf_paragraph(&mut out, &mut current);
            last_line.clear();
            continue;
        }
        if current.is_empty() {
            current.push_str(line);
            last_line.clear();
            last_line.push_str(line);
            continue;
        }
        if looks_like_list_item(line) {
            flush_pdf_paragraph(&mut out, &mut current);
            current.push_str(line);
            last_line.clear();
            last_line.push_str(line);
            continue;
        }
        if should_break_pdf_paragraph(&last_line, line, avg_len) {
            flush_pdf_paragraph(&mut out, &mut current);
            current.push_str(line);
            last_line.clear();
            last_line.push_str(line);
            continue;
        }
        if last_line.ends_with('-') {
            last_line.pop();
            current.pop();
            current.push_str(line);
        } else {
            if !current.ends_with(' ') {
                current.push(' ');
            }
            current.push_str(line);
        }
        last_line.clear();
        last_line.push_str(line);
    }
    flush_pdf_paragraph(&mut out, &mut current);
    out
}

fn is_japanese_pdf_character(ch: char) -> bool {
    matches!(ch,
        '\u{3040}'..='\u{30ff}' | '\u{31f0}'..='\u{31ff}'
        | '\u{3400}'..='\u{4dbf}' | '\u{4e00}'..='\u{9fff}'
        | '\u{ff66}'..='\u{ff9f}'
    )
}

fn flush_pdf_paragraph(out: &mut String, current: &mut String) {
    if current.is_empty() {
        return;
    }
    if !out.is_empty() {
        out.push_str("\n\n");
    }
    out.push_str(current.trim());
    current.clear();
}
fn should_break_pdf_paragraph(prev: &str, next: &str, avg_len: usize) -> bool {
    if prev.is_empty() || avg_len == 0 {
        return false;
    }
    let ends_sentence = prev.ends_with('.') || prev.ends_with('!') || prev.ends_with('?');
    let starts_new = next
        .chars()
        .next()
        .map(|c| c.is_uppercase())
        .unwrap_or(false);
    if prev.len() < (avg_len * 8 / 10) && ends_sentence {
        return true;
    }
    if ends_sentence && starts_new {
        return true;
    }
    false
}

fn looks_like_list_item(line: &str) -> bool {
    let trimmed = line.trim_start();
    if trimmed.starts_with("- ") || trimmed.starts_with("* ") {
        return true;
    }
    let chars = trimmed.chars();
    let mut digits = 0usize;
    for c in chars {
        if c.is_ascii_digit() {
            digits += 1;
        } else if c == '.' && digits > 0 {
            return true;
        } else {
            break;
        }
    }
    false
}

fn average_pdf_line_len(text: &str) -> usize {
    let mut total = 0usize;
    let mut count = 0usize;
    // For performance on large files, sample only the first 2000 lines
    for raw_line in text.lines().take(2000) {
        let line = raw_line.trim();
        if line.is_empty() || looks_like_list_item(line) {
            continue;
        }
        total += line.len();
        count += 1;
    }
    total.checked_div(count).unwrap_or(0)
}

// Error message helpers (copied from main.rs)
fn error_invalid_encoding_message(language: Language) -> String {
    i18n::tr(language, "file_handler.invalid_encoding")
}

pub fn write_pdf_text(
    path: &Path,
    title: &str,
    text: &str,
    language: Language,
) -> Result<(), String> {
    let page_width = Mm(210.0);
    let page_height = Mm(297.0);
    let margin: f32 = 18.0;
    let header_height: f32 = 18.0;
    let footer_height: f32 = 12.0;
    let body_font_size: f32 = 12.0;
    let header_font_size: f32 = 14.0;
    let line_height: f32 = 14.0;
    let bullet_indent_mm: f32 = 6.0;
    let bullet_indent_chars = 4usize;
    let max_chars = estimate_max_chars(page_width.0, margin, body_font_size);
    let title = if title.trim().is_empty() {
        "Sonarpad"
    } else {
        title
    };

    let lines = layout_pdf_lines(
        text,
        max_chars,
        bullet_indent_chars,
        body_font_size,
        bullet_indent_mm,
    );
    let content_top = page_height.0 - margin - header_height;
    let content_bottom = margin + footer_height;

    // Split lines into pages
    let mut page_contents: Vec<Vec<PdfLine>> = Vec::new();
    let mut current: Vec<PdfLine> = Vec::new();
    let mut y = content_top;
    for line in lines {
        if y < content_bottom + line_height {
            page_contents.push(current);
            current = Vec::new();
            y = content_top;
        }
        current.push(line);
        y -= line_height;
    }
    if !current.is_empty() {
        page_contents.push(current);
    } else if page_contents.is_empty() {
        page_contents.push(Vec::new());
    }

    let total_pages = page_contents.len();
    let black = Color::Rgb(Rgb {
        r: 0.0,
        g: 0.0,
        b: 0.0,
        icc_profile: None,
    });

    // Built-in PDF fonts expect a legacy single-byte character encoding.
    // Writing Rust UTF-8 strings with those fonts turns è into Ã¨ when the
    // PDF is opened again. Use an embedded TrueType Unicode font instead.
    // Load the installed Windows fonts at runtime; no font files are shipped.
    let windows_dir = std::env::var_os("WINDIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(r"C:\Windows"));
    let fonts_dir = windows_dir.join("Fonts");
    let mut doc = PdfDocument::new(title);
    let mut unicode_fonts = None;
    for (regular_file, bold_file) in [
        ("segoeui.ttf", "segoeuib.ttf"),
        ("arial.ttf", "arialbd.ttf"),
        ("calibri.ttf", "calibrib.ttf"),
        ("tahoma.ttf", "tahomabd.ttf"),
    ] {
        let (Ok(regular_bytes), Ok(bold_bytes)) = (
            std::fs::read(fonts_dir.join(regular_file)),
            std::fs::read(fonts_dir.join(bold_file)),
        ) else {
            continue;
        };
        let mut warnings = Vec::new();
        if let (Some(regular), Some(bold)) = (
            ParsedFont::from_bytes(&regular_bytes, 0, &mut warnings),
            ParsedFont::from_bytes(&bold_bytes, 0, &mut warnings),
        ) {
            unicode_fonts = Some((doc.add_font(&regular), doc.add_font(&bold)));
            crate::log_debug(&format!(
                "PDF: Unicode fonts loaded from Windows: {regular_file}, {bold_file}"
            ));
            break;
        }
    }
    let (body_font, heading_font) = unicode_fonts.ok_or_else(|| {
        i18n::tr_f(
            language,
            "file_handler.file_save_error",
            &[("err", "No compatible Windows Unicode TrueType fonts found")],
        )
    })?;

    // Build PDF pages
    let mut pdf_pages: Vec<PdfPage> = Vec::new();
    for (page_index, page_lines) in page_contents.iter().enumerate() {
        let mut ops: Vec<Op> = Vec::new();

        // Header (title in bold)
        let header_y = page_height.0 - margin - 8.0;
        ops.push(Op::StartTextSection);
        ops.push(Op::SetTextCursor {
            pos: Point::new(Mm(margin), Mm(header_y)),
        });
        ops.push(Op::SetFontSize {
            size: Pt(header_font_size),
            font: heading_font.clone(),
        });
        ops.push(Op::SetFillColor { col: black.clone() });
        ops.push(Op::WriteText {
            items: vec![TextItem::Text(title.to_string())],
            font: heading_font.clone(),
        });
        ops.push(Op::EndTextSection);

        // Footer (page number)
        let page_label = i18n::tr_f(
            language,
            "file_handler.pdf_page_label",
            &[
                ("page", &(page_index + 1).to_string()),
                ("total", &total_pages.to_string()),
            ],
        );
        ops.push(Op::StartTextSection);
        ops.push(Op::SetTextCursor {
            pos: Point::new(Mm(margin), Mm(margin - 6.0)),
        });
        ops.push(Op::SetFontSize {
            size: Pt(9.0),
            font: body_font.clone(),
        });
        ops.push(Op::SetFillColor { col: black.clone() });
        ops.push(Op::WriteText {
            items: vec![TextItem::Text(page_label)],
            font: body_font.clone(),
        });
        ops.push(Op::EndTextSection);

        // Body content
        let mut y = content_top;
        for line in page_lines {
            if line.is_blank {
                y -= line_height;
                continue;
            }
            ops.push(Op::StartTextSection);
            ops.push(Op::SetTextCursor {
                pos: Point::new(Mm(margin + line.indent), Mm(y)),
            });
            ops.push(Op::SetFontSize {
                size: Pt(line.font_size),
                font: body_font.clone(),
            });
            ops.push(Op::SetFillColor { col: black.clone() });
            ops.push(Op::WriteText {
                items: vec![TextItem::Text(line.text.clone())],
                font: body_font.clone(),
            });
            ops.push(Op::EndTextSection);
            y -= line_height;
        }

        pdf_pages.push(PdfPage::new(page_width, page_height, ops));
    }

    // Save the document with its embedded Unicode font mappings.
    let bytes = doc
        .with_pages(pdf_pages)
        .save(&PdfSaveOptions::default(), &mut Vec::new());

    std::fs::write(path, bytes).map_err(|err| {
        i18n::tr_f(
            language,
            "file_handler.file_save_error",
            &[("err", &err.to_string())],
        )
    })?;
    Ok(())
}

struct PdfLine {
    text: String,
    indent: f32,
    font_size: f32,
    is_blank: bool,
}

fn estimate_max_chars(page_width: f32, margin: f32, font_size: f32) -> usize {
    let usable_mm = page_width - (margin * 2.0);
    let avg_char_mm = (font_size * 0.3528) * 0.5;
    let estimate = (usable_mm / avg_char_mm) as usize;
    estimate.clamp(60, 110)
}

fn layout_pdf_lines(
    text: &str,
    max_chars: usize,
    bullet_indent_chars: usize,
    font_size: f32,
    bullet_indent_mm: f32,
) -> Vec<PdfLine> {
    let mut lines = Vec::new();
    for raw_line in text.lines() {
        let line = raw_line.trim_end_matches('\r');
        if line.trim().is_empty() {
            lines.push(PdfLine {
                text: String::new(),
                indent: 0.0,
                font_size,
                is_blank: true,
            });
            continue;
        }
        if let Some((prefix, content)) = split_list_prefix(line) {
            let first_max = max_chars.saturating_sub(prefix.len());
            let next_max = max_chars.saturating_sub(bullet_indent_chars);
            let mut wrapped = wrap_list_item(content, first_max, next_max);
            if wrapped.is_empty() {
                wrapped.push(String::new());
            }
            lines.push(PdfLine {
                text: format!("{}{}", prefix, wrapped[0]),
                indent: 0.0,
                font_size,
                is_blank: false,
            });
            for rest in wrapped.into_iter().skip(1) {
                lines.push(PdfLine {
                    text: rest,
                    indent: bullet_indent_mm,
                    font_size,
                    is_blank: false,
                });
            }
            continue;
        }
        for wrapped in wrap_words(line, max_chars) {
            lines.push(PdfLine {
                text: wrapped,
                indent: 0.0,
                font_size,
                is_blank: false,
            });
        }
    }
    lines
}

fn split_list_prefix(line: &str) -> Option<(String, &str)> {
    let trimmed = line.trim_start();
    if let Some(rest) = trimmed.strip_prefix("- ") {
        return Some(("- ".to_string(), rest));
    }
    if let Some(rest) = trimmed.strip_prefix("* ") {
        return Some(("* ".to_string(), rest));
    }
    let bytes = trimmed.as_bytes();
    let mut i = 0usize;
    while i < bytes.len() && bytes[i].is_ascii_digit() {
        i += 1;
    }
    if i > 0 && i + 1 < bytes.len() && bytes[i] == b'.' && bytes[i + 1] == b' ' {
        return Some((trimmed[..i + 2].to_string(), &trimmed[i + 2..]));
    }
    None
}

fn wrap_words(text: &str, max_chars: usize) -> Vec<String> {
    let mut lines = Vec::new();
    let mut current = String::new();
    for word in text.split_whitespace() {
        if current.is_empty() {
            current.push_str(word);
        } else if current.len() + 1 + word.len() <= max_chars {
            current.push(' ');
            current.push_str(word);
        } else {
            lines.push(current);
            current = word.to_string();
        }
    }
    if !current.is_empty() {
        lines.push(current);
    }
    lines
}

fn wrap_list_item(content: &str, first_max: usize, next_max: usize) -> Vec<String> {
    let mut lines = Vec::new();
    let mut current = String::new();
    for word in content.split_whitespace() {
        let limit = if lines.is_empty() {
            first_max
        } else {
            next_max
        };
        if current.is_empty() {
            current.push_str(word);
        } else if current.len() + 1 + word.len() <= limit {
            current.push(' ');
            current.push_str(word);
        } else {
            lines.push(current);
            current = word.to_string();
        }
    }
    if !current.is_empty() {
        lines.push(current);
    }
    lines
}

fn error_invalid_utf16le_message(language: Language) -> String {
    i18n::tr(language, "file_handler.utf16le_invalid_length")
}

fn error_invalid_utf16be_message(language: Language) -> String {
    i18n::tr(language, "file_handler.utf16be_invalid_length")
}
