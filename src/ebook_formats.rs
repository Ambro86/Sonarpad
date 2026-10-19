use crate::file_handler::{
    EpubDocumentContent, EpubIndexEntry, byte_offset_to_editor_utf16, decode_ansi_best_effort,
    html_to_text_with_anchors, normalize_epub_index_label, normalize_epub_internal_path,
    percent_decode_epub_component,
};
use crate::settings::{Language, error_open_file_message};
use ebook_rs::{Book as EbookRsBook, MobiBook as EbookRsMobiBook, NavPoint as EbookRsNavPoint};
use quick_xml::events::{BytesStart, Event};
use quick_xml::reader::Reader as XmlReader;
use scraper::{Html, Selector};
use std::collections::hash_map::DefaultHasher;
use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::hash::{Hash, Hasher};
use std::io::Read;
use std::path::{Path, PathBuf};
use zip::ZipArchive;

const MAX_DAISY_TEXT_RESOURCE_BYTES: u64 = 64 * 1024 * 1024;
const MAX_DAISY_RESOURCES: usize = 20_000;
const MAX_KINDLE_TEXT_BYTES: usize = 128 * 1024 * 1024;
const MOBI_COMPRESSION_NONE: u16 = 1;
const MOBI_COMPRESSION_PALMDOC: u16 = 2;
const MOBI_COMPRESSION_HUFF_CDIC: u16 = 0x4448;

#[derive(Clone, Debug)]
struct ResourcePlacement {
    text_start: usize,
    text_len: usize,
    anchors: HashMap<String, usize>,
}

#[derive(Clone, Debug, Default)]
struct RawNavEntry {
    title: String,
    target: String,
    children: Vec<RawNavEntry>,
}

#[derive(Clone, Debug)]
struct FlatNavEntry {
    level: u8,
    title: String,
    target: String,
}

#[derive(Default)]
struct DaisyResources {
    files: HashMap<String, Vec<u8>>,
    entry_hint: Option<String>,
}

#[derive(Default)]
struct DaisyPackage {
    title: Option<String>,
    spine: Vec<String>,
    ncx: Option<String>,
}

#[derive(Default)]
struct SmilInfo {
    target_map: HashMap<String, String>,
    reading_order: HashMap<String, Vec<String>>,
}

pub(crate) fn is_kindle_path(path: &Path) -> bool {
    path.extension()
        .and_then(|value| value.to_str())
        .is_some_and(|extension| {
            extension.eq_ignore_ascii_case("mobi")
                || extension.eq_ignore_ascii_case("azw")
                || extension.eq_ignore_ascii_case("azw3")
        })
}

pub(crate) fn is_daisy_path(path: &Path) -> bool {
    let file_name = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or_default();
    if file_name.eq_ignore_ascii_case("ncc.html") {
        return true;
    }

    let Some(extension) = path.extension().and_then(|value| value.to_str()) else {
        return false;
    };
    if extension.eq_ignore_ascii_case("daisy")
        || extension.eq_ignore_ascii_case("opf")
        || extension.eq_ignore_ascii_case("ncx")
        || extension.eq_ignore_ascii_case("smil")
    {
        return true;
    }
    if extension.eq_ignore_ascii_case("xml") {
        return looks_like_dtbook(path);
    }
    if extension.eq_ignore_ascii_case("zip") {
        return zip_looks_like_daisy(path);
    }
    false
}

pub(crate) fn read_kindle_document(
    path: &Path,
    language: Language,
) -> Result<EpubDocumentContent, String> {
    let bytes = std::fs::read(path).map_err(|error| error_open_file_message(language, error))?;
    let classic_header = parse_classic_mobi_header(&bytes)
        .map_err(|error| error_open_file_message(language, format!("Kindle/MOBI/AZW: {error}")))?;

    if let Some(header) = classic_header.as_ref() {
        if header.encryption != 0 {
            return Err(error_open_file_message(
                language,
                "Kindle/MOBI/AZW: DRM-protected ebooks cannot be imported.",
            ));
        }
        if header.compression == MOBI_COMPRESSION_HUFF_CDIC {
            return read_classic_mobi_document(&bytes, header, language).map_err(|error| {
                error_open_file_message(language, format!("Kindle/MOBI/AZW: {error}"))
            });
        }
    }

    let mut errors = Vec::new();
    match EbookRsMobiBook::parse(&bytes) {
        Ok(book) => match read_ebook_rs_kindle_document(book, language) {
            Ok(document) => return Ok(document),
            Err(message) => errors.push(message),
        },
        Err(error) => errors.push(error),
    }

    if let Some(header) = classic_header.as_ref() {
        match read_classic_mobi_document(&bytes, header, language) {
            Ok(document) => return Ok(document),
            Err(error) => errors.push(error),
        }
    }

    let detail = errors
        .into_iter()
        .find(|message| !message.trim().is_empty())
        .unwrap_or_else(|| "unsupported Kindle ebook".to_string());
    Err(error_open_file_message(
        language,
        format!("Kindle/MOBI/AZW: {detail}"),
    ))
}

#[derive(Clone, Debug)]
struct ClassicMobiHeader {
    compression: u16,
    text_length: usize,
    text_record_count: usize,
    encryption: u16,
    text_encoding: u32,
    mobi_version: u32,
    huff_record_start: usize,
    huff_record_count: usize,
    multibyte_trailer: bool,
    trailing_entry_count: usize,
    record_offsets: Vec<usize>,
}

fn read_u16_be(data: &[u8], offset: usize) -> Result<u16, String> {
    let bytes = data
        .get(offset..offset.saturating_add(2))
        .ok_or_else(|| "truncated Kindle header".to_string())?;
    Ok(u16::from_be_bytes([bytes[0], bytes[1]]))
}

fn read_u32_be(data: &[u8], offset: usize) -> Result<u32, String> {
    let bytes = data
        .get(offset..offset.saturating_add(4))
        .ok_or_else(|| "truncated Kindle header".to_string())?;
    Ok(u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
}

fn parse_pdb_record_offsets(bytes: &[u8]) -> Result<Vec<usize>, String> {
    if bytes.len() < 78 {
        return Err("truncated Palm database header".to_string());
    }
    let record_count = usize::from(read_u16_be(bytes, 76)?);
    if record_count == 0 || record_count > 65_535 {
        return Err("invalid Palm database record count".to_string());
    }
    let table_end = 78usize
        .checked_add(record_count.saturating_mul(8))
        .ok_or_else(|| "invalid Palm database record table".to_string())?;
    if table_end > bytes.len() {
        return Err("truncated Palm database record table".to_string());
    }

    let mut offsets = Vec::with_capacity(record_count);
    for index in 0..record_count {
        let offset = usize::try_from(read_u32_be(bytes, 78 + index * 8)?)
            .map_err(|_| "invalid Palm database record offset".to_string())?;
        if offset < table_end || offset >= bytes.len() {
            return Err("invalid Palm database record offset".to_string());
        }
        if offsets.last().is_some_and(|previous| *previous >= offset) {
            return Err("Palm database record offsets are not ordered".to_string());
        }
        offsets.push(offset);
    }
    Ok(offsets)
}

fn pdb_record<'a>(bytes: &'a [u8], offsets: &[usize], index: usize) -> Result<&'a [u8], String> {
    let start = *offsets
        .get(index)
        .ok_or_else(|| "missing Kindle record".to_string())?;
    let end = offsets.get(index + 1).copied().unwrap_or(bytes.len());
    bytes
        .get(start..end)
        .ok_or_else(|| "invalid Kindle record boundaries".to_string())
}

fn parse_classic_mobi_header(bytes: &[u8]) -> Result<Option<ClassicMobiHeader>, String> {
    if bytes.len() < 78 {
        return Ok(None);
    }
    let signature = bytes.get(60..68).unwrap_or_default();
    if signature != b"BOOKMOBI" && signature != b"TEXtREAd" {
        return Ok(None);
    }

    let offsets = parse_pdb_record_offsets(bytes)?;
    let record_zero = pdb_record(bytes, &offsets, 0)?;
    if record_zero.len() < 16 {
        return Err("truncated PalmDOC header".to_string());
    }

    let compression = read_u16_be(record_zero, 0)?;
    let text_length = usize::try_from(read_u32_be(record_zero, 4)?)
        .map_err(|_| "invalid MOBI text length".to_string())?;
    if text_length > MAX_KINDLE_TEXT_BYTES {
        return Err("Kindle text is larger than the safety limit".to_string());
    }
    let text_record_count = usize::from(read_u16_be(record_zero, 8)?);
    let encryption = read_u16_be(record_zero, 12)?;
    if text_record_count == 0 || text_record_count >= offsets.len() {
        return Err("invalid MOBI text record count".to_string());
    }

    let is_mobi = signature == b"BOOKMOBI";
    let text_encoding = if is_mobi && record_zero.len() >= 32 {
        read_u32_be(record_zero, 0x1c)?
    } else {
        1252
    };
    let mobi_version = if is_mobi && record_zero.len() >= 0x6c {
        read_u32_be(record_zero, 0x68)?
    } else {
        0
    };
    let (huff_record_start, huff_record_count) = if is_mobi && record_zero.len() >= 0x78 {
        (
            usize::try_from(read_u32_be(record_zero, 0x70)?)
                .map_err(|_| "invalid HUFF record offset".to_string())?,
            usize::try_from(read_u32_be(record_zero, 0x74)?)
                .map_err(|_| "invalid HUFF record count".to_string())?,
        )
    } else {
        (0, 0)
    };

    let mut multibyte_trailer = false;
    let mut trailing_entry_count = 0usize;
    if is_mobi && record_zero.len() >= 0xf4 {
        let mobi_header_length = read_u32_be(record_zero, 0x14)?;
        if mobi_header_length >= 0xe4 && mobi_version >= 5 {
            let flags = read_u16_be(record_zero, 0xf2)?;
            multibyte_trailer = flags & 1 != 0;
            trailing_entry_count = (flags >> 1).count_ones() as usize;
        }
    }

    Ok(Some(ClassicMobiHeader {
        compression,
        text_length,
        text_record_count,
        encryption,
        text_encoding,
        mobi_version,
        huff_record_start,
        huff_record_count,
        multibyte_trailer,
        trailing_entry_count,
        record_offsets: offsets,
    }))
}

fn decode_mobi_text(bytes: &[u8], text_encoding: u32, language: Language) -> String {
    if text_encoding == 65001 {
        String::from_utf8_lossy(bytes).into_owned()
    } else {
        decode_ansi_best_effort(bytes, language)
    }
}

fn mobi_title(bytes: &[u8], header: &ClassicMobiHeader, language: Language) -> String {
    let fallback = bytes
        .get(..32)
        .unwrap_or_default()
        .split(|byte| *byte == 0)
        .next()
        .map(|value| decode_mobi_text(value, header.text_encoding, language))
        .unwrap_or_default();
    let Ok(record_zero) = pdb_record(bytes, &header.record_offsets, 0) else {
        return normalize_epub_index_label(&fallback);
    };
    if record_zero.len() < 0x5c || header.mobi_version == 0 {
        return normalize_epub_index_label(&fallback);
    }
    let Ok(offset) = read_u32_be(record_zero, 0x54) else {
        return normalize_epub_index_label(&fallback);
    };
    let Ok(length) = read_u32_be(record_zero, 0x58) else {
        return normalize_epub_index_label(&fallback);
    };
    let (Ok(offset), Ok(length)) = (usize::try_from(offset), usize::try_from(length)) else {
        return normalize_epub_index_label(&fallback);
    };
    let Some(title_bytes) = record_zero.get(offset..offset.saturating_add(length)) else {
        return normalize_epub_index_label(&fallback);
    };
    let title = normalize_epub_index_label(&decode_mobi_text(
        title_bytes,
        header.text_encoding,
        language,
    ));
    if title.is_empty() {
        normalize_epub_index_label(&fallback)
    } else {
        title
    }
}

fn trailing_data_entry_size(data: &[u8]) -> Result<usize, String> {
    if data.is_empty() {
        return Err("invalid empty MOBI trailing-data entry".to_string());
    }
    let mut value = 0usize;
    let start = data.len().saturating_sub(4);
    for &byte in &data[start..] {
        if byte & 0x80 != 0 {
            value = 0;
        }
        value = value
            .checked_shl(7)
            .and_then(|current| current.checked_add(usize::from(byte & 0x7f)))
            .ok_or_else(|| "invalid MOBI trailing-data length".to_string())?;
    }
    if value == 0 || value > data.len() {
        return Err("invalid MOBI trailing-data length".to_string());
    }
    Ok(value)
}

fn trim_mobi_record_trailers<'a>(
    record: &'a [u8],
    header: &ClassicMobiHeader,
) -> Result<&'a [u8], String> {
    let mut end = record.len();
    for _ in 0..header.trailing_entry_count {
        let size = trailing_data_entry_size(&record[..end])?;
        end = end
            .checked_sub(size)
            .ok_or_else(|| "invalid MOBI trailing-data entry".to_string())?;
    }
    if header.multibyte_trailer && end > 0 {
        let size = usize::from(record[end - 1] & 3) + 1;
        end = end
            .checked_sub(size)
            .ok_or_else(|| "invalid MOBI multibyte trailer".to_string())?;
    }
    Ok(&record[..end])
}

fn palmdoc_decompress(input: &[u8]) -> Result<Vec<u8>, String> {
    let mut output = Vec::new();
    let mut cursor = 0usize;
    while cursor < input.len() {
        let control = input[cursor];
        cursor += 1;
        match control {
            0 | 9..=127 => output.push(control),
            1..=8 => {
                let count = usize::from(control);
                let end = cursor
                    .checked_add(count)
                    .ok_or_else(|| "invalid PalmDOC literal run".to_string())?;
                let literal = input
                    .get(cursor..end)
                    .ok_or_else(|| "truncated PalmDOC literal run".to_string())?;
                output.extend_from_slice(literal);
                cursor = end;
            }
            128..=191 => {
                let next = *input
                    .get(cursor)
                    .ok_or_else(|| "truncated PalmDOC back-reference".to_string())?;
                cursor += 1;
                let pair = (u16::from(control) << 8) | u16::from(next);
                let distance = usize::from((pair >> 3) & 0x07ff);
                let count = usize::from((pair & 7) + 3);
                if distance == 0 || distance > output.len() {
                    return Err("invalid PalmDOC back-reference".to_string());
                }
                for _ in 0..count {
                    let byte = output[output.len() - distance];
                    output.push(byte);
                    if output.len() > MAX_KINDLE_TEXT_BYTES {
                        return Err("PalmDOC output exceeds the safety limit".to_string());
                    }
                }
            }
            192..=255 => {
                output.push(b' ');
                output.push(control ^ 0x80);
            }
        }
        if output.len() > MAX_KINDLE_TEXT_BYTES {
            return Err("PalmDOC output exceeds the safety limit".to_string());
        }
    }
    Ok(output)
}

#[derive(Clone, Copy, Debug, Default)]
struct HuffCode {
    length: usize,
    terminal: bool,
    max_code: u32,
}

#[derive(Clone, Debug)]
struct HuffPhrase {
    bytes: Vec<u8>,
    terminal: bool,
}

struct HuffCdicDecoder {
    first_byte_table: [HuffCode; 256],
    min_code: [u32; 33],
    max_code: [u32; 33],
    phrases: Vec<HuffPhrase>,
}

impl HuffCdicDecoder {
    fn from_records(huff: &[u8], cdic_records: &[&[u8]]) -> Result<Self, String> {
        if huff.get(..8) != Some(&b"HUFF\0\0\0\x18"[..]) {
            return Err("invalid HUFF record".to_string());
        }
        let table1_offset = usize::try_from(read_u32_be(huff, 8)?)
            .map_err(|_| "invalid HUFF table offset".to_string())?;
        let table2_offset = usize::try_from(read_u32_be(huff, 12)?)
            .map_err(|_| "invalid HUFF table offset".to_string())?;
        if table1_offset.saturating_add(256 * 4) > huff.len()
            || table2_offset.saturating_add(64 * 4) > huff.len()
        {
            return Err("truncated HUFF table".to_string());
        }

        let mut first_byte_table = [HuffCode::default(); 256];
        for (index, slot) in first_byte_table.iter_mut().enumerate() {
            let value = read_u32_be(huff, table1_offset + index * 4)?;
            let length = usize::try_from(value & 0x1f)
                .map_err(|_| "invalid HUFF code length".to_string())?;
            if length == 0 || length > 32 {
                return Err("invalid HUFF code length".to_string());
            }
            let terminal = value & 0x80 != 0;
            let mut max_code = value >> 8;
            if length <= 8 {
                if !terminal {
                    return Err("invalid short HUFF code".to_string());
                }
                max_code = ((((u64::from(max_code)) + 1) << (32 - length)) - 1) as u32;
            }
            *slot = HuffCode {
                length,
                terminal,
                max_code,
            };
        }

        let mut min_code = [0u32; 33];
        let mut max_code = [0u32; 33];
        for length in 1..=32usize {
            let shift = 32 - length;
            let min_raw = read_u32_be(huff, table2_offset + (length - 1) * 8)?;
            let max_raw = read_u32_be(huff, table2_offset + (length - 1) * 8 + 4)?;
            min_code[length] = ((u64::from(min_raw)) << shift) as u32;
            max_code[length] = ((((u64::from(max_raw)) + 1) << shift) - 1) as u32;
        }

        let mut phrases = Vec::new();
        for cdic in cdic_records {
            if cdic.get(..8) != Some(&b"CDIC\0\0\0\x10"[..]) {
                return Err("invalid CDIC record".to_string());
            }
            let total_phrases = usize::try_from(read_u32_be(cdic, 8)?)
                .map_err(|_| "invalid CDIC phrase count".to_string())?;
            let bits = read_u32_be(cdic, 12)?;
            let per_record = 1usize
                .checked_shl(bits)
                .ok_or_else(|| "invalid CDIC phrase-table size".to_string())?;
            let remaining = total_phrases.saturating_sub(phrases.len());
            let count = per_record.min(remaining);
            if count > 65_536 || 16usize.saturating_add(count.saturating_mul(2)) > cdic.len() {
                return Err("truncated CDIC phrase table".to_string());
            }
            for index in 0..count {
                let offset = usize::from(read_u16_be(cdic, 16 + index * 2)?);
                let start = 16usize
                    .checked_add(offset)
                    .ok_or_else(|| "invalid CDIC phrase offset".to_string())?;
                let length_and_flag = read_u16_be(cdic, start)?;
                let length = usize::from(length_and_flag & 0x7fff);
                let data_start = start.saturating_add(2);
                let data_end = data_start
                    .checked_add(length)
                    .ok_or_else(|| "invalid CDIC phrase length".to_string())?;
                let bytes = cdic
                    .get(data_start..data_end)
                    .ok_or_else(|| "truncated CDIC phrase".to_string())?
                    .to_vec();
                phrases.push(HuffPhrase {
                    bytes,
                    terminal: length_and_flag & 0x8000 != 0,
                });
            }
        }
        if phrases.is_empty() {
            return Err("HUFF/CDIC dictionary is empty".to_string());
        }

        Ok(Self {
            first_byte_table,
            min_code,
            max_code,
            phrases,
        })
    }

    fn peek_code(data: &[u8], bit_position: usize) -> u32 {
        let mut code = 0u32;
        for offset in 0..32usize {
            code <<= 1;
            let position = bit_position + offset;
            if position < data.len().saturating_mul(8) {
                let byte = data[position / 8];
                let bit = (byte >> (7 - (position % 8))) & 1;
                code |= u32::from(bit);
            }
        }
        code
    }

    fn decode_phrase(&mut self, index: usize, depth: usize) -> Result<Vec<u8>, String> {
        if depth > 64 {
            return Err("HUFF/CDIC dictionary recursion is too deep".to_string());
        }
        let phrase = self
            .phrases
            .get(index)
            .ok_or_else(|| "HUFF/CDIC phrase index is out of range".to_string())?;
        if phrase.terminal {
            return Ok(phrase.bytes.clone());
        }
        let compressed = phrase.bytes.clone();
        let expanded = self.decode_stream_internal(&compressed, depth + 1)?;
        if expanded.len() > MAX_KINDLE_TEXT_BYTES {
            return Err("HUFF/CDIC phrase exceeds the safety limit".to_string());
        }
        let slot = self
            .phrases
            .get_mut(index)
            .ok_or_else(|| "HUFF/CDIC phrase index is out of range".to_string())?;
        slot.bytes = expanded.clone();
        slot.terminal = true;
        Ok(expanded)
    }

    fn decode_stream_internal(&mut self, data: &[u8], depth: usize) -> Result<Vec<u8>, String> {
        let total_bits = data.len().saturating_mul(8);
        let mut bit_position = 0usize;
        let mut output = Vec::new();
        while bit_position < total_bits {
            let code = Self::peek_code(data, bit_position);
            let mut descriptor = self.first_byte_table[(code >> 24) as usize];
            if !descriptor.terminal {
                while descriptor.length <= 32 && code < self.min_code[descriptor.length] {
                    descriptor.length += 1;
                }
                if descriptor.length > 32 {
                    return Err("invalid HUFF bit stream".to_string());
                }
                descriptor.max_code = self.max_code[descriptor.length];
            }
            if descriptor.length == 0 || total_bits - bit_position < descriptor.length {
                break;
            }
            if descriptor.max_code < code {
                return Err("invalid HUFF code range".to_string());
            }
            let shift = 32 - descriptor.length;
            let phrase_index = usize::try_from((descriptor.max_code - code) >> shift)
                .map_err(|_| "invalid HUFF phrase index".to_string())?;
            bit_position += descriptor.length;
            let phrase = self.decode_phrase(phrase_index, depth)?;
            output.extend_from_slice(&phrase);
            if output.len() > MAX_KINDLE_TEXT_BYTES {
                return Err("HUFF/CDIC output exceeds the safety limit".to_string());
            }
        }
        Ok(output)
    }

    fn decode_stream(&mut self, data: &[u8]) -> Result<Vec<u8>, String> {
        self.decode_stream_internal(data, 0)
    }
}

fn decode_classic_mobi_markup(bytes: &[u8], header: &ClassicMobiHeader) -> Result<Vec<u8>, String> {
    let mut decoder = if header.compression == MOBI_COMPRESSION_HUFF_CDIC {
        if header.huff_record_count < 2 {
            return Err("HUFF/CDIC dictionary records are missing".to_string());
        }
        let huff = pdb_record(bytes, &header.record_offsets, header.huff_record_start)?;
        let mut cdic_records = Vec::new();
        for index in 1..header.huff_record_count {
            cdic_records.push(pdb_record(
                bytes,
                &header.record_offsets,
                header.huff_record_start + index,
            )?);
        }
        Some(HuffCdicDecoder::from_records(huff, &cdic_records)?)
    } else {
        None
    };

    let mut output = Vec::new();
    for index in 0..header.text_record_count {
        let record = pdb_record(bytes, &header.record_offsets, index + 1)?;
        let record = trim_mobi_record_trailers(record, header)?;
        let expanded = match header.compression {
            MOBI_COMPRESSION_NONE => record.to_vec(),
            MOBI_COMPRESSION_PALMDOC => palmdoc_decompress(record)?,
            MOBI_COMPRESSION_HUFF_CDIC => decoder
                .as_mut()
                .ok_or_else(|| "HUFF/CDIC decoder is unavailable".to_string())?
                .decode_stream(record)?,
            other => return Err(format!("unsupported MOBI compression type 0x{other:04x}")),
        };
        output.extend_from_slice(&expanded);
        if output.len() > MAX_KINDLE_TEXT_BYTES {
            return Err("Kindle text exceeds the safety limit".to_string());
        }
    }
    if header.text_length > 0 && output.len() > header.text_length {
        output.truncate(header.text_length);
    }
    Ok(output)
}

fn read_classic_mobi_document(
    bytes: &[u8],
    header: &ClassicMobiHeader,
    language: Language,
) -> Result<EpubDocumentContent, String> {
    if header.encryption != 0 {
        return Err("DRM-protected ebooks cannot be imported".to_string());
    }
    let raw = decode_classic_mobi_markup(bytes, header)?;
    let markup = decode_mobi_text(&raw, header.text_encoding, language);
    let title = mobi_title(bytes, header, language);
    build_single_markup_kindle_document(&title, &markup, "kindle-book.html", language)
}

fn build_single_markup_kindle_document(
    title: &str,
    markup: &str,
    resource_path: &str,
    language: Language,
) -> Result<EpubDocumentContent, String> {
    let (cleaned, anchors) = html_to_text_with_anchors(markup);
    if cleaned.trim().is_empty() {
        return Err(error_open_file_message(
            language,
            "The Kindle/MOBI/AZW ebook contains no readable text.",
        ));
    }

    let mut full_text = String::new();
    if !title.is_empty() {
        full_text.push_str(title);
        full_text.push_str("\n\n");
    }
    let text_start = full_text.len();
    full_text.push_str(&cleaned);
    if !full_text.ends_with('\n') {
        full_text.push('\n');
    }

    let normalized_resource = normalize_epub_internal_path(resource_path);
    let mut placements = HashMap::new();
    placements.insert(
        normalized_resource.clone(),
        ResourcePlacement {
            text_start,
            text_len: cleaned.len(),
            anchors,
        },
    );
    let headings = extract_markup_navigation(markup, &normalized_resource);
    let navigation = build_navigation_tree(&headings);
    let index = resolve_navigation(&navigation, &placements, &full_text);

    Ok(EpubDocumentContent {
        text: full_text,
        index,
    })
}

fn read_ebook_rs_kindle_document(
    book: EbookRsBook,
    language: Language,
) -> Result<EpubDocumentContent, String> {
    let mut full_text = String::new();
    let title = normalize_epub_index_label(&book.opf.metadata.title);
    if !title.is_empty() {
        full_text.push_str(&title);
        full_text.push_str("\n\n");
    }

    let mut placements = HashMap::new();
    let mut heading_navigation = Vec::new();
    for section in &book.sections {
        let source = if section.full_path.trim().is_empty() {
            section.href.as_str()
        } else {
            section.full_path.as_str()
        };
        let normalized_source = normalize_epub_internal_path(source);
        let markup = if section.raw_html.trim().is_empty() {
            section.processed_html.as_str()
        } else {
            section.raw_html.as_str()
        };
        let (cleaned, anchors) = html_to_text_with_anchors(markup);
        if cleaned.trim().is_empty() {
            continue;
        }

        let text_start = full_text.len();
        full_text.push_str(&cleaned);
        if !full_text.ends_with('\n') {
            full_text.push('\n');
        }
        full_text.push('\n');
        placements.insert(
            normalized_source.clone(),
            ResourcePlacement {
                text_start,
                text_len: cleaned.len(),
                anchors,
            },
        );
        heading_navigation.extend(extract_markup_navigation(markup, &normalized_source));
    }

    if full_text.trim().is_empty() {
        return Err(error_open_file_message(
            language,
            "The Kindle/MOBI/AZW ebook contains no readable text.",
        ));
    }

    // Prefer meaningful headings found in the book. If the KF8 parser exposes
    // a navigation tree, use it when it resolves to actual imported sections.
    let toc = ebook_rs_toc_to_raw(&book.toc);
    let mut index = resolve_navigation(&toc, &placements, &full_text);
    if index.is_empty() || ebook_rs_toc_is_generic(&book.toc) {
        let headings = build_navigation_tree(&heading_navigation);
        let heading_index = resolve_navigation(&headings, &placements, &full_text);
        if !heading_index.is_empty() {
            index = heading_index;
        }
    }

    Ok(EpubDocumentContent {
        text: full_text,
        index,
    })
}

fn ebook_rs_toc_to_raw(entries: &[EbookRsNavPoint]) -> Vec<RawNavEntry> {
    entries
        .iter()
        .filter_map(|entry| {
            let title = normalize_epub_index_label(&entry.label);
            if title.is_empty() {
                return None;
            }
            let target = if entry.full_path.trim().is_empty() {
                entry.href.clone()
            } else {
                entry.full_path.clone()
            };
            Some(RawNavEntry {
                title,
                target,
                children: ebook_rs_toc_to_raw(&entry.subitems),
            })
        })
        .collect()
}

fn ebook_rs_toc_is_generic(entries: &[EbookRsNavPoint]) -> bool {
    !entries.is_empty()
        && entries.iter().enumerate().all(|(index, entry)| {
            entry.subitems.is_empty() && entry.label == format!("Section {}", index + 1)
        })
}

pub(crate) fn read_daisy_document(
    path: &Path,
    language: Language,
) -> Result<EpubDocumentContent, String> {
    let resources = load_daisy_resources(path, language)?;
    let ncc = preferred_resource(&resources, "ncc.html");
    if let Some(ncc_path) = ncc {
        return read_daisy_202(&resources, &ncc_path, language);
    }

    let hinted = resources.entry_hint.as_deref().unwrap_or_default();
    if hinted.to_ascii_lowercase().ends_with(".opf") {
        return read_daisy_3_package(&resources, hinted, language);
    }
    if let Some(opf) = first_resource_with_extension(&resources, "opf") {
        return read_daisy_3_package(&resources, &opf, language);
    }

    if hinted.to_ascii_lowercase().ends_with(".ncx") {
        return read_daisy_from_ncx(&resources, hinted, language);
    }
    if hinted.to_ascii_lowercase().ends_with(".smil") {
        return read_daisy_from_smil(&resources, hinted, language);
    }
    if hinted.to_ascii_lowercase().ends_with(".xml") {
        return read_direct_dtbook(&resources, hinted, language);
    }

    if let Some(ncx) = first_resource_with_extension(&resources, "ncx") {
        return read_daisy_from_ncx(&resources, &ncx, language);
    }
    if let Some(dtbook) = find_dtbook_resource(&resources, language) {
        return read_direct_dtbook(&resources, &dtbook, language);
    }

    Err(error_open_file_message(
        language,
        "No DAISY 2.02 NCC or DAISY 3 package/DTBook was found.",
    ))
}

fn read_daisy_202(
    resources: &DaisyResources,
    ncc_path: &str,
    language: Language,
) -> Result<EpubDocumentContent, String> {
    let ncc_markup = resource_text(resources, ncc_path, language)?;
    let flat_navigation = parse_ncc_navigation(&ncc_markup, ncc_path);
    let navigation = build_navigation_tree(&flat_navigation);
    let smil = parse_all_smil(resources, language);

    let mut content_order = Vec::new();
    for item in &flat_navigation {
        if let Some(target) = map_smil_target(&item.target, &smil) {
            push_unique_target(&mut content_order, &target);
        }
    }
    if content_order.is_empty() {
        let mut smil_keys: Vec<_> = smil.reading_order.keys().cloned().collect();
        smil_keys.sort();
        for key in smil_keys {
            if let Some(targets) = smil.reading_order.get(&key) {
                for target in targets {
                    push_unique_target(&mut content_order, target);
                }
            }
        }
    }

    let mut full_text = String::new();
    let mut placements = HashMap::new();
    let mut fallback_navigation = Vec::new();
    append_resources_in_order(
        resources,
        &content_order,
        language,
        &mut full_text,
        &mut placements,
        &mut fallback_navigation,
    )?;

    let mapped_navigation = remap_navigation_targets(&navigation, &smil);
    let mut index = resolve_navigation(&mapped_navigation, &placements, &full_text);
    if full_text.trim().is_empty() {
        return navigation_only_document(&mapped_navigation, language);
    }
    if index.is_empty() {
        index = resolve_navigation(
            &build_navigation_tree(&fallback_navigation),
            &placements,
            &full_text,
        );
    }

    Ok(EpubDocumentContent {
        text: full_text,
        index,
    })
}

fn read_daisy_3_package(
    resources: &DaisyResources,
    opf_path: &str,
    language: Language,
) -> Result<EpubDocumentContent, String> {
    let opf_markup = resource_text(resources, opf_path, language)?;
    let package = parse_daisy_opf(&opf_markup, opf_path);
    let mut navigation = Vec::new();
    if let Some(ncx_path) = package.ncx.as_deref()
        && let Ok(ncx_markup) = resource_text(resources, ncx_path, language)
    {
        navigation = parse_ncx_navigation(&ncx_markup, ncx_path);
    }

    let mut order = package.spine.clone();
    if order.is_empty() {
        collect_nav_targets(&navigation, &mut order);
    }

    let mut full_text = String::new();
    if let Some(title) = package.title.as_deref() {
        let title = normalize_epub_index_label(title);
        if !title.is_empty() {
            full_text.push_str(&title);
            full_text.push_str("\n\n");
        }
    }
    let mut placements = HashMap::new();
    let mut fallback_navigation = Vec::new();
    append_resources_in_order(
        resources,
        &order,
        language,
        &mut full_text,
        &mut placements,
        &mut fallback_navigation,
    )?;

    let mut index = resolve_navigation(&navigation, &placements, &full_text);
    if full_text.trim().is_empty() {
        return navigation_only_document(&navigation, language);
    }
    if index.is_empty() {
        index = resolve_navigation(
            &build_navigation_tree(&fallback_navigation),
            &placements,
            &full_text,
        );
    }

    Ok(EpubDocumentContent {
        text: full_text,
        index,
    })
}

fn read_daisy_from_ncx(
    resources: &DaisyResources,
    ncx_path: &str,
    language: Language,
) -> Result<EpubDocumentContent, String> {
    let ncx_markup = resource_text(resources, ncx_path, language)?;
    let navigation = parse_ncx_navigation(&ncx_markup, ncx_path);
    let mut order = Vec::new();
    collect_nav_targets(&navigation, &mut order);

    let mut full_text = String::new();
    let mut placements = HashMap::new();
    let mut fallback_navigation = Vec::new();
    append_resources_in_order(
        resources,
        &order,
        language,
        &mut full_text,
        &mut placements,
        &mut fallback_navigation,
    )?;
    if full_text.trim().is_empty() {
        return navigation_only_document(&navigation, language);
    }
    let mut index = resolve_navigation(&navigation, &placements, &full_text);
    if index.is_empty() {
        index = resolve_navigation(
            &build_navigation_tree(&fallback_navigation),
            &placements,
            &full_text,
        );
    }
    Ok(EpubDocumentContent {
        text: full_text,
        index,
    })
}

fn read_daisy_from_smil(
    resources: &DaisyResources,
    smil_path: &str,
    language: Language,
) -> Result<EpubDocumentContent, String> {
    let markup = resource_text(resources, smil_path, language)?;
    let mut smil = SmilInfo::default();
    parse_smil_document(&markup, smil_path, &mut smil);
    let order = smil
        .reading_order
        .get(&normalize_epub_internal_path(smil_path))
        .cloned()
        .unwrap_or_default();
    let mut full_text = String::new();
    let mut placements = HashMap::new();
    let mut fallback_navigation = Vec::new();
    append_resources_in_order(
        resources,
        &order,
        language,
        &mut full_text,
        &mut placements,
        &mut fallback_navigation,
    )?;
    if full_text.trim().is_empty() {
        return Err(error_open_file_message(
            language,
            "This DAISY SMIL references audio only and contains no textual navigation labels.",
        ));
    }
    let fallback = build_navigation_tree(&fallback_navigation);
    let index = resolve_navigation(&fallback, &placements, &full_text);
    Ok(EpubDocumentContent {
        text: full_text,
        index,
    })
}

fn read_direct_dtbook(
    resources: &DaisyResources,
    dtbook_path: &str,
    language: Language,
) -> Result<EpubDocumentContent, String> {
    let markup = resource_text(resources, dtbook_path, language)?;
    if !markup.to_ascii_lowercase().contains("<dtbook") {
        return Err(error_open_file_message(
            language,
            "The XML file is not a DAISY DTBook document.",
        ));
    }
    let (text, anchors) = html_to_text_with_anchors(&markup);
    if text.trim().is_empty() {
        return Err(error_open_file_message(
            language,
            "The DAISY DTBook contains no readable text.",
        ));
    }
    let normalized = normalize_epub_internal_path(dtbook_path);
    let mut placements = HashMap::new();
    placements.insert(
        normalized.clone(),
        ResourcePlacement {
            text_start: 0,
            text_len: text.len(),
            anchors,
        },
    );
    let navigation = build_navigation_tree(&extract_markup_navigation(&markup, &normalized));
    let index = resolve_navigation(&navigation, &placements, &text);
    Ok(EpubDocumentContent { text, index })
}

fn navigation_only_document(
    navigation: &[RawNavEntry],
    language: Language,
) -> Result<EpubDocumentContent, String> {
    let mut text = String::new();
    let mut index = Vec::new();
    append_navigation_labels(navigation, &mut text, &mut index);
    if text.trim().is_empty() {
        return Err(error_open_file_message(
            language,
            "The DAISY book has neither readable text nor navigation labels.",
        ));
    }
    Ok(EpubDocumentContent { text, index })
}

fn append_navigation_labels(
    entries: &[RawNavEntry],
    text: &mut String,
    index: &mut Vec<EpubIndexEntry>,
) {
    for entry in entries {
        let target_utf16 = text.encode_utf16().count().min(i32::MAX as usize) as i32;
        text.push_str(&entry.title);
        text.push('\n');
        let mut children = Vec::new();
        append_navigation_labels(&entry.children, text, &mut children);
        index.push(EpubIndexEntry {
            title: entry.title.clone(),
            target_utf16,
            children,
        });
    }
}

fn append_resources_in_order(
    resources: &DaisyResources,
    targets: &[String],
    language: Language,
    full_text: &mut String,
    placements: &mut HashMap<String, ResourcePlacement>,
    fallback_navigation: &mut Vec<FlatNavEntry>,
) -> Result<(), String> {
    let mut seen = HashSet::new();
    for target in targets {
        let (path_part, _fragment) = split_target(target);
        let normalized = normalize_epub_internal_path(path_part);
        if normalized.is_empty() || !seen.insert(normalized.clone()) {
            continue;
        }
        let Some(key) = find_resource_key(resources, &normalized) else {
            continue;
        };
        let markup = resource_text(resources, &key, language)?;
        let (cleaned, anchors) = html_to_text_with_anchors(&markup);
        if cleaned.trim().is_empty() {
            continue;
        }
        let text_start = full_text.len();
        full_text.push_str(&cleaned);
        if !full_text.ends_with('\n') {
            full_text.push('\n');
        }
        full_text.push('\n');
        placements.insert(
            normalize_epub_internal_path(&key),
            ResourcePlacement {
                text_start,
                text_len: cleaned.len(),
                anchors,
            },
        );
        fallback_navigation.extend(extract_markup_navigation(&markup, &key));
    }
    Ok(())
}

fn resolve_navigation(
    entries: &[RawNavEntry],
    placements: &HashMap<String, ResourcePlacement>,
    full_text: &str,
) -> Vec<EpubIndexEntry> {
    let mut output = Vec::new();
    for entry in entries {
        let children = resolve_navigation(&entry.children, placements, full_text);
        let target = resolve_navigation_target(&entry.target, &entry.title, placements, full_text)
            .or_else(|| children.first().map(|child| child.target_utf16));
        if let Some(target_utf16) = target {
            output.push(EpubIndexEntry {
                title: entry.title.clone(),
                target_utf16,
                children,
            });
        } else {
            output.extend(children);
        }
    }
    output
}

fn resolve_navigation_target(
    target: &str,
    title: &str,
    placements: &HashMap<String, ResourcePlacement>,
    full_text: &str,
) -> Option<i32> {
    let (path_part, fragment) = split_target(target);
    let normalized = normalize_epub_internal_path(&percent_decode_epub_component(path_part));
    let (placement_path, placement) = placements.get_key_value(&normalized).or_else(|| {
        placements.iter().find(|(path, _placement)| {
            path.eq_ignore_ascii_case(&normalized)
                || path.ends_with(&format!("/{normalized}"))
                || normalized.ends_with(&format!("/{path}"))
        })
    })?;

    let decoded_fragment = percent_decode_epub_component(fragment);
    let local_offset = if !decoded_fragment.is_empty() {
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
            .or_else(|| find_title_in_placement(full_text, placement, title))
            .unwrap_or(0)
    } else {
        find_title_in_placement(full_text, placement, title).unwrap_or(0)
    };

    let _placement_path = placement_path;
    Some(byte_offset_to_editor_utf16(
        full_text,
        placement.text_start.saturating_add(local_offset),
    ))
}

fn find_title_in_placement(
    full_text: &str,
    placement: &ResourcePlacement,
    title: &str,
) -> Option<usize> {
    let title = normalize_epub_index_label(title);
    if title.is_empty() {
        return None;
    }
    let start = placement.text_start.min(full_text.len());
    let end = placement
        .text_start
        .saturating_add(placement.text_len)
        .min(full_text.len());
    if start >= end {
        return None;
    }
    let mut offset = 0usize;
    for line in full_text[start..end].split_inclusive('\n') {
        let trimmed = line.trim();
        if normalize_epub_index_label(trimmed).eq_ignore_ascii_case(&title) {
            let leading = line.find(trimmed).unwrap_or(0);
            return Some(offset.saturating_add(leading));
        }
        offset = offset.saturating_add(line.len());
    }
    None
}

fn parse_ncc_navigation(markup: &str, ncc_path: &str) -> Vec<FlatNavEntry> {
    let document = Html::parse_document(markup);
    let Ok(selector) = Selector::parse("h1, h2, h3, h4, h5, h6, span") else {
        return Vec::new();
    };
    let Ok(link_selector) = Selector::parse("a[href]") else {
        return Vec::new();
    };
    let mut output = Vec::new();
    for element in document.select(&selector) {
        let name = element.value().name();
        let level = match name {
            "h1" => 1,
            "h2" => 2,
            "h3" => 3,
            "h4" => 4,
            "h5" => 5,
            "h6" => 6,
            "span" => {
                let class = element.value().attr("class").unwrap_or_default();
                if !class.to_ascii_lowercase().contains("page") {
                    continue;
                }
                7
            }
            _ => continue,
        };
        let Some(link) = element.select(&link_selector).next() else {
            continue;
        };
        let Some(href) = link.value().attr("href") else {
            continue;
        };
        let title = normalize_epub_index_label(&element.text().collect::<Vec<_>>().join(" "));
        if title.is_empty() {
            continue;
        }
        output.push(FlatNavEntry {
            level,
            title,
            target: resolve_relative_target(ncc_path, href),
        });
    }
    output
}

fn parse_daisy_opf(markup: &str, opf_path: &str) -> DaisyPackage {
    let mut reader = XmlReader::from_str(markup);
    reader.config_mut().trim_text(true);
    let mut buf = Vec::new();
    let mut manifest: HashMap<String, (String, String)> = HashMap::new();
    let mut spine_ids = Vec::new();
    let mut spine_toc_id = None;
    let mut title = None;
    let mut in_title = false;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(event)) => match xml_local_name(event.name().as_ref()) {
                b"item" => parse_opf_item(&event, &mut manifest),
                b"itemref" => {
                    if let Some(idref) = xml_attribute(&event, b"idref") {
                        spine_ids.push(idref);
                    }
                }
                b"spine" => spine_toc_id = xml_attribute(&event, b"toc"),
                b"title" => in_title = true,
                _ => {}
            },
            Ok(Event::Empty(event)) => match xml_local_name(event.name().as_ref()) {
                b"item" => parse_opf_item(&event, &mut manifest),
                b"itemref" => {
                    if let Some(idref) = xml_attribute(&event, b"idref") {
                        spine_ids.push(idref);
                    }
                }
                _ => {}
            },
            Ok(Event::Text(text_event)) if in_title && title.is_none() => {
                let value = String::from_utf8_lossy(text_event.as_ref());
                let value = normalize_epub_index_label(&value);
                if !value.is_empty() {
                    title = Some(value);
                }
            }
            Ok(Event::End(event)) if xml_local_name(event.name().as_ref()) == b"title" => {
                in_title = false;
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
        buf.clear();
    }

    let mut spine = Vec::new();
    for id in spine_ids {
        if let Some((href, _media_type)) = manifest.get(&id) {
            push_unique_target(&mut spine, &resolve_relative_target(opf_path, href));
        }
    }
    let ncx = spine_toc_id
        .as_ref()
        .and_then(|id| manifest.get(id))
        .map(|(href, _media_type)| resolve_relative_target(opf_path, href))
        .or_else(|| {
            manifest.values().find_map(|(href, media_type)| {
                (media_type.eq_ignore_ascii_case("application/x-dtbncx+xml")
                    || href.to_ascii_lowercase().ends_with(".ncx"))
                .then(|| resolve_relative_target(opf_path, href))
            })
        });

    DaisyPackage { title, spine, ncx }
}

fn parse_opf_item(event: &BytesStart<'_>, manifest: &mut HashMap<String, (String, String)>) {
    let Some(id) = xml_attribute(event, b"id") else {
        return;
    };
    let Some(href) = xml_attribute(event, b"href") else {
        return;
    };
    let media_type = xml_attribute(event, b"media-type").unwrap_or_default();
    manifest.insert(id, (href, media_type));
}

fn parse_ncx_navigation(markup: &str, ncx_path: &str) -> Vec<RawNavEntry> {
    #[derive(Default)]
    struct PartialNavPoint {
        title: String,
        target: String,
        children: Vec<RawNavEntry>,
    }

    let mut reader = XmlReader::from_str(markup);
    reader.config_mut().trim_text(true);
    let mut buf = Vec::new();
    let mut stack: Vec<PartialNavPoint> = Vec::new();
    let mut roots = Vec::new();
    let mut nav_label_depth = 0usize;
    let mut text_depth = 0usize;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(event)) => match xml_local_name(event.name().as_ref()) {
                b"navPoint" => stack.push(PartialNavPoint::default()),
                b"navLabel" => nav_label_depth = nav_label_depth.saturating_add(1),
                b"text" if nav_label_depth > 0 => text_depth = text_depth.saturating_add(1),
                b"content" => {
                    if let Some(current) = stack.last_mut()
                        && let Some(src) = xml_attribute(&event, b"src")
                    {
                        current.target = resolve_relative_target(ncx_path, &src);
                    }
                }
                _ => {}
            },
            Ok(Event::Empty(event)) if xml_local_name(event.name().as_ref()) == b"content" => {
                if let Some(current) = stack.last_mut()
                    && let Some(src) = xml_attribute(&event, b"src")
                {
                    current.target = resolve_relative_target(ncx_path, &src);
                }
            }
            Ok(Event::Text(event)) if nav_label_depth > 0 && text_depth > 0 => {
                if let Some(current) = stack.last_mut() {
                    let value = String::from_utf8_lossy(event.as_ref());
                    if !value.trim().is_empty() {
                        if !current.title.is_empty() {
                            current.title.push(' ');
                        }
                        current.title.push_str(value.trim());
                    }
                }
            }
            Ok(Event::End(event)) => match xml_local_name(event.name().as_ref()) {
                b"text" => text_depth = text_depth.saturating_sub(1),
                b"navLabel" => nav_label_depth = nav_label_depth.saturating_sub(1),
                b"navPoint" => {
                    if let Some(partial) = stack.pop() {
                        let title = normalize_epub_index_label(&partial.title);
                        if title.is_empty() {
                            if let Some(parent) = stack.last_mut() {
                                parent.children.extend(partial.children);
                            } else {
                                roots.extend(partial.children);
                            }
                        } else {
                            let entry = RawNavEntry {
                                title,
                                target: partial.target,
                                children: partial.children,
                            };
                            if let Some(parent) = stack.last_mut() {
                                parent.children.push(entry);
                            } else {
                                roots.push(entry);
                            }
                        }
                    }
                }
                _ => {}
            },
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
        buf.clear();
    }
    roots
}

fn parse_all_smil(resources: &DaisyResources, language: Language) -> SmilInfo {
    let mut output = SmilInfo::default();
    let mut smil_paths: Vec<_> = resources
        .files
        .keys()
        .filter(|path| path.to_ascii_lowercase().ends_with(".smil"))
        .cloned()
        .collect();
    smil_paths.sort();
    for path in smil_paths {
        if let Ok(markup) = resource_text(resources, &path, language) {
            parse_smil_document(&markup, &path, &mut output);
        }
    }
    output
}

fn parse_smil_document(markup: &str, smil_path: &str, output: &mut SmilInfo) {
    let normalized_smil = normalize_epub_internal_path(smil_path);
    let mut reader = XmlReader::from_str(markup);
    reader.config_mut().trim_text(true);
    let mut buf = Vec::new();
    let mut current_par_id: Option<String> = None;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(event)) => match xml_local_name(event.name().as_ref()) {
                b"par" => current_par_id = xml_attribute(&event, b"id"),
                b"text" => {
                    register_smil_text(&event, &normalized_smil, current_par_id.as_deref(), output)
                }
                _ => {}
            },
            Ok(Event::Empty(event)) if xml_local_name(event.name().as_ref()) == b"text" => {
                register_smil_text(&event, &normalized_smil, current_par_id.as_deref(), output);
            }
            Ok(Event::End(event)) if xml_local_name(event.name().as_ref()) == b"par" => {
                current_par_id = None;
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
        buf.clear();
    }
}

fn register_smil_text(
    event: &BytesStart<'_>,
    smil_path: &str,
    current_par_id: Option<&str>,
    output: &mut SmilInfo,
) {
    let Some(src) = xml_attribute(event, b"src") else {
        return;
    };
    let target = resolve_relative_target(smil_path, &src);
    let reading_order = output
        .reading_order
        .entry(smil_path.to_string())
        .or_default();
    push_unique_target(reading_order, &target);
    output
        .target_map
        .entry(smil_path.to_string())
        .or_insert_with(|| target.clone());
    if let Some(par_id) = current_par_id {
        output
            .target_map
            .insert(format!("{smil_path}#{par_id}"), target.clone());
    }
    if let Some(text_id) = xml_attribute(event, b"id") {
        output
            .target_map
            .insert(format!("{smil_path}#{text_id}"), target);
    }
}

fn map_smil_target(target: &str, smil: &SmilInfo) -> Option<String> {
    let normalized_target = normalize_target(target);
    if let Some(mapped) = smil.target_map.get(&normalized_target) {
        return Some(mapped.clone());
    }
    let (path_part, _fragment) = split_target(&normalized_target);
    let lower = path_part.to_ascii_lowercase();
    if lower.ends_with(".smil") {
        return smil.target_map.get(path_part).cloned();
    }
    Some(normalized_target)
}

fn remap_navigation_targets(entries: &[RawNavEntry], smil: &SmilInfo) -> Vec<RawNavEntry> {
    entries
        .iter()
        .map(|entry| RawNavEntry {
            title: entry.title.clone(),
            target: map_smil_target(&entry.target, smil).unwrap_or_else(|| entry.target.clone()),
            children: remap_navigation_targets(&entry.children, smil),
        })
        .collect()
}

fn build_navigation_tree(flat: &[FlatNavEntry]) -> Vec<RawNavEntry> {
    fn build(flat: &[FlatNavEntry], cursor: &mut usize, parent_level: u8) -> Vec<RawNavEntry> {
        let mut output = Vec::new();
        while *cursor < flat.len() {
            let current = &flat[*cursor];
            if current.level <= parent_level {
                break;
            }
            let level = current.level;
            let title = current.title.clone();
            let target = current.target.clone();
            *cursor += 1;
            let children = build(flat, cursor, level);
            output.push(RawNavEntry {
                title,
                target,
                children,
            });
        }
        output
    }

    let mut cursor = 0usize;
    build(flat, &mut cursor, 0)
}

fn extract_markup_navigation(markup: &str, resource_path: &str) -> Vec<FlatNavEntry> {
    let document = Html::parse_document(markup);
    let Ok(selector) = Selector::parse("h1, h2, h3, h4, h5, h6") else {
        return Vec::new();
    };
    let mut output = Vec::new();
    for element in document.select(&selector) {
        let level = element
            .value()
            .name()
            .as_bytes()
            .get(1)
            .and_then(|value| value.checked_sub(b'0'))
            .filter(|value| (1..=6).contains(value))
            .unwrap_or(1);
        let title = normalize_epub_index_label(&element.text().collect::<Vec<_>>().join(" "));
        if title.is_empty() {
            continue;
        }
        let target = element
            .value()
            .attr("id")
            .filter(|id| !id.trim().is_empty())
            .map(|id| format!("{}#{}", normalize_epub_internal_path(resource_path), id))
            .unwrap_or_else(|| normalize_epub_internal_path(resource_path));
        output.push(FlatNavEntry {
            level,
            title,
            target,
        });
    }
    output
}

fn collect_nav_targets(entries: &[RawNavEntry], output: &mut Vec<String>) {
    for entry in entries {
        push_unique_target(output, &entry.target);
        collect_nav_targets(&entry.children, output);
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct DaisyAudioSegment {
    pub(crate) source: String,
    pub(crate) clip_begin_secs: f64,
    pub(crate) clip_end_secs: Option<f64>,
}

#[derive(Clone, Debug)]
pub(crate) struct DaisyPlaybackChapter {
    pub(crate) title: String,
    pub(crate) segments: Vec<DaisyAudioSegment>,
    // Disjoint intervals for continuous playback. A parent navigation entry's
    // full segments include its children; playing those again would repeat audio.
    pub(crate) playback_start: Option<usize>,
    pub(crate) playback_segments: Vec<DaisyAudioSegment>,
}

#[derive(Clone, Debug)]
pub(crate) struct DaisyPlaybackCatalog {
    pub(crate) index: Vec<EpubIndexEntry>,
    pub(crate) chapters: Vec<DaisyPlaybackChapter>,
}

#[derive(Clone, Debug, Default)]
struct SmilPlaybackPar {
    ids: Vec<String>,
    text_target: Option<String>,
    segments: Vec<DaisyAudioSegment>,
}

fn parse_smil_clock_value(value: &str) -> Option<f64> {
    let mut value = value.trim().to_ascii_lowercase();
    if let Some(stripped) = value.strip_prefix("npt=") {
        value = stripped.trim().to_string();
    }
    if let Some(number) = value.strip_suffix("ms") {
        return number.trim().parse::<f64>().ok().map(|v| v / 1000.0);
    }
    if let Some(number) = value.strip_suffix("min") {
        return number.trim().parse::<f64>().ok().map(|v| v * 60.0);
    }
    if let Some(number) = value.strip_suffix('h') {
        return number.trim().parse::<f64>().ok().map(|v| v * 3600.0);
    }
    if let Some(number) = value.strip_suffix('s') {
        return number.trim().parse::<f64>().ok();
    }
    if value.contains(':') {
        let parts: Vec<_> = value.split(':').collect();
        return match parts.as_slice() {
            [minutes, seconds] => {
                Some(minutes.parse::<f64>().ok()? * 60.0 + seconds.parse::<f64>().ok()?)
            }
            [hours, minutes, seconds] => Some(
                hours.parse::<f64>().ok()? * 3600.0
                    + minutes.parse::<f64>().ok()? * 60.0
                    + seconds.parse::<f64>().ok()?,
            ),
            _ => None,
        };
    }
    value.parse::<f64>().ok()
}

fn smil_clip_attribute(event: &BytesStart<'_>, preferred: &[u8], alternate: &[u8]) -> Option<f64> {
    xml_attribute(event, preferred)
        .or_else(|| xml_attribute(event, alternate))
        .and_then(|value| parse_smil_clock_value(&value))
}

fn register_smil_playback_text(
    event: &BytesStart<'_>,
    smil_path: &str,
    current: &mut SmilPlaybackPar,
) {
    if let Some(id) = xml_attribute(event, b"id") {
        current.ids.push(format!("{smil_path}#{id}"));
    }
    if let Some(src) = xml_attribute(event, b"src") {
        current.text_target = Some(resolve_relative_target(smil_path, &src));
    }
}

fn register_smil_playback_audio(
    event: &BytesStart<'_>,
    smil_path: &str,
    current: &mut SmilPlaybackPar,
) {
    if let Some(id) = xml_attribute(event, b"id") {
        current.ids.push(format!("{smil_path}#{id}"));
    }
    let Some(src) = xml_attribute(event, b"src") else {
        return;
    };
    let source = resolve_relative_target(smil_path, &src);
    let (source, _fragment) = split_target(&source);
    let clip_begin_secs = smil_clip_attribute(event, b"clipBegin", b"clip-begin")
        .unwrap_or(0.0)
        .max(0.0);
    let clip_end_secs =
        smil_clip_attribute(event, b"clipEnd", b"clip-end").filter(|end| *end > clip_begin_secs);
    current.segments.push(DaisyAudioSegment {
        source: source.to_string(),
        clip_begin_secs,
        clip_end_secs,
    });
}

fn parse_smil_playback_document(markup: &str, smil_path: &str) -> Vec<SmilPlaybackPar> {
    let normalized_smil = normalize_epub_internal_path(smil_path);
    let mut reader = XmlReader::from_str(markup);
    reader.config_mut().trim_text(true);
    let mut buf = Vec::new();
    let mut current: Option<SmilPlaybackPar> = None;
    let mut output = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(event)) => match xml_local_name(event.name().as_ref()) {
                b"par" => {
                    if let Some(previous) = current.take()
                        && !previous.segments.is_empty()
                    {
                        output.push(previous);
                    }
                    let mut par = SmilPlaybackPar::default();
                    if let Some(id) = xml_attribute(&event, b"id") {
                        par.ids.push(format!("{normalized_smil}#{id}"));
                    }
                    current = Some(par);
                }
                b"text" => {
                    let par = current.get_or_insert_with(SmilPlaybackPar::default);
                    register_smil_playback_text(&event, &normalized_smil, par);
                }
                b"audio" => {
                    let par = current.get_or_insert_with(SmilPlaybackPar::default);
                    register_smil_playback_audio(&event, &normalized_smil, par);
                }
                _ => {}
            },
            Ok(Event::Empty(event)) => match xml_local_name(event.name().as_ref()) {
                b"text" => {
                    let par = current.get_or_insert_with(SmilPlaybackPar::default);
                    register_smil_playback_text(&event, &normalized_smil, par);
                }
                b"audio" => {
                    let had_par = current.is_some();
                    let par = current.get_or_insert_with(SmilPlaybackPar::default);
                    register_smil_playback_audio(&event, &normalized_smil, par);
                    if !had_par
                        && let Some(standalone) = current.take()
                        && !standalone.segments.is_empty()
                    {
                        output.push(standalone);
                    }
                }
                _ => {}
            },
            Ok(Event::End(event)) if xml_local_name(event.name().as_ref()) == b"par" => {
                if let Some(par) = current.take()
                    && !par.segments.is_empty()
                {
                    output.push(par);
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
        buf.clear();
    }
    if let Some(par) = current
        && !par.segments.is_empty()
    {
        output.push(par);
    }
    output
}

fn flatten_raw_navigation(
    entries: &[RawNavEntry],
    depth: usize,
    output: &mut Vec<(String, String, usize)>,
) {
    for entry in entries {
        output.push((entry.title.clone(), normalize_target(&entry.target), depth));
        flatten_raw_navigation(&entry.children, depth.saturating_add(1), output);
    }
}

fn raw_navigation_to_playback_index(
    entries: &[RawNavEntry],
    chapter_cursor: &mut usize,
) -> Vec<EpubIndexEntry> {
    entries
        .iter()
        .map(|entry| {
            let chapter_index = *chapter_cursor;
            *chapter_cursor += 1;
            let children = raw_navigation_to_playback_index(&entry.children, chapter_cursor);
            EpubIndexEntry {
                title: entry.title.clone(),
                target_utf16: chapter_index.min(i32::MAX as usize) as i32,
                children,
            }
        })
        .collect()
}

fn daisy_playback_navigation(
    resources: &DaisyResources,
    language: Language,
) -> Result<Vec<RawNavEntry>, String> {
    if let Some(ncc_path) = preferred_resource(resources, "ncc.html") {
        let markup = resource_text(resources, &ncc_path, language)?;
        return Ok(build_navigation_tree(&parse_ncc_navigation(
            &markup, &ncc_path,
        )));
    }
    let hinted = resources.entry_hint.as_deref().unwrap_or_default();
    let ncx_path = if hinted.to_ascii_lowercase().ends_with(".ncx") {
        Some(hinted.to_string())
    } else if hinted.to_ascii_lowercase().ends_with(".opf") {
        let markup = resource_text(resources, hinted, language)?;
        parse_daisy_opf(&markup, hinted).ncx
    } else if let Some(opf) = first_resource_with_extension(resources, "opf") {
        let markup = resource_text(resources, &opf, language)?;
        parse_daisy_opf(&markup, &opf).ncx
    } else {
        first_resource_with_extension(resources, "ncx")
    };
    if let Some(ncx_path) = ncx_path
        && let Ok(markup) = resource_text(resources, &ncx_path, language)
    {
        return Ok(parse_ncx_navigation(&markup, &ncx_path));
    }
    if hinted.to_ascii_lowercase().ends_with(".smil") {
        return Ok(vec![RawNavEntry {
            title: Path::new(hinted)
                .file_stem()
                .and_then(|value| value.to_str())
                .unwrap_or("DAISY")
                .to_string(),
            target: normalize_target(hinted),
            children: Vec::new(),
        }]);
    }
    Ok(Vec::new())
}

pub(crate) fn read_daisy_playback_catalog(
    path: &Path,
    language: Language,
) -> Result<DaisyPlaybackCatalog, String> {
    let resources = load_daisy_resources(path, language)?;
    let mut navigation = daisy_playback_navigation(&resources, language)?;
    let mut flat_navigation = Vec::new();
    flatten_raw_navigation(&navigation, 0, &mut flat_navigation);

    let mut ordered_smil_paths = Vec::new();
    for (_title, target, _depth) in &flat_navigation {
        let (target_path, _fragment) = split_target(target);
        if target_path.to_ascii_lowercase().ends_with(".smil")
            && !ordered_smil_paths
                .iter()
                .any(|path: &String| path.eq_ignore_ascii_case(target_path))
        {
            ordered_smil_paths.push(target_path.to_string());
        }
    }
    let mut remaining_smil: Vec<_> = resources
        .files
        .keys()
        .filter(|resource| resource.to_ascii_lowercase().ends_with(".smil"))
        .cloned()
        .collect();
    remaining_smil.sort();
    for smil_path in remaining_smil {
        if !ordered_smil_paths
            .iter()
            .any(|path| path.eq_ignore_ascii_case(&smil_path))
        {
            ordered_smil_paths.push(smil_path);
        }
    }

    let mut pars = Vec::new();
    let mut target_to_par = HashMap::<String, usize>::new();
    for smil_path in &ordered_smil_paths {
        let Ok(markup) = resource_text(&resources, smil_path, language) else {
            continue;
        };
        let parsed = parse_smil_playback_document(&markup, smil_path);
        let first_index = pars.len();
        for par in parsed {
            let par_index = pars.len();
            for id in &par.ids {
                target_to_par.insert(normalize_target(id), par_index);
            }
            if let Some(text_target) = par.text_target.as_deref() {
                target_to_par.insert(normalize_target(text_target), par_index);
                let (text_path, _fragment) = split_target(text_target);
                target_to_par
                    .entry(normalize_target(text_path))
                    .or_insert(par_index);
            }
            pars.push(par);
        }
        if pars.len() > first_index {
            target_to_par
                .entry(normalize_target(smil_path))
                .or_insert(first_index);
        }
    }

    if pars.is_empty() {
        return Ok(DaisyPlaybackCatalog {
            index: Vec::new(),
            chapters: Vec::new(),
        });
    }

    if flat_navigation.is_empty() {
        navigation = ordered_smil_paths
            .iter()
            .enumerate()
            .filter_map(|(index, smil_path)| {
                let target = normalize_target(smil_path);
                target_to_par.get(&target).map(|_| RawNavEntry {
                    title: format!("Chapter {}", index + 1),
                    target,
                    children: Vec::new(),
                })
            })
            .collect();
        flat_navigation.clear();
        flatten_raw_navigation(&navigation, 0, &mut flat_navigation);
    }

    let mut starts: Vec<Option<usize>> = flat_navigation
        .iter()
        .map(|(_title, target, _depth)| {
            let normalized = normalize_target(target);
            target_to_par.get(&normalized).copied().or_else(|| {
                let (target_path, _fragment) = split_target(&normalized);
                target_to_par.get(&normalize_target(target_path)).copied()
            })
        })
        .collect();
    for index in 0..starts.len() {
        if starts[index].is_some() {
            continue;
        }
        let depth = flat_navigation[index].2;
        starts[index] = (index + 1..starts.len())
            .take_while(|candidate| flat_navigation[*candidate].2 > depth)
            .find_map(|candidate| starts[candidate]);
    }

    let mut chapters = Vec::with_capacity(flat_navigation.len());
    for (index, (title, _target, depth)) in flat_navigation.iter().enumerate() {
        let Some(start) = starts[index] else {
            chapters.push(DaisyPlaybackChapter {
                title: title.clone(),
                segments: Vec::new(),
                playback_start: None,
                playback_segments: Vec::new(),
            });
            continue;
        };
        let end = starts
            .iter()
            .enumerate()
            .skip(index + 1)
            .find_map(|(candidate_index, candidate)| {
                let candidate = (*candidate)?;
                let candidate_depth = flat_navigation[candidate_index].2;
                (candidate > start && candidate_depth <= *depth).then_some(candidate)
            })
            .unwrap_or(pars.len());
        let segments = pars[start..end]
            .iter()
            .flat_map(|par| par.segments.iter().cloned())
            .collect();
        let playback_end = starts
            .iter()
            .flatten()
            .copied()
            .filter(|candidate| *candidate > start)
            .min()
            .unwrap_or(pars.len());
        let playback_segments = pars[start..playback_end]
            .iter()
            .flat_map(|par| par.segments.iter().cloned())
            .collect();
        chapters.push(DaisyPlaybackChapter {
            title: title.clone(),
            segments,
            playback_start: Some(start),
            playback_segments,
        });
    }

    let mut chapter_cursor = 0usize;
    let index = raw_navigation_to_playback_index(&navigation, &mut chapter_cursor);
    Ok(DaisyPlaybackCatalog { index, chapters })
}

fn daisy_archive_cache_root(path: &Path) -> PathBuf {
    let mut hasher = DefaultHasher::new();
    path.to_string_lossy()
        .to_ascii_lowercase()
        .hash(&mut hasher);
    if let Ok(metadata) = std::fs::metadata(path) {
        metadata.len().hash(&mut hasher);
        if let Ok(modified) = metadata.modified()
            && let Ok(duration) = modified.duration_since(std::time::UNIX_EPOCH)
        {
            duration.as_secs().hash(&mut hasher);
        }
    }
    std::env::temp_dir()
        .join("Sonarpad")
        .join("daisy")
        .join(format!("{:016x}", hasher.finish()))
}

pub(crate) fn materialize_daisy_audio(
    daisy_path: &Path,
    source: &str,
    language: Language,
) -> Result<PathBuf, String> {
    let normalized = normalize_epub_internal_path(source);
    if normalized.is_empty() || normalized.split('/').any(|part| part == "..") {
        return Err(error_open_file_message(
            language,
            "Invalid DAISY audio path.",
        ));
    }
    let extension = daisy_path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default();
    if extension.eq_ignore_ascii_case("zip") || extension.eq_ignore_ascii_case("daisy") {
        let file =
            File::open(daisy_path).map_err(|error| error_open_file_message(language, error))?;
        let mut archive = ZipArchive::new(file)
            .map_err(|error| error_open_file_message(language, format!("DAISY ZIP: {error}")))?;
        let mut found = None;
        for index in 0..archive.len() {
            let entry = archive.by_index(index).map_err(|error| {
                error_open_file_message(language, format!("DAISY ZIP entry: {error}"))
            })?;
            let entry_name = normalize_epub_internal_path(entry.name());
            if entry_name.eq_ignore_ascii_case(&normalized) {
                found = Some((index, entry.size()));
                break;
            }
        }
        let Some((entry_index, expected_size)) = found else {
            return Err(error_open_file_message(
                language,
                format!("DAISY audio resource not found: {normalized}"),
            ));
        };
        let output_path = daisy_archive_cache_root(daisy_path).join(
            normalized
                .split('/')
                .filter(|part| !part.is_empty() && *part != ".")
                .collect::<PathBuf>(),
        );
        if output_path
            .metadata()
            .map(|metadata| metadata.is_file() && metadata.len() == expected_size)
            .unwrap_or(false)
        {
            return Ok(output_path);
        }
        if let Some(parent) = output_path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|error| error_open_file_message(language, error))?;
        }
        let mut entry = archive.by_index(entry_index).map_err(|error| {
            error_open_file_message(language, format!("DAISY ZIP entry: {error}"))
        })?;
        let mut output =
            File::create(&output_path).map_err(|error| error_open_file_message(language, error))?;
        std::io::copy(&mut entry, &mut output)
            .map_err(|error| error_open_file_message(language, error))?;
        return Ok(output_path);
    }

    let root = daisy_path.parent().unwrap_or_else(|| Path::new("."));
    let candidate = normalized
        .split('/')
        .filter(|part| !part.is_empty() && *part != ".")
        .fold(root.to_path_buf(), |path, part| path.join(part));
    if candidate.is_file() {
        return Ok(candidate);
    }
    Err(error_open_file_message(
        language,
        format!("DAISY audio resource not found: {normalized}"),
    ))
}

fn load_daisy_resources(path: &Path, language: Language) -> Result<DaisyResources, String> {
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default();
    if extension.eq_ignore_ascii_case("zip") || extension.eq_ignore_ascii_case("daisy") {
        return load_daisy_archive(path, language);
    }
    load_daisy_directory(path, language)
}

fn load_daisy_archive(path: &Path, language: Language) -> Result<DaisyResources, String> {
    let file = File::open(path).map_err(|error| error_open_file_message(language, error))?;
    let mut archive = ZipArchive::new(file)
        .map_err(|error| error_open_file_message(language, format!("DAISY ZIP: {error}")))?;
    let mut resources = DaisyResources::default();
    for index in 0..archive.len().min(MAX_DAISY_RESOURCES) {
        let mut entry = archive.by_index(index).map_err(|error| {
            error_open_file_message(language, format!("DAISY ZIP entry: {error}"))
        })?;
        if entry.is_dir() || entry.size() > MAX_DAISY_TEXT_RESOURCE_BYTES {
            continue;
        }
        let normalized = normalize_epub_internal_path(entry.name());
        if !is_daisy_text_resource(&normalized) {
            continue;
        }
        let mut bytes = Vec::with_capacity(entry.size().min(1024 * 1024) as usize);
        entry
            .read_to_end(&mut bytes)
            .map_err(|error| error_open_file_message(language, error))?;
        resources.files.insert(normalized, bytes);
    }
    if resources.files.is_empty() {
        return Err(error_open_file_message(
            language,
            "The archive contains no DAISY text/control resources.",
        ));
    }
    Ok(resources)
}

fn load_daisy_directory(path: &Path, language: Language) -> Result<DaisyResources, String> {
    let root = path.parent().unwrap_or_else(|| Path::new("."));
    let mut resources = DaisyResources {
        files: HashMap::new(),
        entry_hint: path
            .file_name()
            .and_then(|value| value.to_str())
            .map(normalize_epub_internal_path),
    };
    collect_directory_resources(root, root, &mut resources.files, language)?;
    Ok(resources)
}

fn collect_directory_resources(
    root: &Path,
    directory: &Path,
    output: &mut HashMap<String, Vec<u8>>,
    language: Language,
) -> Result<(), String> {
    if output.len() >= MAX_DAISY_RESOURCES {
        return Ok(());
    }
    let entries =
        std::fs::read_dir(directory).map_err(|error| error_open_file_message(language, error))?;
    for entry in entries {
        if output.len() >= MAX_DAISY_RESOURCES {
            break;
        }
        let entry = entry.map_err(|error| error_open_file_message(language, error))?;
        let path = entry.path();
        let file_type = entry
            .file_type()
            .map_err(|error| error_open_file_message(language, error))?;
        if file_type.is_dir() {
            collect_directory_resources(root, &path, output, language)?;
            continue;
        }
        if !file_type.is_file() {
            continue;
        }
        let relative = path.strip_prefix(root).unwrap_or(&path);
        let normalized = normalize_epub_internal_path(&relative.to_string_lossy());
        if !is_daisy_text_resource(&normalized) {
            continue;
        }
        let metadata = entry
            .metadata()
            .map_err(|error| error_open_file_message(language, error))?;
        if metadata.len() > MAX_DAISY_TEXT_RESOURCE_BYTES {
            continue;
        }
        let bytes =
            std::fs::read(&path).map_err(|error| error_open_file_message(language, error))?;
        output.insert(normalized, bytes);
    }
    Ok(())
}

fn is_daisy_text_resource(path: &str) -> bool {
    let lower = path.to_ascii_lowercase();
    lower.ends_with(".html")
        || lower.ends_with(".htm")
        || lower.ends_with(".xhtml")
        || lower.ends_with(".xml")
        || lower.ends_with(".opf")
        || lower.ends_with(".ncx")
        || lower.ends_with(".smil")
}

fn resource_text(
    resources: &DaisyResources,
    requested: &str,
    language: Language,
) -> Result<String, String> {
    let Some(key) = find_resource_key(resources, requested) else {
        return Err(error_open_file_message(
            language,
            format!("DAISY resource not found: {requested}"),
        ));
    };
    let Some(bytes) = resources.files.get(&key) else {
        return Err(error_open_file_message(
            language,
            format!("DAISY resource not found: {requested}"),
        ));
    };
    Ok(decode_ebook_markup(bytes, language))
}

fn find_resource_key(resources: &DaisyResources, requested: &str) -> Option<String> {
    let (path_part, _fragment) = split_target(requested);
    let normalized = normalize_epub_internal_path(&percent_decode_epub_component(path_part));
    if resources.files.contains_key(&normalized) {
        return Some(normalized);
    }
    resources.files.keys().find_map(|path| {
        (path.eq_ignore_ascii_case(&normalized)
            || path.ends_with(&format!("/{normalized}"))
            || normalized.ends_with(&format!("/{path}")))
        .then(|| path.clone())
    })
}

fn preferred_resource(resources: &DaisyResources, file_name: &str) -> Option<String> {
    if let Some(hint) = resources.entry_hint.as_deref()
        && hint
            .rsplit('/')
            .next()
            .is_some_and(|name| name.eq_ignore_ascii_case(file_name))
    {
        return find_resource_key(resources, hint);
    }
    resources.files.keys().find_map(|path| {
        path.rsplit('/')
            .next()
            .is_some_and(|name| name.eq_ignore_ascii_case(file_name))
            .then(|| path.clone())
    })
}

fn first_resource_with_extension(resources: &DaisyResources, extension: &str) -> Option<String> {
    resources.files.keys().find_map(|path| {
        path.rsplit('.')
            .next()
            .is_some_and(|value| value.eq_ignore_ascii_case(extension))
            .then(|| path.clone())
    })
}

fn find_dtbook_resource(resources: &DaisyResources, language: Language) -> Option<String> {
    resources.files.iter().find_map(|(path, bytes)| {
        if !path.to_ascii_lowercase().ends_with(".xml") {
            return None;
        }
        let text = decode_ebook_markup(bytes, language);
        text.to_ascii_lowercase()
            .contains("<dtbook")
            .then(|| path.clone())
    })
}

fn decode_ebook_markup(bytes: &[u8], language: Language) -> String {
    std::str::from_utf8(bytes)
        .map(ToOwned::to_owned)
        .unwrap_or_else(|_| decode_ansi_best_effort(bytes, language))
}

fn resolve_relative_target(base_path: &str, href: &str) -> String {
    let (href_path, fragment) = split_target(href);
    let href_path = href_path.split('?').next().unwrap_or(href_path);
    let decoded = percent_decode_epub_component(href_path);
    let base = Path::new(base_path)
        .parent()
        .unwrap_or_else(|| Path::new(""));
    let joined = if decoded.trim().is_empty() {
        PathBuf::from(base_path)
    } else {
        base.join(decoded)
    };
    let normalized = normalize_epub_internal_path(&joined.to_string_lossy());
    if fragment.is_empty() {
        normalized
    } else {
        format!("{}#{}", normalized, percent_decode_epub_component(fragment))
    }
}

fn normalize_target(target: &str) -> String {
    let (path_part, fragment) = split_target(target);
    let path_part = normalize_epub_internal_path(&percent_decode_epub_component(path_part));
    if fragment.is_empty() {
        path_part
    } else {
        format!("{}#{}", path_part, percent_decode_epub_component(fragment))
    }
}

fn split_target(target: &str) -> (&str, &str) {
    target.split_once('#').unwrap_or((target, ""))
}

fn push_unique_target(output: &mut Vec<String>, target: &str) {
    let normalized = normalize_target(target);
    let (path_part, _fragment) = split_target(&normalized);
    if path_part.is_empty() {
        return;
    }
    if !output.iter().any(|existing| {
        let (existing_path, _fragment) = split_target(existing);
        existing_path.eq_ignore_ascii_case(path_part)
    }) {
        output.push(normalized);
    }
}

fn xml_local_name(name: &[u8]) -> &[u8] {
    name.rsplit(|byte| *byte == b':').next().unwrap_or(name)
}

fn xml_attribute(event: &BytesStart<'_>, requested_name: &[u8]) -> Option<String> {
    event.attributes().flatten().find_map(|attribute| {
        (xml_local_name(attribute.key.as_ref()).eq_ignore_ascii_case(requested_name))
            .then(|| String::from_utf8_lossy(attribute.value.as_ref()).into_owned())
    })
}

fn looks_like_dtbook(path: &Path) -> bool {
    let Ok(mut file) = File::open(path) else {
        return false;
    };
    let mut bytes = vec![0u8; 16 * 1024];
    let Ok(read) = file.read(&mut bytes) else {
        return false;
    };
    bytes.truncate(read);
    String::from_utf8_lossy(&bytes)
        .to_ascii_lowercase()
        .contains("<dtbook")
}

fn zip_looks_like_daisy(path: &Path) -> bool {
    let Ok(file) = File::open(path) else {
        return false;
    };
    let Ok(mut archive) = ZipArchive::new(file) else {
        return false;
    };
    for index in 0..archive.len().min(MAX_DAISY_RESOURCES) {
        let Ok(entry) = archive.by_index(index) else {
            continue;
        };
        let lower = entry.name().to_ascii_lowercase();
        if lower.ends_with("/ncc.html")
            || lower == "ncc.html"
            || lower.ends_with(".opf")
            || lower.ends_with(".ncx")
        {
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::time::{SystemTime, UNIX_EPOCH};
    use zip::write::FileOptions;

    fn unique_temp_path(extension: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or_default();
        std::env::temp_dir().join(format!("sonarpad-ebook-test-{nonce}.{extension}"))
    }

    fn unique_temp_dir() -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or_default();
        std::env::temp_dir().join(format!("sonarpad-ebook-dir-{nonce}"))
    }

    fn palmdoc_literal_encode(input: &[u8]) -> Vec<u8> {
        let mut output = Vec::new();
        for chunk in input.chunks(8) {
            output.push(chunk.len() as u8);
            output.extend_from_slice(chunk);
        }
        output
    }

    fn make_test_huff_record(mapping: &[(u8, usize)]) -> Vec<u8> {
        const TABLE1_OFFSET: usize = 24;
        const TABLE2_OFFSET: usize = TABLE1_OFFSET + (256 * 4);
        let mut huff = vec![0u8; TABLE2_OFFSET + (64 * 4)];
        huff[..8].copy_from_slice(b"HUFF\0\0\0\x18");
        huff[8..12].copy_from_slice(&(TABLE1_OFFSET as u32).to_be_bytes());
        huff[12..16].copy_from_slice(&(TABLE2_OFFSET as u32).to_be_bytes());

        let mut mapped = [0usize; 256];
        for &(code, phrase_index) in mapping {
            mapped[usize::from(code)] = phrase_index;
        }
        for (code, phrase_index) in mapped.into_iter().enumerate() {
            let max_base = code
                .checked_add(phrase_index)
                .unwrap_or_else(|| panic!("test HUFF mapping overflow"));
            assert!(max_base <= 0x00ff_ffff);
            let value = ((max_base as u32) << 8) | 0x88;
            let offset = TABLE1_OFFSET + code * 4;
            huff[offset..offset + 4].copy_from_slice(&value.to_be_bytes());
        }
        huff
    }

    fn make_test_cdic_record(phrases: &[(&[u8], bool)], bits: u32) -> Vec<u8> {
        let table_len = phrases.len() * 2;
        let mut cdic = vec![0u8; 16 + table_len];
        cdic[..8].copy_from_slice(b"CDIC\0\0\0\x10");
        cdic[8..12].copy_from_slice(&(phrases.len() as u32).to_be_bytes());
        cdic[12..16].copy_from_slice(&bits.to_be_bytes());
        for (index, (phrase, terminal)) in phrases.iter().enumerate() {
            let relative = cdic.len() - 16;
            assert!(relative <= usize::from(u16::MAX));
            cdic[16 + index * 2..18 + index * 2].copy_from_slice(&(relative as u16).to_be_bytes());
            let length = u16::try_from(phrase.len())
                .unwrap_or_else(|_| panic!("test CDIC phrase is too large"));
            let length_and_flag = length | if *terminal { 0x8000 } else { 0 };
            cdic.extend_from_slice(&length_and_flag.to_be_bytes());
            cdic.extend_from_slice(phrase);
        }
        cdic
    }

    fn make_kindle_fixture(compression: u16, mobi_version: u32, encryption: u16) -> Vec<u8> {
        const PDB_HEADER_LEN: usize = 78;
        const RECORD_ENTRY_LEN: usize = 8;
        const TITLE_OFFSET: usize = 256;
        let html = b"<html><body><h1 id=\"chapter-one\">Chapter One</h1><p>Kindle fixture body.</p><h2 id=\"chapter-two\">Chapter Two</h2><p>Second section.</p></body></html>";
        let title = b"Sonarpad Kindle Fixture";

        let (text_record, extra_records): (Vec<u8>, Vec<Vec<u8>>) = match compression {
            MOBI_COMPRESSION_NONE => (html.to_vec(), Vec::new()),
            MOBI_COMPRESSION_PALMDOC => (palmdoc_literal_encode(html), Vec::new()),
            MOBI_COMPRESSION_HUFF_CDIC => {
                let phrases: Vec<(&[u8], bool)> = vec![
                    (&html[..48], true),
                    (&html[48..96], true),
                    (&html[96..], true),
                ];
                let huff = make_test_huff_record(&[(0, 0), (1, 1), (2, 2)]);
                let cdic = make_test_cdic_record(&phrases, 2);
                (vec![0, 1, 2], vec![huff, cdic])
            }
            other => panic!("unsupported test compression {other}"),
        };

        let mut record_zero = vec![0u8; TITLE_OFFSET + title.len()];
        record_zero[0..2].copy_from_slice(&compression.to_be_bytes());
        record_zero[4..8].copy_from_slice(&(html.len() as u32).to_be_bytes());
        record_zero[8..10].copy_from_slice(&1u16.to_be_bytes());
        record_zero[10..12].copy_from_slice(&4096u16.to_be_bytes());
        record_zero[12..14].copy_from_slice(&encryption.to_be_bytes());
        record_zero[16..20].copy_from_slice(b"MOBI");
        record_zero[20..24].copy_from_slice(&232u32.to_be_bytes());
        record_zero[28..32].copy_from_slice(&65001u32.to_be_bytes());
        record_zero[0x54..0x58].copy_from_slice(&(TITLE_OFFSET as u32).to_be_bytes());
        record_zero[0x58..0x5c].copy_from_slice(&(title.len() as u32).to_be_bytes());
        record_zero[0x68..0x6c].copy_from_slice(&mobi_version.to_be_bytes());
        if compression == MOBI_COMPRESSION_HUFF_CDIC {
            record_zero[0x70..0x74].copy_from_slice(&2u32.to_be_bytes());
            record_zero[0x74..0x78].copy_from_slice(&2u32.to_be_bytes());
        }
        record_zero[TITLE_OFFSET..TITLE_OFFSET + title.len()].copy_from_slice(title);

        let mut records = vec![record_zero, text_record];
        records.extend(extra_records);
        let record_count = records.len();
        let first_record_offset = PDB_HEADER_LEN + record_count * RECORD_ENTRY_LEN;
        let mut bytes = vec![0u8; first_record_offset];
        let pdb_name = b"Sonarpad_Kindle_Fixture";
        bytes[..pdb_name.len()].copy_from_slice(pdb_name);
        bytes[60..68].copy_from_slice(b"BOOKMOBI");
        bytes[76..78].copy_from_slice(&(record_count as u16).to_be_bytes());

        let mut next_offset = first_record_offset;
        for (index, record) in records.iter().enumerate() {
            let table_offset = 78 + index * RECORD_ENTRY_LEN;
            bytes[table_offset..table_offset + 4]
                .copy_from_slice(&(next_offset as u32).to_be_bytes());
            next_offset += record.len();
        }
        for record in records {
            bytes.extend_from_slice(&record);
        }
        bytes
    }

    fn write_zip(path: &Path, files: &[(&str, &str)]) {
        let file = File::create(path).unwrap_or_else(|error| panic!("create test ZIP: {error}"));
        let mut writer = zip::ZipWriter::new(file);
        let options = FileOptions::default();
        for (name, content) in files {
            writer
                .start_file(*name, options)
                .unwrap_or_else(|error| panic!("start ZIP test file: {error}"));
            writer
                .write_all(content.as_bytes())
                .unwrap_or_else(|error| panic!("write ZIP test file: {error}"));
        }
        writer
            .finish()
            .unwrap_or_else(|error| panic!("finish test ZIP: {error}"));
    }

    #[test]
    fn kindle_extensions_include_mobi_azw_and_azw3_case_insensitively() {
        assert!(is_kindle_path(Path::new("book.mobi")));
        assert!(is_kindle_path(Path::new("book.AZW")));
        assert!(is_kindle_path(Path::new("book.AzW3")));
        assert!(!is_kindle_path(Path::new("book.epub")));
    }

    #[test]
    fn palmdoc_decoder_covers_literal_runs_and_back_references() {
        let literal_run = palmdoc_literal_encode(b"literal PalmDOC data");
        assert_eq!(
            palmdoc_decompress(&literal_run).unwrap_or_else(|error| panic!("PalmDOC: {error}")),
            b"literal PalmDOC data"
        );
        let back_reference = [b'a', b'b', b'c', 0x80, 0x18];
        assert_eq!(
            palmdoc_decompress(&back_reference)
                .unwrap_or_else(|error| panic!("PalmDOC back-reference: {error}")),
            b"abcabc"
        );
    }

    #[test]
    fn kindle_import_covers_mobi_azw_azw3_uncompressed_and_palmdoc() {
        for extension in ["mobi", "azw", "azw3"] {
            for compression in [MOBI_COMPRESSION_NONE, MOBI_COMPRESSION_PALMDOC] {
                for mobi_version in [6u32, 8u32] {
                    let path = unique_temp_path(extension);
                    std::fs::write(&path, make_kindle_fixture(compression, mobi_version, 0))
                        .unwrap_or_else(|error| panic!("write Kindle fixture: {error}"));
                    let document = read_kindle_document(&path, Language::English).unwrap_or_else(
                        |error| {
                            panic!(
                                "read {extension} compression={compression} MOBI={mobi_version}: {error}"
                            )
                        },
                    );
                    let _removed = std::fs::remove_file(&path);

                    assert!(document.text.contains("Kindle fixture body."));
                    assert!(document.text.contains("Second section."));
                    assert!(
                        document
                            .index
                            .iter()
                            .any(|entry| entry.title == "Chapter One")
                    );
                }
            }
        }
    }

    #[test]
    fn huff_cdic_fixture_is_decoded_as_real_text() {
        let path = unique_temp_path("mobi");
        std::fs::write(&path, make_kindle_fixture(MOBI_COMPRESSION_HUFF_CDIC, 6, 0))
            .unwrap_or_else(|error| panic!("write HUFF/CDIC fixture: {error}"));
        let document = read_kindle_document(&path, Language::English)
            .unwrap_or_else(|error| panic!("read HUFF/CDIC fixture: {error}"));
        let _removed = std::fs::remove_file(&path);

        assert!(document.text.contains("Kindle fixture body."));
        assert!(document.text.contains("Second section."));
        assert!(
            document
                .index
                .iter()
                .any(|entry| entry.title == "Chapter One")
        );
    }

    #[test]
    fn huff_cdic_decoder_expands_recursive_dictionary_phrases() {
        let huff = make_test_huff_record(&[(0, 0), (1, 1)]);
        let phrases: Vec<(&[u8], bool)> = vec![(b"recursive phrase", true), (&[0], false)];
        let cdic = make_test_cdic_record(&phrases, 1);
        let mut decoder = HuffCdicDecoder::from_records(&huff, &[cdic.as_slice()])
            .unwrap_or_else(|error| panic!("create HUFF/CDIC decoder: {error}"));
        assert_eq!(
            decoder
                .decode_stream(&[1])
                .unwrap_or_else(|error| panic!("decode recursive phrase: {error}")),
            b"recursive phrase"
        );
    }

    #[test]
    fn kindle_drm_is_rejected_before_any_text_parser() {
        for extension in ["mobi", "azw", "azw3"] {
            let path = unique_temp_path(extension);
            std::fs::write(&path, make_kindle_fixture(MOBI_COMPRESSION_PALMDOC, 8, 2))
                .unwrap_or_else(|error| panic!("write DRM fixture: {error}"));
            let error = match read_kindle_document(&path, Language::English) {
                Ok(_) => panic!("DRM fixture must be rejected"),
                Err(error) => error,
            };
            let _removed = std::fs::remove_file(&path);
            assert!(error.to_ascii_lowercase().contains("drm"));
        }
    }

    #[test]
    fn unsupported_mobi_compression_fails_cleanly() {
        let bytes = make_kindle_fixture(MOBI_COMPRESSION_NONE, 6, 0);
        let mut bytes = bytes;
        let offsets = parse_pdb_record_offsets(&bytes)
            .unwrap_or_else(|error| panic!("parse test record table: {error}"));
        let record_zero_offset = offsets[0];
        bytes[record_zero_offset..record_zero_offset + 2].copy_from_slice(&99u16.to_be_bytes());
        let header = match parse_classic_mobi_header(&bytes)
            .unwrap_or_else(|error| panic!("parse test header: {error}"))
        {
            Some(header) => header,
            None => panic!("classic header fixture was not recognized"),
        };
        let error = match decode_classic_mobi_markup(&bytes, &header) {
            Ok(_) => panic!("unknown compression must fail"),
            Err(error) => error,
        };
        assert!(error.contains("unsupported MOBI compression"));
    }

    #[test]
    fn daisy_202_zip_reads_text_navigation_and_smil_targets() {
        let path = unique_temp_path("daisy");
        write_zip(
            &path,
            &[
                (
                    "ncc.html",
                    r#"<html><body><h1><a href="ch1.smil#p1">Chapter One</a></h1><h2><a href="ch2.smil#p2">Chapter Two</a></h2></body></html>"#,
                ),
                (
                    "ch1.smil",
                    r#"<smil><body><seq><par id="p1"><text src="text/ch1.html#start"/><audio src="audio/ch1.mp3"/></par></seq></body></smil>"#,
                ),
                (
                    "ch2.smil",
                    r#"<smil><body><seq><par id="p2"><text src="text/ch2.html#start"/></par></seq></body></smil>"#,
                ),
                (
                    "text/ch1.html",
                    r#"<html><body><h1 id="start">Chapter One</h1><p>First body.</p></body></html>"#,
                ),
                (
                    "text/ch2.html",
                    r#"<html><body><h2 id="start">Chapter Two</h2><p>Second body.</p></body></html>"#,
                ),
            ],
        );

        let document = read_daisy_document(&path, Language::English)
            .unwrap_or_else(|error| panic!("read DAISY 2.02 fixture: {error}"));
        let _removed = std::fs::remove_file(&path);

        assert!(document.text.contains("First body."));
        assert!(document.text.contains("Second body."));
        assert_eq!(
            document.index.first().map(|entry| entry.title.as_str()),
            Some("Chapter One")
        );
        assert!(document.index.first().is_some_and(|entry| {
            entry.children.first().map(|child| child.title.as_str()) == Some("Chapter Two")
        }));
    }

    #[test]
    fn daisy_3_zip_reads_opf_spine_ncx_and_dtbook() {
        let path = unique_temp_path("zip");
        write_zip(
            &path,
            &[
                (
                    "book.opf",
                    r#"<?xml version="1.0"?><package><metadata><dc:title xmlns:dc="http://purl.org/dc/elements/1.1/">Test DAISY 3</dc:title></metadata><manifest><item id="ncx" href="nav.ncx" media-type="application/x-dtbncx+xml"/><item id="c1" href="text/book.xml" media-type="application/x-dtbook+xml"/></manifest><spine toc="ncx"><itemref idref="c1"/></spine></package>"#,
                ),
                (
                    "nav.ncx",
                    r#"<?xml version="1.0"?><ncx><navMap><navPoint id="n1"><navLabel><text>Chapter A</text></navLabel><content src="text/book.xml#c1"/></navPoint></navMap></ncx>"#,
                ),
                (
                    "text/book.xml",
                    r#"<?xml version="1.0"?><dtbook><book><bodymatter><level1><h1 id="c1">Chapter A</h1><p>DAISY three text.</p></level1></bodymatter></book></dtbook>"#,
                ),
            ],
        );

        let document = read_daisy_document(&path, Language::English)
            .unwrap_or_else(|error| panic!("read DAISY 3 fixture: {error}"));
        let _removed = std::fs::remove_file(&path);

        assert!(document.text.contains("Test DAISY 3"));
        assert!(document.text.contains("DAISY three text."));
        assert_eq!(
            document.index.first().map(|entry| entry.title.as_str()),
            Some("Chapter A")
        );
    }

    #[test]
    fn daisy_202_extracted_ncc_reads_adjacent_smil_and_xhtml() {
        let root = unique_temp_dir();
        std::fs::create_dir_all(root.join("text"))
            .unwrap_or_else(|error| panic!("create DAISY 2.02 fixture directory: {error}"));
        std::fs::write(
            root.join("ncc.html"),
            r#"<html><body><h1><a href="chapter.smil#p1">Extracted Chapter</a></h1></body></html>"#,
        )
        .unwrap_or_else(|error| panic!("write NCC fixture: {error}"));
        std::fs::write(
            root.join("chapter.smil"),
            r#"<smil><body><seq><par id="p1"><text src="text/chapter.xhtml#start"/></par></seq></body></smil>"#,
        )
        .unwrap_or_else(|error| panic!("write SMIL fixture: {error}"));
        std::fs::write(
            root.join("text/chapter.xhtml"),
            r#"<html><body><h1 id="start">Extracted Chapter</h1><p>Extracted DAISY 2 text.</p></body></html>"#,
        )
        .unwrap_or_else(|error| panic!("write XHTML fixture: {error}"));

        let document = read_daisy_document(&root.join("ncc.html"), Language::English)
            .unwrap_or_else(|error| panic!("read extracted DAISY 2.02 fixture: {error}"));
        let _removed = std::fs::remove_dir_all(&root);
        assert!(document.text.contains("Extracted DAISY 2 text."));
        assert_eq!(
            document.index.first().map(|entry| entry.title.as_str()),
            Some("Extracted Chapter")
        );
    }

    #[test]
    fn daisy_3_extracted_opf_reads_spine_ncx_and_dtbook() {
        let root = unique_temp_dir();
        std::fs::create_dir_all(root.join("text"))
            .unwrap_or_else(|error| panic!("create DAISY 3 fixture directory: {error}"));
        std::fs::write(
            root.join("book.opf"),
            r#"<package><metadata><dc:title xmlns:dc="http://purl.org/dc/elements/1.1/">Extracted DAISY 3</dc:title></metadata><manifest><item id="nav" href="nav.ncx" media-type="application/x-dtbncx+xml"/><item id="body" href="text/book.xml" media-type="application/x-dtbook+xml"/></manifest><spine toc="nav"><itemref idref="body"/></spine></package>"#,
        )
        .unwrap_or_else(|error| panic!("write OPF fixture: {error}"));
        std::fs::write(
            root.join("nav.ncx"),
            r#"<ncx><navMap><navPoint id="c1"><navLabel><text>Extracted Three</text></navLabel><content src="text/book.xml#c1"/></navPoint></navMap></ncx>"#,
        )
        .unwrap_or_else(|error| panic!("write NCX fixture: {error}"));
        std::fs::write(
            root.join("text/book.xml"),
            r#"<dtbook><book><bodymatter><level1><h1 id="c1">Extracted Three</h1><p>Extracted DAISY 3 text.</p></level1></bodymatter></book></dtbook>"#,
        )
        .unwrap_or_else(|error| panic!("write DTBook fixture: {error}"));

        let document = read_daisy_document(&root.join("book.opf"), Language::English)
            .unwrap_or_else(|error| panic!("read extracted DAISY 3 fixture: {error}"));
        let _removed = std::fs::remove_dir_all(&root);
        assert!(document.text.contains("Extracted DAISY 3 text."));
        assert_eq!(
            document.index.first().map(|entry| entry.title.as_str()),
            Some("Extracted Three")
        );
    }

    #[test]
    fn daisy_direct_ncx_smil_and_dtbook_entry_points_are_readable() {
        let root = unique_temp_dir();
        std::fs::create_dir_all(&root)
            .unwrap_or_else(|error| panic!("create direct DAISY fixture directory: {error}"));
        let dtbook = root.join("book.xml");
        std::fs::write(
            &dtbook,
            r#"<dtbook><book><bodymatter><level1><h1 id="c1">Direct Chapter</h1><p>Direct DTBook text.</p></level1></bodymatter></book></dtbook>"#,
        )
        .unwrap_or_else(|error| panic!("write direct DTBook: {error}"));
        let ncx = root.join("nav.ncx");
        std::fs::write(
            &ncx,
            r#"<ncx><navMap><navPoint id="c1"><navLabel><text>Direct Chapter</text></navLabel><content src="book.xml#c1"/></navPoint></navMap></ncx>"#,
        )
        .unwrap_or_else(|error| panic!("write direct NCX: {error}"));
        let smil = root.join("chapter.smil");
        std::fs::write(
            &smil,
            r#"<smil><body><seq><par id="p1"><text src="book.xml#c1"/></par></seq></body></smil>"#,
        )
        .unwrap_or_else(|error| panic!("write direct SMIL: {error}"));

        let ncx_document = read_daisy_document(&ncx, Language::English)
            .unwrap_or_else(|error| panic!("read direct NCX: {error}"));
        assert!(ncx_document.text.contains("Direct DTBook text."));
        let smil_document = read_daisy_document(&smil, Language::English)
            .unwrap_or_else(|error| panic!("read direct SMIL: {error}"));
        assert!(smil_document.text.contains("Direct DTBook text."));
        let dtbook_document = read_daisy_document(&dtbook, Language::English)
            .unwrap_or_else(|error| panic!("read direct DTBook: {error}"));
        assert!(dtbook_document.text.contains("Direct DTBook text."));
        let _removed = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn daisy_audio_only_uses_navigation_labels_as_editor_text() {
        let path = unique_temp_path("daisy");
        write_zip(
            &path,
            &[
                (
                    "ncc.html",
                    r#"<html><body><h1><a href="audio.smil#p1">Audio Chapter</a></h1></body></html>"#,
                ),
                (
                    "audio.smil",
                    r#"<smil><body><seq><par id="p1"><audio src="audio/chapter.mp3"/></par></seq></body></smil>"#,
                ),
            ],
        );

        let document = read_daisy_document(&path, Language::English)
            .unwrap_or_else(|error| panic!("read audio-only DAISY fixture: {error}"));
        let _removed = std::fs::remove_file(&path);

        assert!(document.text.contains("Audio Chapter"));
        assert_eq!(document.index.len(), 1);
    }

    #[test]
    fn dtbook_xml_is_detected_without_claiming_generic_xml() {
        let dtbook_path = unique_temp_path("xml");
        std::fs::write(&dtbook_path, "<dtbook><book><p>Hello</p></book></dtbook>")
            .unwrap_or_else(|error| panic!("write DTBook fixture: {error}"));
        assert!(is_daisy_path(&dtbook_path));
        let _removed = std::fs::remove_file(&dtbook_path);

        let generic_path = unique_temp_path("xml");
        std::fs::write(&generic_path, "<root><item>Hello</item></root>")
            .unwrap_or_else(|error| panic!("write generic XML fixture: {error}"));
        assert!(!is_daisy_path(&generic_path));
        let _removed = std::fs::remove_file(&generic_path);
    }

    #[test]
    fn zip_detection_requires_daisy_control_files() {
        let path = unique_temp_path("zip");
        write_zip(&path, &[("notes.txt", "not a DAISY book")]);
        assert!(!is_daisy_path(&path));
        let _removed = std::fs::remove_file(&path);
    }

    #[test]
    fn smil_clock_values_support_fractional_npt_and_clock_syntax() {
        assert_eq!(parse_smil_clock_value("npt=12.5s"), Some(12.5));
        assert_eq!(parse_smil_clock_value("250ms"), Some(0.25));
        assert_eq!(parse_smil_clock_value("1.5min"), Some(90.0));
        assert_eq!(parse_smil_clock_value("00:01:02.500"), Some(62.5));
        assert_eq!(parse_smil_clock_value("3.25"), Some(3.25));
    }

    #[test]
    fn daisy_202_playback_catalog_honors_navigation_clip_ranges() {
        let path = unique_temp_path("daisy");
        write_zip(
            &path,
            &[
                (
                    "ncc.html",
                    r#"<html><body><h1><a href="book.smil#p1">Chapter One</a></h1><h1><a href="book.smil#p3">Chapter Two</a></h1></body></html>"#,
                ),
                (
                    "book.smil",
                    r#"<smil><body><seq>
                    <par id="p1"><text src="text.html#a"/><audio src="audio/book.mp3" clipBegin="npt=0s" clipEnd="npt=5s"/></par>
                    <par id="p2"><text src="text.html#b"/><audio src="audio/book.mp3" clipBegin="5s" clipEnd="10s"/></par>
                    <par id="p3"><text src="text.html#c"/><audio src="audio/book.mp3" clipBegin="10s" clipEnd="15.5s"/></par>
                    </seq></body></smil>"#,
                ),
                (
                    "text.html",
                    r#"<html><body><p id="a">A</p><p id="b">B</p><p id="c">C</p></body></html>"#,
                ),
            ],
        );
        let catalog = read_daisy_playback_catalog(&path, Language::English)
            .unwrap_or_else(|error| panic!("read DAISY playback catalog: {error}"));
        let _removed = std::fs::remove_file(&path);

        assert_eq!(catalog.chapters.len(), 2);
        assert_eq!(catalog.chapters[0].segments.len(), 2);
        assert_eq!(catalog.chapters[1].segments.len(), 1);
        assert_eq!(catalog.chapters[1].segments[0].clip_begin_secs, 10.0);
        assert_eq!(catalog.chapters[1].segments[0].clip_end_secs, Some(15.5));
    }

    #[test]
    fn daisy_chapter_can_span_multiple_audio_files() {
        let path = unique_temp_path("daisy");
        write_zip(
            &path,
            &[
                (
                    "ncc.html",
                    r#"<html><body><h1><a href="book.smil#p1">Long Chapter</a></h1></body></html>"#,
                ),
                (
                    "book.smil",
                    r#"<smil><body><seq>
                    <par id="p1"><audio src="audio/one.mp3" clipBegin="1.25s" clipEnd="4.5s"/></par>
                    <par id="p2"><audio src="audio/two.mp3" clipBegin="0s" clipEnd="8s"/></par>
                    </seq></body></smil>"#,
                ),
            ],
        );
        let catalog = read_daisy_playback_catalog(&path, Language::English)
            .unwrap_or_else(|error| panic!("read multi-file DAISY playback catalog: {error}"));
        let _removed = std::fs::remove_file(&path);
        let segments = &catalog.chapters[0].segments;
        assert_eq!(segments.len(), 2);
        assert_eq!(segments[0].source, "audio/one.mp3");
        assert_eq!(segments[1].source, "audio/two.mp3");
    }

    #[test]
    fn daisy_3_ncx_to_smil_builds_playable_chapters() {
        let path = unique_temp_path("daisy");
        write_zip(
            &path,
            &[
                (
                    "book.opf",
                    r#"<package><manifest><item id="nav" href="nav.ncx" media-type="application/x-dtbncx+xml"/><item id="s" href="sync.smil" media-type="application/smil+xml"/></manifest><spine toc="nav"><itemref idref="s"/></spine></package>"#,
                ),
                (
                    "nav.ncx",
                    r#"<ncx><navMap><navPoint><navLabel><text>DAISY Three Audio</text></navLabel><content src="sync.smil#p1"/></navPoint></navMap></ncx>"#,
                ),
                (
                    "sync.smil",
                    r#"<smil><body><par id="p1"><audio src="audio/chapter.mp3" clipBegin="2s" clipEnd="9s"/></par></body></smil>"#,
                ),
            ],
        );
        let catalog = read_daisy_playback_catalog(&path, Language::English)
            .unwrap_or_else(|error| panic!("read DAISY 3 playback catalog: {error}"));
        let _removed = std::fs::remove_file(&path);
        assert_eq!(catalog.chapters.len(), 1);
        assert_eq!(catalog.chapters[0].title, "DAISY Three Audio");
        assert_eq!(catalog.chapters[0].segments[0].clip_begin_secs, 2.0);
    }

    #[test]
    fn materialize_daisy_audio_extracts_archive_resource() {
        let path = unique_temp_path("daisy");
        write_zip(
            &path,
            &[
                (
                    "ncc.html",
                    r#"<html><body><h1><a href="sync.smil#p1">Audio</a></h1></body></html>"#,
                ),
                (
                    "sync.smil",
                    r#"<smil><body><par id="p1"><audio src="audio/chapter.mp3"/></par></body></smil>"#,
                ),
                ("audio/chapter.mp3", "FAKE-MP3-BYTES"),
            ],
        );
        let extracted = materialize_daisy_audio(&path, "audio/chapter.mp3", Language::English)
            .unwrap_or_else(|error| panic!("materialize DAISY audio: {error}"));
        assert_eq!(
            std::fs::read(&extracted).unwrap_or_default(),
            b"FAKE-MP3-BYTES"
        );
        let _removed = std::fs::remove_file(&path);
        if let Some(cache_book_dir) = extracted.parent().and_then(|parent| parent.parent()) {
            let _removed_cache = std::fs::remove_dir_all(cache_book_dir);
        }
    }

    #[test]
    fn materialize_daisy_audio_uses_extracted_book_resource() {
        let root = unique_temp_dir();
        std::fs::create_dir_all(root.join("audio"))
            .unwrap_or_else(|error| panic!("create extracted DAISY audio fixture: {error}"));
        std::fs::write(root.join("ncc.html"), "<html></html>")
            .unwrap_or_else(|error| panic!("write extracted NCC: {error}"));
        std::fs::write(root.join("audio/chapter.mp3"), b"LOCAL-AUDIO")
            .unwrap_or_else(|error| panic!("write extracted DAISY audio: {error}"));
        let materialized = materialize_daisy_audio(
            &root.join("ncc.html"),
            "audio/chapter.mp3",
            Language::English,
        )
        .unwrap_or_else(|error| panic!("resolve extracted DAISY audio: {error}"));
        assert_eq!(materialized, root.join("audio/chapter.mp3"));
        let _removed = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn daisy_parent_chapter_includes_nested_sections_until_next_peer() {
        let path = unique_temp_path("daisy");
        write_zip(
            &path,
            &[
                (
                    "ncc.html",
                    r#"<html><body><h1><a href="book.smil#p1">Chapter One</a></h1><h2><a href="book.smil#p2">Section</a></h2><h1><a href="book.smil#p3">Chapter Two</a></h1></body></html>"#,
                ),
                (
                    "book.smil",
                    r#"<smil><body><seq><par id="p1"><audio src="a.mp3" clipBegin="0s" clipEnd="5s"/></par><par id="p2"><audio src="a.mp3" clipBegin="5s" clipEnd="10s"/></par><par id="p3"><audio src="a.mp3" clipBegin="10s" clipEnd="15s"/></par></seq></body></smil>"#,
                ),
            ],
        );
        let catalog = read_daisy_playback_catalog(&path, Language::English)
            .unwrap_or_else(|error| panic!("read hierarchical DAISY playback catalog: {error}"));
        let _removed = std::fs::remove_file(&path);
        assert_eq!(catalog.chapters.len(), 3);
        assert_eq!(catalog.chapters[0].segments.len(), 2);
        assert_eq!(catalog.chapters[1].segments.len(), 1);
        assert_eq!(catalog.chapters[2].segments.len(), 1);
        assert_eq!(catalog.chapters[0].playback_segments.len(), 1);
        assert_eq!(catalog.chapters[1].playback_segments.len(), 1);
        assert_eq!(catalog.chapters[2].playback_segments.len(), 1);
        assert_eq!(
            catalog
                .chapters
                .iter()
                .map(|c| c.playback_start)
                .collect::<Vec<_>>(),
            vec![Some(0), Some(1), Some(2)]
        );
        assert_eq!(
            catalog
                .chapters
                .iter()
                .flat_map(|c| c.playback_segments.iter())
                .map(|s| s.clip_end_secs.unwrap() - s.clip_begin_secs)
                .sum::<f64>(),
            15.0
        );
    }
}
