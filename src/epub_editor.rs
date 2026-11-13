use crate::file_handler::read_epub_document;
use crate::i18n;
use crate::settings::{Language, error_save_file_message};
use epub::doc::EpubDoc;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs::File;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use zip::write::FileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};

const MAX_EPUB_EDIT_DISTANCE: usize = 4096;
const SONARPAD_LINE_BREAK_CLASS: &str = "sonarpad-preserve-line-break";

type ResourceReplacement = (String, Vec<u8>);
type AppliedBookEdits = (String, Vec<ResourceReplacement>);

#[derive(Clone, Copy, Debug)]
enum HtmlCharSource {
    NodeChar {
        node_index: usize,
        char_index: usize,
    },
    BreakTag {
        source_start: usize,
        source_end: usize,
        preserve_empty: bool,
    },
    Structural,
}

#[derive(Clone, Debug)]
struct HtmlTextNode {
    source_start: usize,
    source_end: usize,
    chars: Vec<char>,
    raw_spans: Vec<(usize, usize)>,
}

#[derive(Clone, Debug)]
struct HtmlExtraction {
    chars: Vec<char>,
    sources: Vec<HtmlCharSource>,
    nodes: Vec<HtmlTextNode>,
}

struct FilteredHtmlExtraction {
    chars: Vec<char>,
    sources: Vec<HtmlCharSource>,
}

#[derive(Clone, Debug)]
struct EditableResource {
    archive_path: String,
    html: String,
    filtered_sources: Vec<HtmlCharSource>,
    nodes: Vec<HtmlTextNode>,
}

#[derive(Clone, Copy, Debug)]
enum BookCharSource {
    Title(usize),
    Resource {
        resource_index: usize,
        local_index: usize,
    },
    Structural,
}

#[derive(Clone, Debug)]
struct EpubEditModel {
    full_text: String,
    full_chars: Vec<char>,
    full_sources: Vec<BookCharSource>,
    title: String,
    resources: Vec<EditableResource>,
    opf_path: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DiffTag {
    Equal,
    Delete,
    Insert,
}

#[derive(Clone, Copy, Debug)]
enum TextEdit {
    Delete { old_index: usize },
    Insert { old_index: usize, ch: char },
}

#[derive(Default)]
struct TextPlan {
    deletions: HashSet<usize>,
    insertions: BTreeMap<usize, Vec<char>>,
}

#[derive(Default)]
struct ResourcePlan {
    filtered_deletions: Vec<usize>,
    filtered_insertions: BTreeMap<usize, Vec<char>>,
}

#[derive(Default)]
struct NodePlan {
    deletions: HashSet<usize>,
    insertions: BTreeMap<usize, Vec<char>>,
}

#[derive(Clone, Copy, Debug)]
enum EditTarget {
    TitleBoundary(usize),
    ResourceBoundary {
        resource_index: usize,
        local_index: usize,
    },
}

pub(crate) fn write_epub_preserving_structure(
    source_path: &Path,
    destination_path: &Path,
    original_text: &str,
    edited_text: &str,
    language: Language,
) -> Result<(), String> {
    crate::log_debug(&format!(
        "EPUB save: start source='{}' destination='{}' original_chars={} edited_chars={}",
        source_path.display(),
        destination_path.display(),
        original_text.chars().count(),
        edited_text.chars().count()
    ));
    let model = build_epub_edit_model(source_path, language)?;
    let normalized_original = normalize_newlines(original_text);
    let mapped_original = if normalized_original == model.full_text {
        normalized_original
    } else {
        normalize_epub_editor_text(&normalized_original, &model.title)
    };
    if mapped_original != model.full_text {
        return Err(epub_save_error(language, "epub_editor.source_changed"));
    }

    let normalized_input = normalize_newlines(edited_text);
    let normalized_edited = if normalized_input == model.full_text {
        normalized_input
    } else {
        normalize_epub_editor_text(&normalized_input, &model.title)
    };
    if let Some(invalid) = normalized_edited.chars().find(|ch| !is_valid_xml_char(*ch)) {
        return Err(epub_save_error_f(
            language,
            "epub_editor.invalid_xml_character",
            &[("code", &format!("{:04X}", invalid as u32))],
        ));
    }
    let edited_chars: Vec<char> = normalized_edited.chars().collect();
    let edits = compute_text_edits(&model.full_chars, &edited_chars, MAX_EPUB_EDIT_DISTANCE)
        .map_err(|key| epub_save_error(language, key))?;
    let insertion_count = edits
        .iter()
        .filter(|edit| matches!(**edit, TextEdit::Insert { .. }))
        .count();
    let deletion_count = edits.len().saturating_sub(insertion_count);
    let inserted_newlines = edits
        .iter()
        .filter(|edit| matches!(**edit, TextEdit::Insert { ch: '\n', .. }))
        .count();
    crate::log_debug(&format!(
        "EPUB save: model_chars={} normalized_edited_chars={} edits={} insertions={} deletions={} inserted_newlines={} expected_newlines={}",
        model.full_chars.len(),
        edited_chars.len(),
        edits.len(),
        insertion_count,
        deletion_count,
        inserted_newlines,
        normalized_edited.matches('\n').count()
    ));

    if edits.is_empty() {
        if source_path == destination_path {
            crate::log_debug("EPUB save: no text changes; destination already is the source file.");
            return Ok(());
        }
        let temporary_path = unique_sibling_path(destination_path, "sonarpad-epub.tmp");
        let copy_result = std::fs::copy(source_path, &temporary_path)
            .map(|_| ())
            .map_err(|error| save_error(language, error))
            .and_then(|()| validate_epub(&temporary_path, &normalized_edited, language));
        if let Err(message) = copy_result {
            crate::log_debug(&format!(
                "EPUB save: unchanged-copy validation failed: {message}"
            ));
            let _remove_result = std::fs::remove_file(&temporary_path);
            return Err(message);
        }
        let commit_result = commit_temporary_file(&temporary_path, destination_path, language);
        if commit_result.is_ok() {
            crate::log_debug("EPUB save: unchanged EPUB copied and validated successfully.");
        }
        return commit_result;
    }

    let (title, resource_replacements) = apply_book_edits(&model, &edits, language)?;
    let mut replacements = HashMap::new();
    for (path, bytes) in resource_replacements {
        replacements.insert(normalize_epub_internal_path(&path), bytes);
    }

    if title != model.title {
        let opf_bytes = read_archive_entry(source_path, &model.opf_path, language)?;
        let opf = String::from_utf8(opf_bytes)
            .map_err(|_| epub_save_error(language, "epub_editor.metadata_not_utf8"))?;
        let updated_opf = replace_package_title(&opf, &title)
            .ok_or_else(|| epub_save_error(language, "epub_editor.title_update_failed"))?;
        replacements.insert(
            normalize_epub_internal_path(&model.opf_path),
            updated_opf.into_bytes(),
        );
    }

    let temporary_path = unique_sibling_path(destination_path, "sonarpad-epub.tmp");
    let write_result = repack_epub(source_path, &temporary_path, &replacements, language)
        .and_then(|()| validate_epub(&temporary_path, &normalized_edited, language));
    if let Err(message) = write_result {
        crate::log_debug(&format!(
            "EPUB save: repack or validation failed: {message}"
        ));
        let _remove_result = std::fs::remove_file(&temporary_path);
        return Err(message);
    }

    let commit_result = commit_temporary_file(&temporary_path, destination_path, language);
    if commit_result.is_ok() {
        crate::log_debug(&format!(
            "EPUB save: completed successfully; changed_resources={}",
            replacements.len()
        ));
    }
    commit_result
}

fn build_epub_edit_model(path: &Path, language: Language) -> Result<EpubEditModel, String> {
    let opf_path = read_epub_rootfile_path(path, language)?;
    let mut document = EpubDoc::new(path).map_err(|error| save_error(language, error))?;
    let title_metadata = document.mdata("title");
    let has_title_metadata = title_metadata.is_some();
    let title = title_metadata
        .map(|item| item.value.clone())
        .unwrap_or_default();

    let mut full_text = String::new();
    let mut full_sources = Vec::new();
    if has_title_metadata {
        for (index, ch) in title.chars().enumerate() {
            full_text.push(ch);
            full_sources.push(BookCharSource::Title(index));
        }
        full_text.push_str("\n\n");
        full_sources.push(BookCharSource::Structural);
        full_sources.push(BookCharSource::Structural);
    }

    let mut resources = Vec::new();
    for spine_item in document.spine.clone() {
        let Some(resource_path) = document
            .resources
            .get(&spine_item.idref)
            .map(|resource| resource.path.clone())
        else {
            continue;
        };
        let Some((content, mime)) = document.get_resource(&spine_item.idref) else {
            continue;
        };
        if !(mime.contains("xhtml") || mime.contains("html") || mime.contains("xml")) {
            continue;
        }
        let html = String::from_utf8(content).map_err(|_| {
            epub_save_error_f(
                language,
                "epub_editor.chapter_not_utf8",
                &[("chapter", &resource_path.display().to_string())],
            )
        })?;
        let extraction = extract_html_for_editing(&html);
        let filtered = filter_epub_extraction(&extraction);
        if filtered.chars.iter().all(|ch| ch.is_whitespace()) {
            continue;
        }

        let resource_index = resources.len();
        let archive_path = normalize_epub_internal_path(&percent_decode_epub_component(
            &resource_path.to_string_lossy(),
        ));
        for (local_index, ch) in filtered.chars.iter().copied().enumerate() {
            full_text.push(ch);
            full_sources.push(BookCharSource::Resource {
                resource_index,
                local_index,
            });
        }
        full_text.push('\n');
        full_sources.push(BookCharSource::Structural);
        resources.push(EditableResource {
            archive_path,
            html,
            filtered_sources: filtered.sources,
            nodes: extraction.nodes,
        });
    }

    let reference = read_epub_document(path, language)?;
    if reference.text != full_text {
        return Err(epub_save_error(language, "epub_editor.layout_not_mappable"));
    }

    let full_chars: Vec<char> = full_text.chars().collect();
    if full_chars.len() != full_sources.len() {
        return Err(epub_save_error(
            language,
            "epub_editor.text_map_inconsistent",
        ));
    }

    Ok(EpubEditModel {
        full_text,
        full_chars,
        full_sources,
        title,
        resources,
        opf_path,
    })
}

fn extract_html_for_editing(html: &str) -> HtmlExtraction {
    let mut chars = Vec::new();
    let mut sources = Vec::new();
    let mut nodes = Vec::new();
    let mut current_node_start = None;
    let mut current_node_chars = Vec::new();
    let mut current_node_raw_spans = Vec::new();
    let mut inside_tag = false;
    let mut tag = String::new();
    let mut tag_start = 0usize;
    let mut last_newline = false;
    let mut skip_stack: Vec<String> = Vec::new();
    let mut in_comment = false;
    let mut entity = String::new();
    let mut entity_start = None;
    let mut in_entity = false;

    let append_node_char = |ch: char,
                            raw_start: usize,
                            raw_end: usize,
                            chars: &mut Vec<char>,
                            sources: &mut Vec<HtmlCharSource>,
                            nodes: &[HtmlTextNode],
                            current_node_start: &mut Option<usize>,
                            current_node_chars: &mut Vec<char>,
                            current_node_raw_spans: &mut Vec<(usize, usize)>,
                            last_newline: &mut bool| {
        if current_node_start.is_none() {
            *current_node_start = Some(raw_start);
        }
        let node_index = nodes.len();
        let char_index = current_node_chars.len();
        current_node_chars.push(ch);
        current_node_raw_spans.push((raw_start, raw_end));
        chars.push(ch);
        sources.push(HtmlCharSource::NodeChar {
            node_index,
            char_index,
        });
        *last_newline = ch == '\n';
    };

    let flush_node = |source_end: usize,
                      nodes: &mut Vec<HtmlTextNode>,
                      current_node_start: &mut Option<usize>,
                      current_node_chars: &mut Vec<char>,
                      current_node_raw_spans: &mut Vec<(usize, usize)>| {
        if let Some(source_start) = current_node_start.take() {
            debug_assert_eq!(current_node_chars.len(), current_node_raw_spans.len());
            nodes.push(HtmlTextNode {
                source_start,
                source_end,
                chars: std::mem::take(current_node_chars),
                raw_spans: std::mem::take(current_node_raw_spans),
            });
        }
    };

    for (byte_index, ch) in html.char_indices() {
        if in_comment {
            tag.push(ch);
            if tag.ends_with("-->") {
                in_comment = false;
                tag.clear();
            }
            continue;
        }

        if inside_tag {
            if ch == '>' {
                inside_tag = false;
                let source_end = byte_index.saturating_add(ch.len_utf8());
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
                        if let Some(position) =
                            skip_stack.iter().rposition(|value| value == &tag_name)
                        {
                            skip_stack.truncate(position);
                        }
                    } else {
                        skip_stack.push(tag_name.clone());
                    }
                    tag.clear();
                    continue;
                }

                let preserve_empty = tag_name == "br"
                    && tag_attribute(tag_trimmed, "class").is_some_and(|classes| {
                        classes
                            .split_ascii_whitespace()
                            .any(|class| class == SONARPAD_LINE_BREAK_CLASS)
                    });
                if tag_name == "br"
                    && skip_stack.is_empty()
                    && (preserve_empty || !last_newline)
                    && !chars.is_empty()
                {
                    chars.push('\n');
                    sources.push(HtmlCharSource::BreakTag {
                        source_start: tag_start,
                        source_end,
                        preserve_empty,
                    });
                    last_newline = true;
                } else if matches!(
                    tag_name.as_str(),
                    "p" | "div"
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
                    && !chars.is_empty()
                {
                    chars.push('\n');
                    sources.push(HtmlCharSource::Structural);
                    last_newline = true;
                }
                tag.clear();
            } else {
                tag.push(ch);
            }
            continue;
        }

        if ch == '<' {
            if in_entity {
                let decoded = decode_html_entity(&entity, true);
                for decoded_ch in decoded {
                    append_node_char(
                        decoded_ch,
                        entity_start.unwrap_or(byte_index),
                        byte_index,
                        &mut chars,
                        &mut sources,
                        &nodes,
                        &mut current_node_start,
                        &mut current_node_chars,
                        &mut current_node_raw_spans,
                        &mut last_newline,
                    );
                }
                entity.clear();
                in_entity = false;
            }
            flush_node(
                byte_index,
                &mut nodes,
                &mut current_node_start,
                &mut current_node_chars,
                &mut current_node_raw_spans,
            );
            inside_tag = true;
            tag_start = byte_index;
            continue;
        }
        if !skip_stack.is_empty() {
            continue;
        }
        if in_entity {
            if ch == ';' {
                let raw_end = byte_index.saturating_add(ch.len_utf8());
                let raw_start = entity_start.unwrap_or(byte_index);
                let decoded = decode_html_entity(&entity, true);
                for decoded_ch in decoded {
                    append_node_char(
                        decoded_ch,
                        raw_start,
                        raw_end,
                        &mut chars,
                        &mut sources,
                        &nodes,
                        &mut current_node_start,
                        &mut current_node_chars,
                        &mut current_node_raw_spans,
                        &mut last_newline,
                    );
                }
                entity.clear();
                in_entity = false;
            } else if entity.len() < 16 && !ch.is_whitespace() {
                entity.push(ch);
            } else {
                let raw_end = byte_index.saturating_add(ch.len_utf8());
                let raw_start = entity_start.unwrap_or(byte_index);
                append_node_char(
                    '&',
                    raw_start,
                    raw_end,
                    &mut chars,
                    &mut sources,
                    &nodes,
                    &mut current_node_start,
                    &mut current_node_chars,
                    &mut current_node_raw_spans,
                    &mut last_newline,
                );
                for entity_ch in entity.chars() {
                    append_node_char(
                        entity_ch,
                        raw_start,
                        raw_end,
                        &mut chars,
                        &mut sources,
                        &nodes,
                        &mut current_node_start,
                        &mut current_node_chars,
                        &mut current_node_raw_spans,
                        &mut last_newline,
                    );
                }
                append_node_char(
                    ch,
                    raw_start,
                    raw_end,
                    &mut chars,
                    &mut sources,
                    &nodes,
                    &mut current_node_start,
                    &mut current_node_chars,
                    &mut current_node_raw_spans,
                    &mut last_newline,
                );
                entity.clear();
                in_entity = false;
            }
            continue;
        }
        if ch == '&' {
            if current_node_start.is_none() {
                current_node_start = Some(byte_index);
            }
            entity_start = Some(byte_index);
            in_entity = true;
            entity.clear();
            continue;
        }
        append_node_char(
            ch,
            byte_index,
            byte_index.saturating_add(ch.len_utf8()),
            &mut chars,
            &mut sources,
            &nodes,
            &mut current_node_start,
            &mut current_node_chars,
            &mut current_node_raw_spans,
            &mut last_newline,
        );
    }

    if in_entity {
        let raw_start = entity_start.unwrap_or(html.len());
        append_node_char(
            '&',
            raw_start,
            html.len(),
            &mut chars,
            &mut sources,
            &nodes,
            &mut current_node_start,
            &mut current_node_chars,
            &mut current_node_raw_spans,
            &mut last_newline,
        );
        for entity_ch in entity.chars() {
            append_node_char(
                entity_ch,
                raw_start,
                html.len(),
                &mut chars,
                &mut sources,
                &nodes,
                &mut current_node_start,
                &mut current_node_chars,
                &mut current_node_raw_spans,
                &mut last_newline,
            );
        }
    }
    flush_node(
        html.len(),
        &mut nodes,
        &mut current_node_start,
        &mut current_node_chars,
        &mut current_node_raw_spans,
    );

    HtmlExtraction {
        chars,
        sources,
        nodes,
    }
}

fn filter_epub_extraction(extraction: &HtmlExtraction) -> FilteredHtmlExtraction {
    let mut chars = Vec::new();
    let mut sources = Vec::new();
    let mut line_start = 0usize;

    while line_start < extraction.chars.len() {
        let newline_position = extraction.chars[line_start..]
            .iter()
            .position(|ch| *ch == '\n')
            .map(|offset| line_start + offset);
        let line_end = newline_position.unwrap_or(extraction.chars.len());
        let mut trimmed_start = line_start;
        while trimmed_start < line_end && extraction.chars[trimmed_start].is_whitespace() {
            trimmed_start += 1;
        }
        let mut trimmed_end = line_end;
        while trimmed_end > trimmed_start && extraction.chars[trimmed_end - 1].is_whitespace() {
            trimmed_end -= 1;
        }

        let newline_source = newline_position
            .map(|position| extraction.sources[position])
            .unwrap_or(HtmlCharSource::Structural);
        let preserve_empty_line = matches!(
            newline_source,
            HtmlCharSource::BreakTag {
                preserve_empty: true,
                ..
            }
        );

        if trimmed_start < trimmed_end {
            let line: String = extraction.chars[trimmed_start..trimmed_end]
                .iter()
                .collect();
            if !is_epub_metadata_noise_line(&line)
                && !(line.starts_with("part") && line.len() <= 12)
            {
                chars.extend_from_slice(&extraction.chars[trimmed_start..trimmed_end]);
                sources.extend_from_slice(&extraction.sources[trimmed_start..trimmed_end]);
                chars.push('\n');
                sources.push(newline_source);
            }
        } else if preserve_empty_line {
            chars.push('\n');
            sources.push(newline_source);
        }

        let Some(position) = newline_position else {
            break;
        };
        line_start = position.saturating_add(1);
    }

    FilteredHtmlExtraction { chars, sources }
}

fn compute_text_edits(
    old: &[char],
    new: &[char],
    maximum_distance: usize,
) -> Result<Vec<TextEdit>, &'static str> {
    let prefix = old
        .iter()
        .zip(new.iter())
        .take_while(|(left, right)| left == right)
        .count();
    let maximum_suffix = old
        .len()
        .saturating_sub(prefix)
        .min(new.len().saturating_sub(prefix));
    let suffix = old[old.len().saturating_sub(maximum_suffix)..]
        .iter()
        .rev()
        .zip(new[new.len().saturating_sub(maximum_suffix)..].iter().rev())
        .take_while(|(left, right)| left == right)
        .count();

    let old_middle = &old[prefix..old.len().saturating_sub(suffix)];
    let new_middle = &new[prefix..new.len().saturating_sub(suffix)];
    if old_middle.is_empty() {
        if new_middle.len() > maximum_distance {
            return Err("epub_editor.too_many_changes");
        }
        return Ok(new_middle
            .iter()
            .copied()
            .map(|ch| TextEdit::Insert {
                old_index: prefix,
                ch,
            })
            .collect());
    }
    if new_middle.is_empty() {
        if old_middle.len() > maximum_distance {
            return Err("epub_editor.too_many_changes");
        }
        return Ok((0..old_middle.len())
            .map(|offset| TextEdit::Delete {
                old_index: prefix + offset,
            })
            .collect());
    }

    let tags = myers_diff_tags(old_middle, new_middle, maximum_distance)?;
    let mut edits = Vec::new();
    let mut old_position = prefix;
    let mut new_position = prefix;
    for tag in tags {
        match tag {
            DiffTag::Equal => {
                old_position += 1;
                new_position += 1;
            }
            DiffTag::Delete => {
                edits.push(TextEdit::Delete {
                    old_index: old_position,
                });
                old_position += 1;
            }
            DiffTag::Insert => {
                let Some(ch) = new.get(new_position).copied() else {
                    return Err("epub_editor.diff_failed");
                };
                edits.push(TextEdit::Insert {
                    old_index: old_position,
                    ch,
                });
                new_position += 1;
            }
        }
    }
    Ok(edits)
}

fn myers_diff_tags(
    old: &[char],
    new: &[char],
    maximum_distance: usize,
) -> Result<Vec<DiffTag>, &'static str> {
    let old_len = old.len() as isize;
    let new_len = new.len() as isize;
    let maximum = old.len().saturating_add(new.len()).min(maximum_distance);
    let mut trace: Vec<Vec<isize>> = Vec::new();

    let mut first_x = 0isize;
    let mut first_y = 0isize;
    while first_x < old_len && first_y < new_len && old[first_x as usize] == new[first_y as usize] {
        first_x += 1;
        first_y += 1;
    }
    trace.push(vec![first_x]);
    if first_x == old_len && first_y == new_len {
        return Ok(vec![DiffTag::Equal; old.len()]);
    }

    let mut found_distance = None;
    for distance in 1..=maximum {
        let distance_isize = distance as isize;
        let previous = &trace[distance - 1];
        let mut current = vec![-1isize; distance.saturating_mul(2).saturating_add(1)];
        let mut diagonal = -distance_isize;
        while diagonal <= distance_isize {
            let move_down = diagonal == -distance_isize
                || (diagonal != distance_isize
                    && get_diagonal(previous, distance - 1, diagonal - 1)
                        < get_diagonal(previous, distance - 1, diagonal + 1));
            let mut x = if move_down {
                get_diagonal(previous, distance - 1, diagonal + 1)
            } else {
                get_diagonal(previous, distance - 1, diagonal - 1).saturating_add(1)
            };
            let mut y = x - diagonal;
            while x >= 0
                && y >= 0
                && x < old_len
                && y < new_len
                && old[x as usize] == new[y as usize]
            {
                x += 1;
                y += 1;
            }
            let index = (diagonal + distance_isize) as usize;
            current[index] = x;
            if x == old_len && y == new_len {
                found_distance = Some(distance);
                break;
            }
            diagonal += 2;
        }
        trace.push(current);
        if found_distance.is_some() {
            break;
        }
    }

    let Some(distance) = found_distance else {
        return Err("epub_editor.too_many_changes");
    };

    let mut tags = Vec::with_capacity(old.len().saturating_add(new.len()));
    let mut x = old_len;
    let mut y = new_len;
    for current_distance in (1..=distance).rev() {
        let diagonal = x - y;
        let distance_isize = current_distance as isize;
        let previous = &trace[current_distance - 1];
        let move_down = diagonal == -distance_isize
            || (diagonal != distance_isize
                && get_diagonal(previous, current_distance - 1, diagonal - 1)
                    < get_diagonal(previous, current_distance - 1, diagonal + 1));
        let previous_diagonal = if move_down {
            diagonal + 1
        } else {
            diagonal - 1
        };
        let previous_x = get_diagonal(previous, current_distance - 1, previous_diagonal);
        let previous_y = previous_x - previous_diagonal;

        while x > previous_x && y > previous_y {
            tags.push(DiffTag::Equal);
            x -= 1;
            y -= 1;
        }
        if move_down {
            tags.push(DiffTag::Insert);
            y -= 1;
        } else {
            tags.push(DiffTag::Delete);
            x -= 1;
        }
    }
    while x > 0 && y > 0 {
        tags.push(DiffTag::Equal);
        x -= 1;
        y -= 1;
    }
    while x > 0 {
        tags.push(DiffTag::Delete);
        x -= 1;
    }
    while y > 0 {
        tags.push(DiffTag::Insert);
        y -= 1;
    }
    tags.reverse();
    Ok(tags)
}

fn get_diagonal(values: &[isize], distance: usize, diagonal: isize) -> isize {
    let distance_isize = distance as isize;
    if diagonal < -distance_isize
        || diagonal > distance_isize
        || (diagonal + distance_isize) % 2 != 0
    {
        return -1;
    }
    values
        .get((diagonal + distance_isize) as usize)
        .copied()
        .unwrap_or(-1)
}

fn apply_book_edits(
    model: &EpubEditModel,
    edits: &[TextEdit],
    language: Language,
) -> Result<AppliedBookEdits, String> {
    let mut title_plan = TextPlan::default();
    let mut resource_plans: Vec<ResourcePlan> = (0..model.resources.len())
        .map(|_| ResourcePlan::default())
        .collect();

    for edit in edits {
        match *edit {
            TextEdit::Delete { old_index } => match model.full_sources.get(old_index).copied() {
                Some(BookCharSource::Title(char_index)) => {
                    title_plan.deletions.insert(char_index);
                }
                Some(BookCharSource::Resource {
                    resource_index,
                    local_index,
                }) => {
                    if let Some(plan) = resource_plans.get_mut(resource_index) {
                        plan.filtered_deletions.push(local_index);
                    }
                }
                Some(BookCharSource::Structural) | None => {
                    return Err(epub_save_error(
                        language,
                        "epub_editor.structural_boundary_removed",
                    ));
                }
            },
            TextEdit::Insert { old_index, ch } => {
                let Some(target) = find_edit_target(&model.full_sources, old_index) else {
                    return Err(epub_save_error(
                        language,
                        "epub_editor.insert_target_failed",
                    ));
                };
                match target {
                    EditTarget::TitleBoundary(boundary) => {
                        title_plan.insertions.entry(boundary).or_default().push(ch);
                    }
                    EditTarget::ResourceBoundary {
                        resource_index,
                        local_index,
                    } => {
                        let Some(plan) = resource_plans.get_mut(resource_index) else {
                            return Err(epub_save_error(
                                language,
                                "epub_editor.insert_invalid_chapter",
                            ));
                        };
                        plan.filtered_insertions
                            .entry(local_index)
                            .or_default()
                            .push(ch);
                    }
                }
            }
        }
    }

    let title = apply_text_plan(&model.title, &title_plan);
    let mut replacements = Vec::new();
    for (resource, plan) in model.resources.iter().zip(resource_plans.iter()) {
        if plan.filtered_deletions.is_empty() && plan.filtered_insertions.is_empty() {
            continue;
        }
        let updated = apply_resource_plan(resource, plan, language)?;
        replacements.push((resource.archive_path.clone(), updated.into_bytes()));
    }
    Ok((title, replacements))
}

fn find_edit_target(sources: &[BookCharSource], old_index: usize) -> Option<EditTarget> {
    if let Some(source) = sources.get(old_index).copied()
        && let Some(target) = target_before_source(source)
    {
        return Some(target);
    }

    let mut previous = None;
    let mut cursor = old_index.min(sources.len());
    while cursor > 0 {
        cursor -= 1;
        if let Some(target) = target_after_source(sources[cursor]) {
            previous = Some((old_index.saturating_sub(cursor), target));
            break;
        }
    }

    let mut next = None;
    let mut cursor = old_index;
    while cursor < sources.len() {
        if let Some(target) = target_before_source(sources[cursor]) {
            next = Some((cursor.saturating_sub(old_index), target));
            break;
        }
        cursor += 1;
    }

    match (previous, next) {
        (Some((previous_distance, previous_target)), Some((next_distance, next_target))) => {
            if previous_distance <= next_distance {
                Some(previous_target)
            } else {
                Some(next_target)
            }
        }
        (Some((_, target)), None) | (None, Some((_, target))) => Some(target),
        (None, None) => None,
    }
}

fn target_before_source(source: BookCharSource) -> Option<EditTarget> {
    match source {
        BookCharSource::Title(index) => Some(EditTarget::TitleBoundary(index)),
        BookCharSource::Resource {
            resource_index,
            local_index,
        } => Some(EditTarget::ResourceBoundary {
            resource_index,
            local_index,
        }),
        BookCharSource::Structural => None,
    }
}

fn target_after_source(source: BookCharSource) -> Option<EditTarget> {
    match source {
        BookCharSource::Title(index) => Some(EditTarget::TitleBoundary(index.saturating_add(1))),
        BookCharSource::Resource {
            resource_index,
            local_index,
        } => Some(EditTarget::ResourceBoundary {
            resource_index,
            local_index: local_index.saturating_add(1),
        }),
        BookCharSource::Structural => None,
    }
}

fn apply_text_plan(original: &str, plan: &TextPlan) -> String {
    let chars: Vec<char> = original.chars().collect();
    let mut output = String::new();
    for boundary in 0..=chars.len() {
        if let Some(inserted) = plan.insertions.get(&boundary) {
            output.extend(inserted.iter().copied());
        }
        if boundary < chars.len() && !plan.deletions.contains(&boundary) {
            output.push(chars[boundary]);
        }
    }
    output
}

fn apply_resource_plan(
    resource: &EditableResource,
    plan: &ResourcePlan,
    language: Language,
) -> Result<String, String> {
    let mut node_plans: Vec<NodePlan> = (0..resource.nodes.len())
        .map(|_| NodePlan::default())
        .collect();
    let mut removed_break_tags = HashSet::new();

    for local_index in &plan.filtered_deletions {
        let Some(source) = resource.filtered_sources.get(*local_index).copied() else {
            return Err(epub_save_error(
                language,
                "epub_editor.deletion_invalid_position",
            ));
        };
        match source {
            HtmlCharSource::NodeChar {
                node_index,
                char_index,
            } => {
                let Some(node_plan) = node_plans.get_mut(node_index) else {
                    return Err(epub_save_error(
                        language,
                        "epub_editor.text_node_edit_failed",
                    ));
                };
                node_plan.deletions.insert(char_index);
            }
            HtmlCharSource::BreakTag {
                source_start,
                source_end,
                ..
            } => {
                removed_break_tags.insert((source_start, source_end));
            }
            HtmlCharSource::Structural => {
                return Err(epub_save_error(
                    language,
                    "epub_editor.paragraph_boundary_removed",
                ));
            }
        }
    }

    for (boundary, inserted) in &plan.filtered_insertions {
        let Some((node_index, char_boundary)) =
            find_node_insertion_target(&resource.filtered_sources, *boundary)
        else {
            return Err(epub_save_error(
                language,
                "epub_editor.insert_node_target_failed",
            ));
        };
        let Some(node_plan) = node_plans.get_mut(node_index) else {
            return Err(epub_save_error(language, "epub_editor.insert_invalid_node"));
        };
        node_plan
            .insertions
            .entry(char_boundary)
            .or_default()
            .extend(inserted.iter().copied());
    }

    let mut replacements: Vec<(usize, usize, String)> = Vec::new();
    for (node, node_plan) in resource.nodes.iter().zip(node_plans.iter()) {
        if node_plan.deletions.is_empty() && node_plan.insertions.is_empty() {
            continue;
        }
        let rendered = render_edited_node(node, node_plan, &resource.html);
        replacements.push((node.source_start, node.source_end, rendered));
    }
    for (source_start, source_end) in removed_break_tags {
        replacements.push((source_start, source_end, String::new()));
    }
    replacements.sort_by_key(|replacement| std::cmp::Reverse(replacement.0));

    let mut html = resource.html.clone();
    let mut previous_start = html.len();
    for (source_start, source_end, replacement) in replacements {
        if source_start > source_end || source_end > html.len() || source_end > previous_start {
            return Err(epub_save_error(language, "epub_editor.overlapping_edits"));
        }
        html.replace_range(source_start..source_end, &replacement);
        previous_start = source_start;
    }
    Ok(html)
}

fn find_node_insertion_target(
    sources: &[HtmlCharSource],
    boundary: usize,
) -> Option<(usize, usize)> {
    if let Some(HtmlCharSource::NodeChar {
        node_index,
        char_index,
    }) = sources.get(boundary).copied()
    {
        return Some((node_index, char_index));
    }

    let mut previous = None;
    let mut cursor = boundary.min(sources.len());
    while cursor > 0 {
        cursor -= 1;
        if let HtmlCharSource::NodeChar {
            node_index,
            char_index,
        } = sources[cursor]
        {
            previous = Some((
                boundary.saturating_sub(cursor),
                (node_index, char_index.saturating_add(1)),
            ));
            break;
        }
    }

    let mut next = None;
    let mut cursor = boundary;
    while cursor < sources.len() {
        if let HtmlCharSource::NodeChar {
            node_index,
            char_index,
        } = sources[cursor]
        {
            next = Some((cursor.saturating_sub(boundary), (node_index, char_index)));
            break;
        }
        cursor += 1;
    }

    match (previous, next) {
        (Some((previous_distance, previous_target)), Some((next_distance, next_target))) => {
            if previous_distance <= next_distance {
                Some(previous_target)
            } else {
                Some(next_target)
            }
        }
        (Some((_, target)), None) | (None, Some((_, target))) => Some(target),
        (None, None) => None,
    }
}

fn render_edited_node(node: &HtmlTextNode, plan: &NodePlan, html: &str) -> String {
    let mut output = String::new();
    let mut raw_cursor = node.source_start;
    let mut char_index = 0usize;

    while char_index < node.chars.len() {
        if let Some(inserted) = plan.insertions.get(&char_index) {
            for ch in inserted {
                append_inserted_xhtml_char(&mut output, *ch);
            }
        }

        let Some(&(raw_start, raw_end)) = node.raw_spans.get(char_index) else {
            break;
        };
        if raw_start > raw_cursor && raw_start <= html.len() {
            output.push_str(&html[raw_cursor..raw_start]);
        }

        let mut group_end = char_index.saturating_add(1);
        while group_end < node.raw_spans.len() && node.raw_spans[group_end] == (raw_start, raw_end)
        {
            group_end += 1;
        }
        let group_has_deletion =
            (char_index..group_end).any(|index| plan.deletions.contains(&index));
        let group_has_internal_insertion = (char_index.saturating_add(1)..group_end)
            .any(|boundary| plan.insertions.contains_key(&boundary));

        if !group_has_deletion
            && !group_has_internal_insertion
            && raw_start <= raw_end
            && raw_end <= html.len()
        {
            output.push_str(&html[raw_start..raw_end]);
        } else {
            for (index, &ch) in node
                .chars
                .iter()
                .enumerate()
                .take(group_end)
                .skip(char_index)
            {
                if index > char_index
                    && let Some(inserted) = plan.insertions.get(&index)
                {
                    for ch in inserted {
                        append_inserted_xhtml_char(&mut output, *ch);
                    }
                }
                if !plan.deletions.contains(&index) {
                    append_escaped_xml_char(&mut output, ch);
                }
            }
        }

        raw_cursor = raw_end;
        char_index = group_end;
    }

    if let Some(inserted) = plan.insertions.get(&node.chars.len()) {
        for ch in inserted {
            append_inserted_xhtml_char(&mut output, *ch);
        }
    }
    if raw_cursor < node.source_end && node.source_end <= html.len() {
        output.push_str(&html[raw_cursor..node.source_end]);
    }
    output
}

fn append_inserted_xhtml_char(output: &mut String, ch: char) {
    match ch {
        '\n' => output.push_str("<br class=\"sonarpad-preserve-line-break\"/>"),
        '\r' => {}
        _ => append_escaped_xml_char(output, ch),
    }
}

fn append_escaped_xml_char(output: &mut String, ch: char) {
    match ch {
        '&' => output.push_str("&amp;"),
        '<' => output.push_str("&lt;"),
        '>' => output.push_str("&gt;"),
        _ => output.push(ch),
    }
}

fn is_valid_xml_char(ch: char) -> bool {
    matches!(
        ch,
        '\u{9}'
            | '\u{A}'
            | '\u{D}'
            | '\u{20}'..='\u{D7FF}'
            | '\u{E000}'..='\u{FFFD}'
            | '\u{10000}'..='\u{10FFFF}'
    )
}

fn decode_html_entity(entity: &str, include_semicolon_for_unknown: bool) -> Vec<char> {
    match entity {
        "nbsp" => vec![' '],
        "lt" => vec!['<'],
        "gt" => vec!['>'],
        "amp" => vec!['&'],
        "quot" => vec!['"'],
        "apos" => vec!['\''],
        value if value.starts_with("#x") || value.starts_with("#X") => {
            if let Ok(number) = u32::from_str_radix(&value[2..], 16)
                && let Some(ch) = char::from_u32(number)
            {
                return vec![ch];
            }
            unknown_entity_chars(entity, include_semicolon_for_unknown)
        }
        value if value.starts_with('#') => {
            if let Ok(number) = value[1..].parse::<u32>()
                && let Some(ch) = char::from_u32(number)
            {
                return vec![ch];
            }
            unknown_entity_chars(entity, include_semicolon_for_unknown)
        }
        _ => unknown_entity_chars(entity, include_semicolon_for_unknown),
    }
}

fn unknown_entity_chars(entity: &str, include_semicolon: bool) -> Vec<char> {
    let mut output = String::with_capacity(entity.len().saturating_add(2));
    output.push('&');
    output.push_str(entity);
    if include_semicolon {
        output.push(';');
    }
    output.chars().collect()
}

fn is_epub_metadata_noise_line(line: &str) -> bool {
    let normalized = line.split_whitespace().collect::<Vec<_>>().join(" ");
    normalized.eq_ignore_ascii_case("epub r1.0")
        || normalized.eq_ignore_ascii_case("epub base r2.1")
}

fn read_epub_rootfile_path(path: &Path, language: Language) -> Result<String, String> {
    let file = File::open(path).map_err(|error| save_error(language, error))?;
    let mut archive = ZipArchive::new(file).map_err(|error| save_error(language, error))?;
    let mut container = archive
        .by_name("META-INF/container.xml")
        .map_err(|error| save_error(language, error))?;
    let mut xml = String::new();
    container
        .read_to_string(&mut xml)
        .map_err(|error| save_error(language, error))?;
    find_start_tag_attribute(&xml, "rootfile", "full-path")
        .ok_or_else(|| epub_save_error(language, "epub_editor.container_missing_package"))
}

fn find_start_tag_attribute(
    xml: &str,
    element_local_name: &str,
    attribute: &str,
) -> Option<String> {
    let mut cursor = 0usize;
    while let Some(relative_start) = xml[cursor..].find('<') {
        let start = cursor + relative_start;
        let relative_end = xml[start..].find('>')?;
        let end = start + relative_end;
        let tag = xml[start + 1..end].trim();
        cursor = end.saturating_add(1);
        if tag.starts_with('/') || tag.starts_with('!') || tag.starts_with('?') {
            continue;
        }
        let name = tag.split_whitespace().next().unwrap_or("");
        let local_name = name
            .rsplit(':')
            .next()
            .unwrap_or(name)
            .trim_end_matches('/');
        if local_name.eq_ignore_ascii_case(element_local_name) {
            return tag_attribute(tag, attribute);
        }
    }
    None
}

fn tag_attribute(tag: &str, requested_name: &str) -> Option<String> {
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
            return Some(decode_basic_xml_entities(&tag[value_start..value_end]));
        }
    }
    None
}

fn replace_package_title(opf: &str, title: &str) -> Option<String> {
    let lowercase = opf.to_ascii_lowercase();
    let mut cursor = 0usize;
    while let Some(relative_start) = lowercase[cursor..].find('<') {
        let start = cursor + relative_start;
        let relative_end = lowercase[start..].find('>')?;
        let start_tag_end = start + relative_end;
        let tag = opf[start + 1..start_tag_end].trim();
        cursor = start_tag_end.saturating_add(1);
        if tag.starts_with('/') || tag.starts_with('!') || tag.starts_with('?') {
            continue;
        }
        let name = tag
            .split_whitespace()
            .next()
            .unwrap_or("")
            .trim_end_matches('/');
        let local_name = name.rsplit(':').next().unwrap_or(name);
        if !local_name.eq_ignore_ascii_case("title") {
            continue;
        }
        let closing = format!("</{}>", name.to_ascii_lowercase());
        let content_start = start_tag_end.saturating_add(1);
        let relative_closing = lowercase[content_start..].find(&closing)?;
        let content_end = content_start + relative_closing;
        if opf[content_start..content_end].contains('<') {
            return None;
        }
        let mut output = opf.to_string();
        output.replace_range(content_start..content_end, &escape_xml_text(title));
        return Some(output);
    }
    None
}

fn escape_xml_text(value: &str) -> String {
    let mut output = String::new();
    for ch in value.chars() {
        append_escaped_xml_char(&mut output, ch);
    }
    output
}

fn decode_basic_xml_entities(value: &str) -> String {
    let mut output = String::new();
    let mut entity = String::new();
    let mut in_entity = false;
    for ch in value.chars() {
        if in_entity {
            if ch == ';' {
                output.extend(decode_html_entity(&entity, true));
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

fn read_archive_entry(
    path: &Path,
    entry_path: &str,
    language: Language,
) -> Result<Vec<u8>, String> {
    let file = File::open(path).map_err(|error| save_error(language, error))?;
    let mut archive = ZipArchive::new(file).map_err(|error| save_error(language, error))?;
    let target = normalize_epub_internal_path(entry_path);
    for index in 0..archive.len() {
        let mut entry = archive
            .by_index(index)
            .map_err(|error| save_error(language, error))?;
        if normalize_epub_internal_path(&percent_decode_epub_component(entry.name())) == target {
            let mut bytes = Vec::new();
            entry
                .read_to_end(&mut bytes)
                .map_err(|error| save_error(language, error))?;
            return Ok(bytes);
        }
    }
    Err(epub_save_error_f(
        language,
        "epub_editor.entry_not_found",
        &[("entry", entry_path)],
    ))
}

fn repack_epub(
    source_path: &Path,
    output_path: &Path,
    replacements: &HashMap<String, Vec<u8>>,
    language: Language,
) -> Result<(), String> {
    let source_file = File::open(source_path).map_err(|error| save_error(language, error))?;
    let mut source = ZipArchive::new(source_file).map_err(|error| save_error(language, error))?;
    let archive_comment = source.comment().to_vec();
    let output_file = File::create(output_path).map_err(|error| save_error(language, error))?;
    let mut writer = ZipWriter::new(output_file);
    writer.set_raw_comment(archive_comment);
    let mut matched_replacements = HashSet::new();

    for index in 0..source.len() {
        let entry = source
            .by_index(index)
            .map_err(|error| save_error(language, error))?;
        let name = entry.name().to_string();
        let normalized_name =
            normalize_epub_internal_path(&percent_decode_epub_component(entry.name()));

        if let Some(replacement) = replacements.get(&normalized_name) {
            if !matched_replacements.insert(normalized_name.clone()) {
                return Err(epub_save_error_f(
                    language,
                    "epub_editor.duplicate_entry",
                    &[("entry", &normalized_name)],
                ));
            }
            let compression = if name == "mimetype" {
                CompressionMethod::Stored
            } else {
                entry.compression()
            };
            let mut options = FileOptions::default()
                .compression_method(compression)
                .last_modified_time(entry.last_modified())
                .large_file(entry.size() > u64::from(u32::MAX));
            if let Some(mode) = entry.unix_mode() {
                options = options.unix_permissions(mode);
            }
            writer
                .start_file(name, options)
                .map_err(|error| save_error(language, error))?;
            writer
                .write_all(replacement)
                .map_err(|error| save_error(language, error))?;
        } else if entry.is_dir() {
            let mut options = FileOptions::default()
                .compression_method(entry.compression())
                .last_modified_time(entry.last_modified());
            if let Some(mode) = entry.unix_mode() {
                options = options.unix_permissions(mode);
            }
            writer
                .add_directory(name, options)
                .map_err(|error| save_error(language, error))?;
        } else {
            writer
                .raw_copy_file(entry)
                .map_err(|error| save_error(language, error))?;
        }
    }

    if matched_replacements.len() != replacements.len() {
        return Err(epub_save_error(language, "epub_editor.chapters_missing"));
    }
    let output_file = writer
        .finish()
        .map_err(|error| save_error(language, error))?;
    output_file
        .sync_all()
        .map_err(|error| save_error(language, error))?;
    Ok(())
}

fn validate_epub(path: &Path, expected_text: &str, language: Language) -> Result<(), String> {
    {
        let file = File::open(path).map_err(|error| save_error(language, error))?;
        let mut archive = ZipArchive::new(file).map_err(|error| save_error(language, error))?;
        if archive.is_empty() {
            return Err(epub_save_error(language, "epub_editor.archive_empty"));
        }
        {
            let mut mimetype = archive
                .by_index(0)
                .map_err(|error| save_error(language, error))?;
            if mimetype.name() != "mimetype" || mimetype.compression() != CompressionMethod::Stored
            {
                return Err(epub_save_error(language, "epub_editor.mimetype_not_first"));
            }
            let mut mimetype_value = String::new();
            mimetype
                .read_to_string(&mut mimetype_value)
                .map_err(|error| save_error(language, error))?;
            if mimetype_value != "application/epub+zip" {
                return Err(epub_save_error(language, "epub_editor.mimetype_invalid"));
            }
        }
        for index in 0..archive.len() {
            let mut entry = archive
                .by_index(index)
                .map_err(|error| save_error(language, error))?;
            if !entry.is_dir() {
                std::io::copy(&mut entry, &mut std::io::sink())
                    .map_err(|error| save_error(language, error))?;
            }
        }
    }

    let reopened = read_epub_document(path, language)?;
    let actual = normalize_newlines(&reopened.text);
    let expected = normalize_newlines(expected_text);
    if actual != expected {
        log_epub_text_mismatch(&expected, &actual);
        return Err(epub_save_error(
            language,
            "epub_editor.validation_text_mismatch",
        ));
    }
    crate::log_debug(&format!(
        "EPUB validation: ok path='{}' chars={} newlines={}",
        path.display(),
        actual.chars().count(),
        actual.matches('\n').count()
    ));
    Ok(())
}

fn log_epub_text_mismatch(expected: &str, actual: &str) {
    let mismatch_index = expected
        .chars()
        .zip(actual.chars())
        .position(|(left, right)| left != right)
        .unwrap_or_else(|| expected.chars().count().min(actual.chars().count()));
    let expected_context = epub_text_context(expected, mismatch_index, 48);
    let actual_context = epub_text_context(actual, mismatch_index, 48);
    crate::log_debug(&format!(
        "EPUB validation mismatch: char_index={} expected_chars={} actual_chars={} expected_newlines={} actual_newlines={} expected_context=\"{}\" actual_context=\"{}\"",
        mismatch_index,
        expected.chars().count(),
        actual.chars().count(),
        expected.matches('\n').count(),
        actual.matches('\n').count(),
        expected_context.escape_debug(),
        actual_context.escape_debug()
    ));
}

fn epub_text_context(value: &str, center: usize, radius: usize) -> String {
    value
        .chars()
        .skip(center.saturating_sub(radius))
        .take(radius.saturating_mul(2))
        .collect()
}

fn commit_temporary_file(
    temporary_path: &Path,
    destination_path: &Path,
    language: Language,
) -> Result<(), String> {
    if !destination_path.exists() {
        return std::fs::rename(temporary_path, destination_path)
            .map_err(|error| save_error(language, error));
    }

    let backup_path = unique_sibling_path(destination_path, "sonarpad-epub.backup");
    std::fs::rename(destination_path, &backup_path).map_err(|error| save_error(language, error))?;
    if let Err(error) = std::fs::rename(temporary_path, destination_path) {
        if let Err(restore_error) = std::fs::rename(&backup_path, destination_path) {
            return Err(epub_save_error_f(
                language,
                "epub_editor.replace_restore_failed",
                &[
                    ("error", &error.to_string()),
                    ("backup", &backup_path.display().to_string()),
                    ("restore_error", &restore_error.to_string()),
                ],
            ));
        }
        return Err(save_error(language, error));
    }
    let _remove_result = std::fs::remove_file(backup_path);
    Ok(())
}

fn unique_sibling_path(path: &Path, suffix: &str) -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .unwrap_or("document.epub");
    path.with_file_name(format!(".{file_name}.{suffix}.{stamp}"))
}

fn normalize_newlines(value: &str) -> String {
    value.replace("\r\n", "\n").replace('\r', "\n")
}

fn normalize_epub_editor_text(value: &str, original_title: &str) -> String {
    if !original_title.is_empty() {
        let title_prefix = format!("{original_title}\n\n");
        if let Some(remainder) = value.strip_prefix(&title_prefix) {
            let mut output = title_prefix;
            output.push_str(&trim_epub_line_edges(remainder));
            return output;
        }
    }
    trim_epub_line_edges(value)
}

fn trim_epub_line_edges(value: &str) -> String {
    let mut output = String::with_capacity(value.len());
    for segment in value.split_inclusive('\n') {
        let (line, has_newline) = segment
            .strip_suffix('\n')
            .map_or((segment, false), |line| (line, true));
        output.push_str(line.trim());
        if has_newline {
            output.push('\n');
        }
    }
    output
}

fn normalize_epub_internal_path(path: &str) -> String {
    let mut parts = Vec::new();
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

fn percent_decode_epub_component(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0usize;
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

fn epub_save_error(language: Language, key: &str) -> String {
    save_error(language, i18n::tr(language, key))
}

fn epub_save_error_f(language: Language, key: &str, args: &[(&str, &str)]) -> String {
    save_error(language, i18n::tr_f(language, key, args))
}

fn save_error(language: Language, detail: impl std::fmt::Display) -> String {
    error_save_file_message(language, detail)
}

#[cfg(test)]
mod tests {
    use super::{
        NodePlan, TextEdit, compute_text_edits, extract_html_for_editing, filter_epub_extraction,
        normalize_epub_editor_text, render_edited_node, replace_package_title,
    };
    use std::collections::{BTreeMap, HashSet};

    fn apply_edits(original: &str, edits: &[TextEdit]) -> String {
        let chars: Vec<char> = original.chars().collect();
        let mut deletions = HashSet::new();
        let mut insertions: BTreeMap<usize, Vec<char>> = BTreeMap::new();
        for edit in edits {
            match *edit {
                TextEdit::Delete { old_index } => {
                    deletions.insert(old_index);
                }
                TextEdit::Insert { old_index, ch } => {
                    insertions.entry(old_index).or_default().push(ch);
                }
            }
        }

        let mut output = String::new();
        for boundary in 0..=chars.len() {
            if let Some(inserted) = insertions.get(&boundary) {
                output.extend(inserted.iter().copied());
            }
            if boundary < chars.len() && !deletions.contains(&boundary) {
                output.push(chars[boundary]);
            }
        }
        output
    }

    #[test]
    fn diff_reconstructs_separate_insertions_and_deletions() {
        for (old, new) in [
            ("alpha beta gamma", "alpha new gamma!"),
            ("uno due tre", "uno\ndue quattro"),
            ("caffè già", "caffè molto già"),
            ("abcdef", "abef"),
            ("", "testo"),
            ("testo", ""),
        ] {
            let old_chars: Vec<char> = old.chars().collect();
            let new_chars: Vec<char> = new.chars().collect();
            let edits = compute_text_edits(&old_chars, &new_chars, 128);
            assert!(edits.is_ok());
            if let Ok(edits) = edits {
                assert_eq!(apply_edits(old, &edits), new);
            }
        }
    }

    #[test]
    fn html_mapping_preserves_inline_tags_and_breaks() {
        let html = "<p>Prima <em>riga</em><br/>Seconda</p>";
        let extraction = extract_html_for_editing(html);
        let filtered = filter_epub_extraction(&extraction);
        let text: String = filtered.chars.iter().collect();
        assert_eq!(text, "Prima riga\nSeconda\n");
        assert!(!extraction.nodes.is_empty());
    }

    #[test]
    fn marked_breaks_preserve_consecutive_blank_lines() {
        let html = "<p>Prima<br class=\"sonarpad-preserve-line-break\"/><br class=\"sonarpad-preserve-line-break\"/><br class=\"sonarpad-preserve-line-break\"/>Seconda</p>";
        let extraction = extract_html_for_editing(html);
        let filtered = filter_epub_extraction(&extraction);
        let text: String = filtered.chars.iter().collect();
        assert_eq!(text, "Prima\n\n\nSeconda\n");
    }

    #[test]
    fn edited_node_preserves_untouched_entities_byte_for_byte() {
        let html = "<p>A&nbsp;B &amp; C</p>";
        let extraction = extract_html_for_editing(html);
        let node = &extraction.nodes[0];
        let mut plan = NodePlan::default();
        plan.insertions
            .entry(node.chars.len())
            .or_default()
            .push('!');
        let rendered = render_edited_node(node, &plan, html);
        assert_eq!(rendered, "A&nbsp;B &amp; C!");
    }

    #[test]
    fn epub_line_normalization_removes_only_line_edge_spaces() {
        let text = "Titolo\n\nPrima riga \n seconda riga\n\n";
        assert_eq!(
            normalize_epub_editor_text(text, "Titolo"),
            "Titolo\n\nPrima riga\nseconda riga\n\n"
        );
    }

    #[test]
    fn package_title_update_leaves_markup_unchanged() {
        let opf = r#"<package><metadata><dc:title id="title">Vecchio &amp; titolo</dc:title></metadata></package>"#;
        let updated = replace_package_title(opf, "Nuovo & titolo");
        assert_eq!(
            updated.as_deref(),
            Some(
                r#"<package><metadata><dc:title id="title">Nuovo &amp; titolo</dc:title></metadata></package>"#
            )
        );
    }
}
