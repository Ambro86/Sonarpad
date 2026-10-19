use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use windows::Win32::Foundation::HWND;

use crate::accessibility::PlayerCommand;
use crate::ebook_formats::{
    DaisyPlaybackCatalog, materialize_daisy_audio, read_daisy_playback_catalog,
};
use crate::settings::Language;
use crate::{audio_player, editor_manager, i18n, with_state};

#[derive(Clone, Debug)]
struct PreparedSegment {
    path: PathBuf,
    begin_secs: f64,
    end_secs: Option<f64>,
}

impl PreparedSegment {
    fn duration_secs(&self) -> Option<f64> {
        self.end_secs.map(|end| (end - self.begin_secs).max(0.0))
    }
}

#[derive(Clone, Debug)]
struct DaisyPlayerContext {
    source_path: PathBuf,
    catalog: DaisyPlaybackCatalog,
    selected_chapter: Option<usize>,
    prepared: Vec<PreparedSegment>,
    current_segment: usize,
    active_audio_path: Option<PathBuf>,
    finished: bool,
}

static DAISY_PLAYER: OnceLock<Mutex<Option<DaisyPlayerContext>>> = OnceLock::new();

fn context() -> &'static Mutex<Option<DaisyPlayerContext>> {
    DAISY_PLAYER.get_or_init(|| Mutex::new(None))
}

fn language(hwnd: HWND) -> Language {
    with_state(hwnd, |state| state.settings.language).unwrap_or_default()
}

fn show_info(hwnd: HWND, key: &str) {
    let language = language(hwnd);
    crate::show_info(hwnd, language, &i18n::tr(language, key));
}

fn catalog_has_audio(catalog: &DaisyPlaybackCatalog) -> bool {
    catalog
        .chapters
        .iter()
        .any(|chapter| !chapter.segments.is_empty())
}

fn choose_chapter(hwnd: HWND, source_path: &Path, catalog: DaisyPlaybackCatalog) -> bool {
    if !catalog_has_audio(&catalog) || catalog.index.is_empty() {
        show_info(hwnd, "daisy_index.no_audio");
        return false;
    }
    let language = language(hwnd);
    let initial_target = context().lock().ok().and_then(|guard| {
        let ctx = guard.as_ref()?;
        (ctx.source_path.as_path() == source_path)
            .then_some(ctx.selected_chapter?)
            .and_then(|index| i32::try_from(index).ok())
    });
    let selected = crate::app_windows::epub_index_window::select_daisy_index_entry(
        hwnd,
        &catalog.index,
        language,
        initial_target,
    );
    let Some(selected) = selected else {
        if let Ok(mut guard) = context().lock() {
            *guard = None;
        }
        return false;
    };
    let chapter_index = usize::try_from(selected).unwrap_or(usize::MAX);
    if chapter_index >= catalog.chapters.len() {
        return false;
    }
    if catalog.chapters[chapter_index].segments.is_empty() {
        show_info(hwnd, "daisy_index.chapter_no_audio");
        if let Ok(mut guard) = context().lock() {
            *guard = Some(DaisyPlayerContext {
                source_path: source_path.to_path_buf(),
                catalog,
                selected_chapter: Some(chapter_index),
                prepared: Vec::new(),
                current_segment: 0,
                active_audio_path: None,
                finished: false,
            });
        }
        return false;
    }
    start_chapter(hwnd, source_path, catalog, chapter_index)
}

fn prepare_chapter(
    source_path: &Path,
    catalog: &DaisyPlaybackCatalog,
    chapter_index: usize,
    language: Language,
) -> Result<Vec<PreparedSegment>, String> {
    let chapter = catalog
        .chapters
        .get(chapter_index)
        .ok_or_else(|| "Invalid DAISY chapter index.".to_string())?;
    let mut prepared = Vec::with_capacity(chapter.playback_segments.len());
    for segment in &chapter.playback_segments {
        let path = materialize_daisy_audio(source_path, &segment.source, language)?;
        let end_secs = segment.clip_end_secs.or_else(|| {
            audio_player::audiobook_duration_secs(&path).map(|duration| duration as f64)
        });
        if end_secs.is_some_and(|end| end <= segment.clip_begin_secs) {
            continue;
        }
        prepared.push(PreparedSegment {
            path,
            begin_secs: segment.clip_begin_secs,
            end_secs,
        });
    }
    if prepared.is_empty() {
        return Err(i18n::tr(language, "daisy_index.chapter_no_audio"));
    }
    Ok(prepared)
}

fn start_chapter(
    hwnd: HWND,
    source_path: &Path,
    catalog: DaisyPlaybackCatalog,
    mut chapter_index: usize,
) -> bool {
    let language = language(hwnd);
    if catalog
        .chapters
        .get(chapter_index)
        .is_some_and(|chapter| chapter.playback_segments.is_empty())
    {
        let Some(next) = adjacent_chapter(&catalog, chapter_index, true) else {
            show_info(hwnd, "daisy_index.chapter_no_audio");
            return false;
        };
        chapter_index = next;
    }
    let prepared = match prepare_chapter(source_path, &catalog, chapter_index, language) {
        Ok(prepared) => prepared,
        Err(message) => {
            crate::show_info(hwnd, language, &message);
            return false;
        }
    };
    activate_chapter(
        hwnd,
        source_path,
        catalog,
        chapter_index,
        prepared,
        0.0,
        false,
    )
}

fn activate_chapter(
    hwnd: HWND,
    source_path: &Path,
    catalog: DaisyPlaybackCatalog,
    chapter_index: usize,
    prepared: Vec<PreparedSegment>,
    target: f64,
    continue_playing: bool,
) -> bool {
    let Some((segment_index, position)) =
        locate_chapter_position(&prepared, target).or_else(|| {
            (target == 0.0)
                .then(|| prepared.first().map(|s| (0, s.begin_secs)))
                .flatten()
        })
    else {
        return false;
    };
    let path = prepared[segment_index].path.clone();
    let title = catalog.chapters[chapter_index].title.clone();
    if !editor_manager::retarget_current_audiobook_document(hwnd, &path, &title) {
        let Some(tab_index) = editor_manager::ensure_audio_document_tab(hwnd, &path) else {
            return false;
        };
        editor_manager::select_tab(hwnd, tab_index);
        editor_manager::retarget_current_audiobook_document(hwnd, &path, &title);
    }
    if let Ok(mut guard) = context().lock() {
        *guard = Some(DaisyPlayerContext {
            source_path: source_path.to_path_buf(),
            catalog,
            selected_chapter: Some(chapter_index),
            prepared,
            current_segment: segment_index,
            active_audio_path: Some(path.clone()),
            finished: false,
        });
    }
    crate::log_debug(&format!(
        "DAISY: activate chapter={chapter_index} segment={segment_index} position={position:.3}s seamless={continue_playing}"
    ));
    if !continue_playing {
        audio_player::start_audiobook_at_precise(hwnd, &path, position);
    }
    true
}

fn start_segment(hwnd: HWND, segment_index: usize, position_secs: Option<f64>) -> bool {
    let Some((path, begin, title)) = context().lock().ok().and_then(|mut guard| {
        let ctx = guard.as_mut()?;
        let segment = ctx.prepared.get(segment_index)?.clone();
        ctx.current_segment = segment_index;
        ctx.active_audio_path = Some(segment.path.clone());
        ctx.finished = false;
        let title = ctx
            .selected_chapter
            .and_then(|index| ctx.catalog.chapters.get(index))
            .map(|chapter| chapter.title.clone())
            .unwrap_or_else(|| "DAISY".to_string());
        Some((
            segment.path,
            position_secs.unwrap_or(segment.begin_secs),
            title,
        ))
    }) else {
        return false;
    };
    if !editor_manager::retarget_current_audiobook_document(hwnd, &path, &title) {
        let Some(index) = editor_manager::ensure_audio_document_tab(hwnd, &path) else {
            return false;
        };
        editor_manager::select_tab(hwnd, index);
        editor_manager::retarget_current_audiobook_document(hwnd, &path, &title);
    }
    audio_player::start_audiobook_at_precise(hwnd, &path, begin);
    true
}

pub(crate) fn open_index_for_document(hwnd: HWND, source_path: &Path) -> bool {
    let language = language(hwnd);
    let catalog = match read_daisy_playback_catalog(source_path, language) {
        Ok(catalog) => catalog,
        Err(error) => {
            crate::log_debug(&format!("DAISY playback catalog: {error}"));
            show_info(hwnd, "daisy_index.no_audio");
            return false;
        }
    };
    choose_chapter(hwnd, source_path, catalog)
}

pub(crate) fn open_index_for_current_document(hwnd: HWND) -> bool {
    let Some(path) = editor_manager::current_daisy_document_path(hwnd) else {
        return false;
    };
    open_index_for_document(hwnd, &path)
}

pub(crate) fn restore_index_after_stop(hwnd: HWND, stopped_path: Option<&Path>) -> bool {
    let Some((source_path, catalog, matches_path)) = context().lock().ok().and_then(|guard| {
        let ctx = guard.as_ref()?;
        let matches_path = stopped_path
            .zip(ctx.active_audio_path.as_deref())
            .is_some_and(|(stopped, active)| stopped == active);
        Some((ctx.source_path.clone(), ctx.catalog.clone(), matches_path))
    }) else {
        return false;
    };
    if !matches_path {
        return false;
    }
    choose_chapter(hwnd, &source_path, catalog);
    true
}

pub(crate) fn is_active_audio_document_path(path: Option<&Path>) -> bool {
    context()
        .lock()
        .ok()
        .and_then(|guard| {
            let ctx = guard.as_ref()?;
            Some(
                path.zip(ctx.active_audio_path.as_deref())
                    .is_some_and(|(candidate, active)| candidate == active),
            )
        })
        .unwrap_or(false)
}

const CONTIGUOUS_CLIP_TOLERANCE_SECS: f64 = 0.125;

fn segments_can_continue_without_restart(
    current: &PreparedSegment,
    next: &PreparedSegment,
) -> bool {
    if current.path != next.path {
        return false;
    }
    let Some(current_end) = current.end_secs else {
        return false;
    };
    (next.begin_secs - current_end).abs() <= CONTIGUOUS_CLIP_TOLERANCE_SECS
}

fn chapter_total_duration(prepared: &[PreparedSegment]) -> Option<f64> {
    prepared.iter().try_fold(0.0, |total, segment| {
        segment.duration_secs().map(|duration| total + duration)
    })
}

fn locate_chapter_position(prepared: &[PreparedSegment], target: f64) -> Option<(usize, f64)> {
    let mut remaining = target.max(0.0);
    for (index, segment) in prepared.iter().enumerate() {
        let duration = segment.duration_secs()?;
        if remaining < duration || index + 1 == prepared.len() {
            return Some((index, segment.begin_secs + remaining.min(duration.max(0.0))));
        }
        remaining -= duration;
    }
    None
}

fn current_chapter_position(hwnd: HWND) -> Option<f64> {
    let (prepared, current_segment, active_path) = context().lock().ok().and_then(|guard| {
        let ctx = guard.as_ref()?;
        Some((
            ctx.prepared.clone(),
            ctx.current_segment,
            ctx.active_audio_path.clone()?,
        ))
    })?;
    let player_position = with_state(hwnd, |state| {
        let player = state.active_audiobook.as_ref()?;
        (player.path == active_path).then(|| audio_player::audiobook_position_secs(player))
    })
    .flatten()?;
    let previous = prepared
        .iter()
        .take(current_segment)
        .try_fold(0.0, |total, segment| {
            segment.duration_secs().map(|duration| total + duration)
        })?;
    let current = prepared.get(current_segment)?;
    Some(previous + (player_position - current.begin_secs).max(0.0))
}

fn seek_chapter_to(hwnd: HWND, target: f64) -> bool {
    let prepared = context()
        .lock()
        .ok()
        .and_then(|guard| guard.as_ref().map(|ctx| ctx.prepared.clone()))
        .unwrap_or_default();
    if prepared.is_empty() {
        return false;
    }
    let total = chapter_total_duration(&prepared).unwrap_or(target.max(0.0));
    let target = target.clamp(0.0, (total - 0.05).max(0.0));
    let Some((segment_index, media_position)) = locate_chapter_position(&prepared, target) else {
        return false;
    };
    start_segment(hwnd, segment_index, Some(media_position))
}

fn adjacent_chapter(
    catalog: &DaisyPlaybackCatalog,
    current: usize,
    forward: bool,
) -> Option<usize> {
    let start = catalog.chapters.get(current)?.playback_start?;
    let candidates = catalog
        .chapters
        .iter()
        .enumerate()
        .filter_map(|(index, chapter)| {
            let candidate = chapter.playback_start?;
            if chapter.playback_segments.is_empty()
                || (if forward {
                    candidate <= start
                } else {
                    candidate >= start
                })
            {
                return None;
            }
            Some((candidate, index))
        });
    if forward {
        candidates.min().map(|(_, index)| index)
    } else {
        // Prefer the first navigation entry when parent and child share a start.
        candidates
            .max_by_key(|(start, index)| (*start, std::cmp::Reverse(*index)))
            .map(|(_, index)| index)
    }
}

fn switch_chapter(hwnd: HWND, delta: isize) -> bool {
    let Some((source_path, catalog, current)) = context().lock().ok().and_then(|guard| {
        let ctx = guard.as_ref()?;
        Some((
            ctx.source_path.clone(),
            ctx.catalog.clone(),
            ctx.selected_chapter?,
        ))
    }) else {
        return false;
    };
    let Some(index) = adjacent_chapter(&catalog, current, delta > 0) else {
        return false;
    };
    start_chapter(hwnd, &source_path, catalog, index)
}

fn resolve_book_seek(
    catalog: &DaisyPlaybackCatalog,
    mut chapter: usize,
    mut prepared: Vec<PreparedSegment>,
    mut target: f64,
    mut prepare: impl FnMut(usize) -> Result<Vec<PreparedSegment>, String>,
) -> Result<Option<(usize, Vec<PreparedSegment>, f64)>, String> {
    loop {
        let Some(total) = chapter_total_duration(&prepared) else {
            return Ok(None);
        };
        let forward = target >= total;
        if target >= 0.0 && !forward {
            break;
        }
        let Some(next) = adjacent_chapter(catalog, chapter, forward) else {
            target = target.clamp(0.0, (total - 0.05).max(0.0));
            break;
        };
        let next_prepared = prepare(next)?;
        if forward {
            target -= total;
        } else {
            let Some(previous_total) = chapter_total_duration(&next_prepared) else {
                return Ok(None);
            };
            target += previous_total;
        }
        chapter = next;
        prepared = next_prepared;
    }
    Ok(Some((chapter, prepared, target)))
}

fn seek_across_chapters(hwnd: HWND, target: f64) -> bool {
    let Some((source, catalog, chapter, prepared)) = context().lock().ok().and_then(|guard| {
        let ctx = guard.as_ref()?;
        Some((
            ctx.source_path.clone(),
            ctx.catalog.clone(),
            ctx.selected_chapter?,
            ctx.prepared.clone(),
        ))
    }) else {
        return false;
    };
    let language = language(hwnd);
    match resolve_book_seek(&catalog, chapter, prepared, target, |index| {
        prepare_chapter(&source, &catalog, index, language)
    }) {
        Ok(Some((chapter, prepared, target))) => {
            activate_chapter(hwnd, &source, catalog, chapter, prepared, target, false)
        }
        result => {
            crate::log_debug(&format!("DAISY: seek unavailable: {result:?}"));
            // Consume the command: never fall back to seeking outside SMIL clips.
            true
        }
    }
}

fn continue_next_chapter(hwnd: HWND, segment: &PreparedSegment, output_stopped: bool) -> bool {
    let Some((source, catalog, current)) = context().lock().ok().and_then(|guard| {
        let ctx = guard.as_ref()?;
        Some((
            ctx.source_path.clone(),
            ctx.catalog.clone(),
            ctx.selected_chapter?,
        ))
    }) else {
        return false;
    };
    let Some(next) = adjacent_chapter(&catalog, current, true) else {
        return false;
    };
    let prepared = match prepare_chapter(&source, &catalog, next, language(hwnd)) {
        Ok(prepared) => prepared,
        Err(error) => {
            crate::log_debug(&format!("DAISY: next chapter {next} unavailable: {error}"));
            return false;
        }
    };
    let seamless = !output_stopped && segments_can_continue_without_restart(segment, &prepared[0]);
    activate_chapter(hwnd, &source, catalog, next, prepared, 0.0, seamless)
}

pub(crate) fn handle_player_command(hwnd: HWND, command: &PlayerCommand) -> bool {
    let active = context()
        .lock()
        .ok()
        .and_then(|guard| guard.as_ref().map(|ctx| !ctx.prepared.is_empty()))
        .unwrap_or(false);
    if !active {
        return false;
    }
    match command {
        PlayerCommand::Stop => {
            let stopped_path = with_state(hwnd, |state| {
                state
                    .active_audiobook
                    .as_ref()
                    .map(|player| player.path.clone())
            })
            .flatten();
            editor_manager::close_current_document(hwnd);
            restore_index_after_stop(hwnd, stopped_path.as_deref());
            true
        }
        PlayerCommand::Seek(delta) => {
            let Some(current) = current_chapter_position(hwnd) else {
                return false;
            };
            seek_across_chapters(hwnd, current + *delta as f64)
        }
        PlayerCommand::SeekToStart => seek_chapter_to(hwnd, 0.0),
        PlayerCommand::SeekToEnd => {
            let total = context()
                .lock()
                .ok()
                .and_then(|guard| {
                    guard
                        .as_ref()
                        .and_then(|ctx| chapter_total_duration(&ctx.prepared))
                })
                .unwrap_or(0.0);
            seek_chapter_to(hwnd, total)
        }
        PlayerCommand::ChapterPrev | PlayerCommand::TrackPrev => switch_chapter(hwnd, -1),
        PlayerCommand::ChapterNext | PlayerCommand::TrackNext => switch_chapter(hwnd, 1),
        PlayerCommand::TogglePause => {
            let finished = context()
                .lock()
                .ok()
                .and_then(|guard| guard.as_ref().map(|ctx| ctx.finished))
                .unwrap_or(false);
            if finished {
                seek_chapter_to(hwnd, 0.0)
            } else {
                false
            }
        }
        _ => false,
    }
}

pub(crate) fn handle_playback_timer(hwnd: HWND) -> bool {
    let Some((segment, segment_index, prepared_len, active_path, finished)) =
        context().lock().ok().and_then(|guard| {
            let ctx = guard.as_ref()?;
            let segment = ctx.prepared.get(ctx.current_segment)?.clone();
            Some((
                segment,
                ctx.current_segment,
                ctx.prepared.len(),
                ctx.active_audio_path.clone()?,
                ctx.finished,
            ))
        })
    else {
        return false;
    };
    if finished {
        return true;
    }
    let playback = with_state(hwnd, |state| {
        let player = state.active_audiobook.as_ref()?;
        if player.path != active_path {
            return None;
        }
        Some((
            audio_player::audiobook_position_secs(player),
            player.is_paused,
        ))
    })
    .flatten();
    if playback.is_some_and(|(_position, paused)| paused) {
        return true;
    }
    let output_stopped = audio_player::audiobook_output_stopped(hwnd).unwrap_or(false);
    let reached_clip_end = playback
        .map(|(position, paused)| {
            !paused && segment.end_secs.is_some_and(|end| position + 0.05 >= end)
        })
        .unwrap_or(false);
    if !reached_clip_end && !output_stopped {
        return true;
    }
    if segment_index + 1 < prepared_len {
        let next_index = segment_index + 1;
        let next_segment = context().lock().ok().and_then(|guard| {
            guard
                .as_ref()
                .and_then(|ctx| ctx.prepared.get(next_index).cloned())
        });
        let playback_position = playback.map(|(position, _paused)| position);
        if let Some(next) = next_segment.as_ref()
            && segments_can_continue_without_restart(&segment, next)
            && !output_stopped
        {
            crate::log_debug(&format!(
                "DAISY: seamless SMIL transition segment {} -> {} path={} current_end={:.3}s next_begin={:.3}s player_position={}",
                segment_index,
                next_index,
                segment.path.display(),
                segment.end_secs.unwrap_or(segment.begin_secs),
                next.begin_secs,
                playback_position
                    .map(|position| format!("{position:.3}s"))
                    .unwrap_or_else(|| "unknown".to_string()),
            ));
            if let Ok(mut guard) = context().lock()
                && let Some(ctx) = guard.as_mut()
            {
                ctx.current_segment = next_index;
                ctx.active_audio_path = Some(next.path.clone());
                ctx.finished = false;
            }
            return true;
        }
        if let Some(next) = next_segment.as_ref() {
            crate::log_debug(&format!(
                "DAISY: restarting for SMIL transition segment {} -> {} current_path={} next_path={} current_end={} next_begin={:.3}s player_position={} output_stopped={}",
                segment_index,
                next_index,
                segment.path.display(),
                next.path.display(),
                segment
                    .end_secs
                    .map(|end| format!("{end:.3}s"))
                    .unwrap_or_else(|| "none".to_string()),
                next.begin_secs,
                playback_position
                    .map(|position| format!("{position:.3}s"))
                    .unwrap_or_else(|| "unknown".to_string()),
                output_stopped,
            ));
        }
        start_segment(hwnd, next_index, None);
        return true;
    }
    if continue_next_chapter(hwnd, &segment, output_stopped) {
        return true;
    }
    audio_player::pause_audiobook_if_playing(hwnd);
    if let Ok(mut guard) = context().lock()
        && let Some(ctx) = guard.as_mut()
    {
        ctx.finished = true;
    }
    let language = language(hwnd);
    crate::accessibility::screen_reader_speak(&i18n::tr(language, "daisy_index.finished"));
    true
}

#[cfg(test)]
mod tests {
    use super::{
        PreparedSegment, chapter_total_duration, locate_chapter_position,
        segments_can_continue_without_restart,
    };
    use super::{adjacent_chapter, prepare_chapter, resolve_book_seek};
    use crate::ebook_formats::{DaisyAudioSegment, DaisyPlaybackCatalog, DaisyPlaybackChapter};
    use crate::settings::Language;
    use std::path::PathBuf;

    fn catalog(starts: &[Option<usize>]) -> DaisyPlaybackCatalog {
        DaisyPlaybackCatalog {
            index: Vec::new(),
            chapters: starts
                .iter()
                .enumerate()
                .map(|(index, start)| {
                    let segments = if start.is_some() {
                        vec![DaisyAudioSegment {
                            source: format!("{index}.wav"),
                            clip_begin_secs: 0.0,
                            clip_end_secs: Some(10.0),
                        }]
                    } else {
                        Vec::new()
                    };
                    DaisyPlaybackChapter {
                        title: format!("Chapter {index}"),
                        segments: segments.clone(),
                        playback_start: *start,
                        playback_segments: segments,
                    }
                })
                .collect(),
        }
    }

    #[test]
    fn continuous_order_skips_empty_entries_and_duplicate_parent_targets() {
        let book = catalog(&[Some(0), Some(0), None, Some(4), Some(8)]);
        assert_eq!(adjacent_chapter(&book, 0, true), Some(3));
        assert_eq!(adjacent_chapter(&book, 1, true), Some(3));
        assert_eq!(adjacent_chapter(&book, 3, false), Some(0));
        assert_eq!(adjacent_chapter(&book, 4, true), None);
        assert_eq!(adjacent_chapter(&book, 0, false), None);
    }

    #[test]
    fn timed_seeks_cross_multiple_chapters_both_ways_and_clamp_at_book_edges() {
        let book = catalog(&[Some(0), Some(1), Some(2)]);
        let prepare = |index: usize| Ok(vec![segment(&format!("{index}.wav"), 2.0, 12.0)]);
        for (chapter, target, expected_chapter, expected_position) in [
            (0, 13.0, 1, 3.0),
            (0, 25.0, 2, 5.0),
            (2, -15.0, 0, 5.0),
            (1, -3.0, 0, 7.0),
            (0, 10.0, 1, 0.0),
            (0, -100.0, 0, 0.0),
            (2, 100.0, 2, 9.95),
        ] {
            let (resolved, clips, position) = resolve_book_seek(
                &book,
                chapter,
                prepare(chapter).expect("prepare"),
                target,
                prepare,
            )
            .expect("seek")
            .expect("position");
            assert_eq!(resolved, expected_chapter);
            assert!((position - expected_position).abs() < 0.0001);
            assert_eq!(
                locate_chapter_position(&clips, position),
                Some((0, position + 2.0))
            );
        }
    }

    #[test]
    fn unavailable_or_unknown_chapter_does_not_fall_back_to_raw_file_seek() {
        let book = catalog(&[Some(0), Some(1)]);
        let clips = vec![segment("a.wav", 0.0, 10.0)];
        assert!(
            resolve_book_seek(&book, 0, clips.clone(), 15.0, |_| Err(
                "missing audio".into()
            ))
            .is_err()
        );
        let unknown = vec![PreparedSegment {
            path: "unknown.wav".into(),
            begin_secs: 0.0,
            end_secs: None,
        }];
        assert!(
            resolve_book_seek(&book, 0, clips, 15.0, |_| Ok(unknown.clone()))
                .expect("no error")
                .is_none()
        );
    }

    #[test]
    #[ignore = "Requires SONARPAD_DAISY_TEST_PATH and a real audio output device"]
    fn real_daisy_audio_playback_smoke() {
        use rodio::Source;
        use std::time::Duration;
        let path = PathBuf::from(std::env::var("SONARPAD_DAISY_TEST_PATH").expect("sample path"));
        let book = crate::ebook_formats::read_daisy_playback_catalog(&path, Language::English)
            .expect("real catalog");
        let first = book
            .chapters
            .iter()
            .position(|c| !c.playback_segments.is_empty())
            .expect("audio chapter");
        let prepare = |index| prepare_chapter(&path, &book, index, Language::English);
        let clips = prepare(first).expect("first chapter");
        let duration = chapter_total_duration(&clips).expect("known duration");
        let next = adjacent_chapter(&book, first, true).expect("next chapter");
        let (resolved, next_clips, target) =
            resolve_book_seek(&book, first, clips.clone(), duration + 0.25, prepare)
                .expect("forward seek")
                .expect("position");
        assert_eq!(resolved, next);
        assert!((target - 0.25).abs() < 0.001);
        let (back, _, position) =
            resolve_book_seek(&book, next, next_clips.clone(), -0.25, prepare)
                .expect("backward seek")
                .expect("position");
        assert_eq!(back, first);
        assert!((position - (duration - 0.25)).abs() < 0.001);
        let stream = rodio::OutputStreamBuilder::open_default_stream().expect("audio output");
        let sink = rodio::Sink::connect_new(stream.mixer());
        // Decode and audibly play real clips around the chapter boundary.
        for (segments, target) in [(&clips, (duration - 1.0).max(0.0)), (&next_clips, 0.0)] {
            let (index, media_position) =
                locate_chapter_position(segments, target).expect("media position");
            let clip = &segments[index];
            let decoder =
                rodio::Decoder::try_from(std::fs::File::open(&clip.path).expect("audio file"))
                    .expect("decode audio");
            sink.append(
                decoder
                    .skip_duration(Duration::from_secs_f64(media_position))
                    .take_duration(Duration::from_secs_f64(
                        (clip.end_secs.expect("end") - media_position).min(1.0),
                    )),
            );
        }
        sink.sleep_until_end();
        println!(
            "Real DAISY: {} chapters, forward {} -> {}, backward {} -> {}, audio decoded and played",
            book.chapters.len(),
            first,
            next,
            next,
            first
        );
    }

    fn segment(name: &str, begin: f64, end: f64) -> PreparedSegment {
        PreparedSegment {
            path: PathBuf::from(name),
            begin_secs: begin,
            end_secs: Some(end),
        }
    }

    #[test]
    fn chapter_position_maps_across_multiple_audio_files() {
        let segments = vec![segment("a.mp3", 10.0, 15.0), segment("b.mp3", 2.0, 9.0)];
        assert_eq!(chapter_total_duration(&segments), Some(12.0));
        assert_eq!(locate_chapter_position(&segments, 0.0), Some((0, 10.0)));
        assert_eq!(locate_chapter_position(&segments, 6.5), Some((1, 3.5)));
    }

    #[test]
    fn contiguous_smil_clips_in_same_audio_continue_without_restart() {
        let first = segment("chapter.mp3", 0.0, 3.893);
        let exact_next = segment("chapter.mp3", 3.893, 7.779);
        let rounded_next = segment("chapter.mp3", 3.900, 7.779);
        assert!(segments_can_continue_without_restart(&first, &exact_next));
        assert!(segments_can_continue_without_restart(&first, &rounded_next));
    }

    #[test]
    fn smil_transition_restarts_for_real_gap_or_different_audio_file() {
        let first = segment("chapter.mp3", 0.0, 3.893);
        let gap = segment("chapter.mp3", 4.500, 7.779);
        let other_file = segment("next.mp3", 3.893, 7.779);
        assert!(!segments_can_continue_without_restart(&first, &gap));
        assert!(!segments_can_continue_without_restart(&first, &other_file));
    }
}
