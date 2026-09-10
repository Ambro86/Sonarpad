import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
PROJECT = (
    ROOT / "src" / "app_windows" / "audio_description_project_window.rs"
).read_text(encoding="utf-8")
WINDOW = (
    ROOT / "src" / "app_windows" / "audio_description_window.rs"
).read_text(encoding="utf-8")
MAIN = (ROOT / "src" / "main.rs").read_text(encoding="utf-8")
AUDIO = (ROOT / "src" / "audio_description.rs").read_text(encoding="utf-8")


class AudioDescriptionProjectWindowAccessibilityTests(unittest.TestCase):
    def test_project_opens_and_reopens_on_inserted_descriptions_list(self):
        self.assertIn("fn focus_descriptions_list", PROJECT)
        self.assertIn("SetFocus((*pointer).list);", PROJECT)
        self.assertIn("focus_descriptions_list(existing);", PROJECT)
        self.assertIn("SetFocus(list);", PROJECT)

    def test_tab_and_shift_tab_leave_multiline_description_editor(self):
        navigation = PROJECT[
            PROJECT.index("pub fn handle_navigation"):
            PROJECT.index("fn get_text")
        ]
        self.assertIn("key == VK_TAB.0 as u32", navigation)
        self.assertIn("VK_SHIFT", navigation)
        self.assertIn("get_next_dlg_tab_item_safe", navigation)
        self.assertIn("set_focus_safe(next)", navigation)
        self.assertIn("focus_descriptions_list(hwnd)", navigation)

    def test_navigation_only_handles_messages_from_project_window(self):
        navigation = PROJECT[
            PROJECT.index("pub fn handle_navigation"):
            PROJECT.index("fn get_text")
        ]
        self.assertIn("msg.hwnd != hwnd", navigation)
        self.assertIn("!crate::is_child_safe(hwnd, msg.hwnd)", navigation)

    def test_project_is_registered_before_main_focus_timers_can_steal_focus(self):
        create = PROJECT[PROJECT.index("WM_CREATE =>"):PROJECT.index("WM_COMMAND =>")]
        register = create.index("state.audio_description_project_window = hwnd")
        focus = create.index("SetFocus(list);")
        self.assertLess(register, focus)
        self.assertIn("state.audio_description_project_window.0 == 0", MAIN)
        self.assertIn("!IsWindowVisible(state.audio_description_project_window).as_bool()", MAIN)
        self.assertIn("audio_description_window::blocks_parent_focus", MAIN)

    def test_explorer_open_is_deferred_while_creation_window_is_active(self):
        self.assertIn("COPYDATA_RESULT_DEFERRED_MODAL", MAIN)
        self.assertIn("fn defer_copydata_paths_while_main_window_disabled", MAIN)
        self.assertIn(
            "if defer_copydata_paths_while_main_window_disabled(hwnd, &paths)",
            MAIN,
        )
        self.assertIn("return LRESULT(COPYDATA_RESULT_DEFERRED_MODAL);", MAIN)
        self.assertIn("fn process_deferred_modal_copydata_paths_if_ready", MAIN)
        self.assertIn("open_copydata_paths(hwnd, pending_paths);", MAIN)
        self.assertIn("should_focus_existing_window_after_copydata(copydata_result.0)", MAIN)
        self.assertIn(
            "Existing Sonarpad instance deferred the file open because a modal dialog is active",
            MAIN,
        )

    def test_creation_window_survives_show_desktop_focus_round_trip(self):
        self.assertIn("pub(crate) fn restore_on_parent_activation", WINDOW)
        self.assertIn("if foreground != parent", WINDOW)
        self.assertIn("ShowWindow(window, SW_SHOW);", WINDOW)
        self.assertIn("SetForegroundWindow(window);", WINDOW)
        self.assertGreaterEqual(
            MAIN.count("audio_description_window::restore_on_parent_activation(hwnd)"),
            3,
        )
        self.assertGreaterEqual(
            MAIN.count("audio_description_window::blocks_parent_focus("),
            2,
        )

    def test_project_is_owned_by_creation_window_and_returns_focus_on_close(self):
        self.assertIn("open_from_dialog(parent: HWND, owner: HWND)", PROJECT)
        self.assertIn("hwndOwner: owner", PROJECT)
        self.assertIn("parent, owner, project_path.to_path_buf(), project", PROJECT)
        self.assertIn("SetForegroundWindow(owner);", PROJECT)
        self.assertIn("SetFocus(owner);", PROJECT)
        call = WINDOW[
            WINDOW.index("audio_description_project_window::open_from_dialog"):
            WINDOW.index("audio_description_project_window::open_from_dialog") + 220
        ]
        self.assertIn("state.parent", call)
        self.assertIn("hwnd", call)

    def test_space_enter_and_context_menu_choose_exported_or_modified_preview(self):
        navigation = PROJECT[
            PROJECT.index("pub fn handle_navigation"):
            PROJECT.index("fn get_text")
        ]
        self.assertIn("VK_SPACE", navigation)
        self.assertIn("VK_RETURN", navigation)
        preview = PROJECT[
            PROJECT.index("fn stop_preview"):
            PROJECT.index("fn delete_selected_description")
        ]
        self.assertIn("fn play_exported_description", preview)
        self.assertIn("state.project.output_mp3_path", preview)
        self.assertIn("description.output_start_sec", preview)
        self.assertIn(".output_end_sec", preview)
        self.assertIn(
            "ifdescription.extended_pause||draft!=description.text||description.rendered_text!=description.text{",
            "".join(preview.split()),
        )
        self.assertIn("start_draft_preview(hwnd, state, index, draft)", preview)
        self.assertIn("synthesize_audio_description_project_preview", PROJECT)
        self.assertIn("WM_PROJECT_DRAFT_PREVIEW_DONE", PROJECT)
        self.assertIn("audio_description.project.play_description", PROJECT)
        self.assertIn("ID_CONTEXT_PLAY", PROJECT)

    def test_modified_preview_overlays_ducked_original_audio_without_saving(self):
        preview = PROJECT[
            PROJECT.index("fn play_modified_preview"):
            PROJECT.index("fn play_selected_description")
        ]
        self.assertIn("BassOutput::start(preview_audio.path()", preview)
        self.assertIn("BassOutput::start_with_ffmpeg_at", preview)
        self.assertIn("source_start", preview)
        self.assertNotIn("source.seek_to_seconds(source_start)", preview)
        self.assertIn("state.project.source_path", preview)
        self.assertIn("state.project.ducking_db", preview)
        self.assertIn("description.source_start_sec", preview)
        self.assertIn("description.extended_pause", preview)
        self.assertNotIn("save_audio_description_project", preview)
        synthesis = AUDIO[
            AUDIO.index("pub fn synthesize_audio_description_project_preview"):
            AUDIO.index("pub fn apply_audio_description_project_batch_edits")
        ]
        self.assertIn("synthesize_description(", synthesis)
        self.assertIn("modified_description_preview.wav", synthesis)
        self.assertNotIn("save_audio_description_project", synthesis)



    def test_modified_preview_uses_precise_ffmpeg_start_without_bass_reseek(self):
        bass_output = (ROOT / "src" / "bass_output.rs").read_text(encoding="utf-8")
        bass_stream = (ROOT / "src" / "bass_ffmpeg_stream.rs").read_text(encoding="utf-8")
        ffmpeg_source = (ROOT / "src" / "ffmpeg_source.rs").read_text(encoding="utf-8")
        self.assertIn("pub fn start_with_ffmpeg_at", bass_output)
        self.assertIn("start_seconds: f64", bass_stream)
        self.assertIn("FfmpegSource::try_new_at", bass_stream)
        self.assertIn("pub fn try_new_at", ffmpeg_source)
        self.assertIn("Duration::from_secs_f64(start_seconds)", ffmpeg_source)

    def test_applied_but_not_exported_text_still_uses_synthesized_preview(self):
        self.assertIn("pub rendered_text: String", AUDIO)
        loader = AUDIO[
            AUDIO.index("pub fn load_audio_description_project"):
            AUDIO.index("pub fn save_audio_description_project")
        ]
        self.assertIn("description.rendered_text.is_empty()", loader)
        prepare = AUDIO[
            AUDIO.index("fn prepare_audio_description_project_batch_edits("):
            AUDIO.index("pub fn apply_audio_description_project_batch_edits(")
        ]
        apply_fn = AUDIO[
            AUDIO.index("pub fn apply_audio_description_project_batch_edits("):
            AUDIO.index("pub fn apply_reanalyzed_audio_description_project_segment_and_reexport(")
        ]
        self.assertIn("description.text = text.clone()", prepare)
        self.assertNotIn("description.rendered_text =", prepare + apply_fn)
        self.assertIn("prepare_audio_description_project_batch_edits(", apply_fn)
        builder = AUDIO[
            AUDIO.index("fn build_audio_description_project"):
            AUDIO.index("pub fn load_audio_description_project")
        ]
        self.assertIn("rendered_text: description.text.clone()", builder)

    def test_context_menu_deletes_and_saves_selected_description(self):
        self.assertIn("WM_CONTEXTMENU", PROJECT)
        self.assertIn("ID_CONTEXT_DELETE", PROJECT)
        self.assertIn("delete_audio_description_project_description", PROJECT)
        delete = AUDIO[
            AUDIO.index("pub fn delete_audio_description_project_description"):
            AUDIO.index("fn validate_job")
        ]
        self.assertIn("updated.descriptions.remove(index)", delete)
        self.assertIn("save_audio_description_project(project_path, &updated)?", delete)

    def test_apply_synthesizes_checks_duration_then_saves_batch_once(self):
        prepare = AUDIO[
            AUDIO.index("fn prepare_audio_description_project_batch_edits("):
            AUDIO.index("pub fn apply_audio_description_project_batch_edits(")
        ]
        apply_fn = AUDIO[
            AUDIO.index("pub fn apply_audio_description_project_batch_edits("):
            AUDIO.index("pub fn apply_reanalyzed_audio_description_project_segment_and_reexport(")
        ]
        synthesis = prepare.index("synthesize_description(")
        duration_check = prepare.index("validate_audio_description_project_edit_duration(")
        self.assertLess(synthesis, duration_check)
        self.assertLess(duration_check, prepare.index("validation_result?;"))
        self.assertLess(prepare.index("validation_result?;"), prepare.index("description.text = text.clone()"))
        self.assertNotIn("save_audio_description_project(", prepare)
        self.assertLess(
            apply_fn.index("prepare_audio_description_project_batch_edits("),
            apply_fn.index("save_audio_description_project(project_path, &outcome.project)"),
        )
        self.assertRegex("".join(apply_fn.split()), r"None,?\)\?;")
        self.assertIn("AudioDescriptionProjectEditError::TooLong", AUDIO)
        self.assertIn("start_apply(hwnd, state)", PROJECT)
        self.assertIn("WM_PROJECT_APPLY_DONE", PROJECT)

    def test_apply_error_returns_to_project_description_list(self):
        apply_done = PROJECT[
            PROJECT.index("WM_PROJECT_APPLY_DONE =>"):
            PROJECT.index("WM_PROJECT_PROGRESS =>")
        ]
        too_long = apply_done[
            apply_done.index("AudioDescriptionProjectEditError::TooLong"):
            apply_done.index("AudioDescriptionProjectEditError::Other")
        ]
        self.assertIn("show_project_error(hwnd, state.language, &message)", too_long)
        self.assertNotIn("show_error(hwnd", too_long)
        helper = PROJECT[
            PROJECT.index("fn show_project_error"):
            PROJECT.index("pub fn handle_navigation")
        ]
        self.assertIn("MessageBoxW(", helper)
        self.assertIn("crate::watchdog::enter_modal_dialog()", helper)
        self.assertIn("crate::watchdog::exit_modal_dialog()", helper)
        self.assertIn("crate::is_window_handle_valid(hwnd)", helper)
        self.assertIn("SetForegroundWindow(hwnd)", helper)
        self.assertIn("focus_descriptions_list(hwnd)", helper)
        self.assertNotIn("show_error(", helper)
        self.assertNotIn("show_blocking_modal_message_box", helper)


    def test_project_search_is_after_apply_and_before_tts_engine_in_tab_order(self):
        create = PROJECT[PROJECT.index("let apply_button = CreateWindowExW"):PROJECT.index("let voice_label = CreateWindowExW")]
        self.assertLess(create.index("ID_APPLY as isize"), create.index("ID_SEARCH as isize"))
        self.assertLess(create.index("ID_SEARCH as isize"), create.index("ID_SEARCH_BUTTON as isize"))
        self.assertLess(create.index("ID_SEARCH_BUTTON as isize"), create.index("ID_ENGINE as isize"))
        self.assertIn("audio_description.project.search", PROJECT)
        self.assertIn("audio_description.project.search_button", PROJECT)

    def test_project_search_prioritizes_matches_without_hiding_other_descriptions(self):
        self.assertIn("fn description_search_order", PROJECT)
        search = PROJECT[PROJECT.index("fn description_search_order"):PROJECT.index("fn selected_edit_text")]
        self.assertIn("matching.push(index)", search)
        self.assertIn("remaining.push(index)", search)
        self.assertIn("matching.extend(remaining)", search)
        self.assertIn("state.display_order", search)
        command = PROJECT[PROJECT.index("WM_COMMAND =>"):PROJECT.index("WM_CONTEXTMENU =>")]
        self.assertIn(".display_order", command)
        self.assertIn("ID_SEARCH_BUTTON if !state.running => apply_description_search(state)", command)

    def test_project_search_enter_focuses_list_and_empty_search_restores_original_order(self):
        navigation = PROJECT[PROJECT.index("pub fn handle_navigation"):PROJECT.index("fn get_text")]
        self.assertIn("search_edit == Some(current)", navigation)
        self.assertIn("apply_description_search(&mut *pointer)", navigation)
        search = PROJECT[PROJECT.index("fn description_search_order"):PROJECT.index("fn selected_edit_text")]
        self.assertIn("if query.is_empty()", search)
        self.assertIn("(0..project.descriptions.len()).collect()", search)
        self.assertIn("SetFocus(state.list)", search)
        command = PROJECT[PROJECT.index("WM_COMMAND =>"):PROJECT.index("WM_CONTEXTMENU =>")]
        self.assertIn("ID_SEARCH if notification == EN_CHANGE", command)
        self.assertIn("get_text(state.search_edit).trim().is_empty()", command)

    def test_project_exports_srt_and_vtt_after_mp3_with_final_timeline(self):
        create = PROJECT[PROJECT.index("let export_button = CreateWindowExW"):PROJECT.index("let cancel_button = CreateWindowExW")]
        self.assertLess(create.index("ID_EXPORT as isize"), create.index("ID_EXPORT_SRT as isize"))
        self.assertLess(create.index("ID_EXPORT_SRT as isize"), create.index("ID_EXPORT_VTT as isize"))
        self.assertIn("audio_description.project.export_srt", PROJECT)
        self.assertIn("audio_description.project.export_vtt", PROJECT)
        self.assertIn("fn render_project_srt", PROJECT)
        self.assertIn("fn render_project_vtt", PROJECT)
        self.assertIn("description.output_start_sec", PROJECT)
        self.assertIn("description.output_end_sec", PROJECT)
        self.assertIn('String::from("WEBVTT\\r\\n\\r\\n")', PROJECT)
        self.assertIn("GetSaveFileNameW", PROJECT)
        self.assertIn("OFN_OVERWRITEPROMPT", PROJECT)
        command = PROJECT[PROJECT.index("WM_COMMAND =>"):PROJECT.index("WM_CONTEXTMENU =>")]
        self.assertIn('ID_EXPORT_SRT if !state.running => export_project_subtitles(hwnd, state, "srt")', command)
        self.assertIn('ID_EXPORT_VTT if !state.running => export_project_subtitles(hwnd, state, "vtt")', command)
        self.assertIn("has_unapplied_edit(state)", PROJECT)

    def test_export_result_messages_return_to_project_description_list(self):
        done = PROJECT[
            PROJECT.index("WM_PROJECT_DONE =>"):
            PROJECT.index("WM_SETFOCUS =>")
        ]
        self.assertIn(
            "show_project_info(hwnd, state.language, &labels(state.language).success)",
            done,
        )
        self.assertNotIn("show_info(state.parent", done)
        self.assertIn("show_project_error(hwnd, state.language, &error)", done)
        helper = PROJECT[
            PROJECT.index("fn show_project_info"):
            PROJECT.index("pub fn handle_navigation")
        ]
        self.assertIn("MessageBoxW(", helper)
        self.assertIn("MB_ICONINFORMATION", helper)
        self.assertIn("crate::watchdog::enter_modal_dialog()", helper)
        self.assertIn("crate::watchdog::exit_modal_dialog()", helper)
        self.assertIn("crate::is_window_handle_valid(hwnd)", helper)
        self.assertIn("SetForegroundWindow(hwnd)", helper)
        self.assertIn("focus_descriptions_list(hwnd)", helper)
        self.assertNotIn("show_info(", helper)

    def test_selection_change_keeps_unapplied_text_in_memory_without_saving(self):
        command = PROJECT[PROJECT.index("WM_COMMAND =>"):PROJECT.index("WM_CONTEXTMENU =>")]
        selection = command[
            command.index("ID_LIST if notification == LBN_SELCHANGE"):
            command.index("ID_APPLY if !state.running")
        ]
        self.assertIn("capture_current_draft(state)", selection)
        self.assertIn("select_project_description(state, index)", selection)
        self.assertNotIn("save_audio_description_project", selection)
        self.assertNotIn("apply_audio_description_project_batch_edits", selection)
        self.assertIn("drafts: HashMap<usize, String>", PROJECT)
        self.assertIn("apply_before_export", PROJECT)

    def test_apply_text_validates_all_drafts_and_saves_once(self):
        apply = PROJECT[
            PROJECT.index("fn start_apply"):
            PROJECT.index("fn start_voice_change")
        ]
        self.assertIn("capture_current_draft(state)", apply)
        self.assertIn("state.drafts.get(&description.id)", apply)
        self.assertIn("apply_audio_description_project_batch_edits", apply)
        prepare = AUDIO[
            AUDIO.index("fn prepare_audio_description_project_batch_edits("):
            AUDIO.index("pub fn apply_audio_description_project_batch_edits(")
        ]
        batch = AUDIO[
            AUDIO.index("pub fn apply_audio_description_project_batch_edits("):
            AUDIO.index("pub fn apply_reanalyzed_audio_description_project_segment_and_reexport(")
        ]
        self.assertIn("normalized_edits.iter().enumerate()", prepare)
        self.assertIn("for (index, text) in &normalized_edits", prepare)
        self.assertIn("validate_audio_description_project_edit_duration(", prepare)
        self.assertNotIn("save_audio_description_project(", prepare)
        self.assertEqual(batch.count("save_audio_description_project("), 1)
        self.assertIn("save_audio_description_project(project_path, &outcome.project)", batch)
        self.assertIn("applied_count: normalized_edits.len()", prepare)

    def test_batch_apply_focuses_invalid_description_and_keeps_other_drafts(self):
        done = PROJECT[
            PROJECT.index("WM_PROJECT_APPLY_DONE =>"):
            PROJECT.index("WM_PROJECT_VOICES_LOADED =>")
        ]
        self.assertIn("if let Some(index) = batch_error.index", done)
        self.assertIn("refill_list(state, index)", done)
        self.assertIn("SetFocus(state.edit)", done)
        self.assertIn("state.drafts.clear()", done)
        self.assertIn("edit_saved_multiple", done)

    def test_output_player_return_focuses_close_not_start(self):
        returned = WINDOW[
            WINDOW.index("WM_AD_PLAYER_RETURN =>"):
            WINDOW.index("WM_AD_SET_INPUT =>")
        ]
        self.assertIn("return_to_editor_after_player = true", returned)
        self.assertIn("SetFocus((*pointer).close_button);", returned)
        self.assertNotIn("SetFocus((*pointer).start_button);", returned)

    def test_audio_description_windows_block_player_keyboard_shortcuts(self):
        keyboard = MAIN[
            MAIN.index("// Audiobook keyboard controls (ONLY if no secondary window is open)"):
            MAIN.index("// Exclude voice panel controls from player keyboard handling")
        ]
        self.assertIn("audio_description_window::blocks_parent_focus", keyboard)
        self.assertIn("state.audio_description_project_window", keyboard)
        self.assertIn("IsWindowVisible(state.audio_description_project_window)", keyboard)

    def test_voice_panels_stay_hidden_while_mpv_player_is_active(self):
        sync_start = MAIN.index("pub(crate) fn sync_voice_panel_visibility_for_current_document")
        sync_end = MAIN.index("pub(crate) fn refresh_voice_panel", sync_start)
        sync = MAIN[sync_start:sync_end]
        self.assertIn("state.active_mpv_session.is_some()", sync)
        self.assertIn("voice_requested && !player_active", sync)
        self.assertIn("favorites_requested && !player_active", sync)

        tab_start = MAIN.index("fn handle_voice_panel_tab")
        tab_end = MAIN.index("fn is_modifier_vk", tab_start)
        tab = MAIN[tab_start:tab_end]
        self.assertIn("if is_mpv_playback_active(hwnd)", tab)
        self.assertIn("return false;", tab)

        recovery_start = MAIN.index("fn recover_main_window_after_audio_description")
        recovery_end = MAIN.index("fn defer_copydata_paths_while_main_window_disabled", recovery_start)
        recovery = MAIN[recovery_start:recovery_end]
        self.assertIn("sync_voice_panel_visibility_for_current_document(hwnd);", recovery)

    def test_closing_after_output_preview_returns_to_normal_editor(self):
        destroyed = WINDOW[WINDOW.index("WM_DESTROY =>"):WINDOW.index("WM_NCDESTROY =>")]
        self.assertIn("return_to_editor_after_player", destroyed)
        self.assertIn("source_player_path", destroyed)
        self.assertIn("finish_audio_description_after_output_preview", destroyed)

        cleanup = MAIN[
            MAIN.index("pub(crate) fn finish_audio_description_after_output_preview"):
            MAIN.index("fn pause_active_playback_for_audio_description")
        ]
        self.assertIn("stop_managed_mpv_playback(hwnd)", cleanup)
        self.assertIn("matches!(doc.format, FileFormat::Audiobook)", cleanup)
        self.assertIn("editor_manager::close_document_at(hwnd, index)", cleanup)
        self.assertIn("clear_active_podcast_chapters(hwnd)", cleanup)
        self.assertIn("clear_active_youtube_return_context(hwnd)", cleanup)
        self.assertIn(
            'recover_main_window_after_audio_description(hwnd, "output_preview_cleanup")',
            cleanup,
        )

        recovery = MAIN[
            MAIN.index("fn recover_main_window_after_audio_description"):
            MAIN.index("fn defer_copydata_paths_while_main_window_disabled")
        ]
        self.assertIn("enable_window_safe(hwnd, true)", recovery)
        self.assertIn("WM_CANCELMODE", recovery)
        self.assertIn("GetLastActivePopup(hwnd)", recovery)
        self.assertIn("tracked_progress_modal_while_main_disabled(hwnd)", recovery)
        self.assertIn("has_pending_blocking_modal(hwnd)", recovery)
        self.assertIn("state.alt_menu_suppressed = false", cleanup)
        self.assertIn("state.alt_menu_used_with_key = false", cleanup)
        self.assertIn("DrawMenuBar(hwnd)", cleanup)
        self.assertIn("WM_FOCUS_EDITOR", cleanup)
        self.assertIn("post_message_w_safe", cleanup)
        self.assertNotIn("focus_editor(hwnd);", cleanup)

    def test_hidden_output_preview_window_does_not_block_main_alt_f4(self):
        self.assertIn("pub(crate) fn blocks_main_window_close", WINDOW)
        close_guard = WINDOW[
            WINDOW.index("pub(crate) fn blocks_main_window_close"):
            WINDOW.index("pub(crate) fn restore_on_parent_activation")
        ]
        self.assertIn("!is_hidden_for_output_player(parent, window)", close_guard)

        main_close = MAIN[MAIN.index("WM_CLOSE =>") : MAIN.index("WM_DESTROY =>", MAIN.index("WM_CLOSE =>"))]
        self.assertIn("audio_description_window::blocks_main_window_close", main_close)
        self.assertIn("hidden for output preview; closing application", main_close)
        self.assertIn("stale audio-description window handle", main_close)
        self.assertIn("try_close_app(hwnd);", main_close)

    def test_output_preview_focus_is_deferred_until_audio_description_window_is_destroyed(self):
        destroyed = WINDOW[WINDOW.index("WM_DESTROY =>"):WINDOW.index("WM_NCDESTROY =>")]
        self.assertIn("audio_description_window = HWND(0)", destroyed)
        self.assertIn("finish_audio_description_after_output_preview", destroyed)

        cleanup = MAIN[
            MAIN.index("pub(crate) fn finish_audio_description_after_output_preview"):
            MAIN.index("fn pause_active_playback_for_audio_description")
        ]
        self.assertIn("scheduling deferred editor focus after preview cleanup", cleanup)
        self.assertIn("post_message_w_safe", cleanup)
        self.assertNotIn("bring_window_to_foreground(hwnd);", cleanup)



    def test_creation_window_uses_specific_browse_labels_and_explicit_new_catalog_ui(self):
        self.assertIn('audio_description.browse_input', WINDOW)
        self.assertIn('audio_description.browse_output', WINDOW)
        self.assertIn('labels.input_browse', WINDOW)
        self.assertIn('labels.output_browse', WINDOW)
        self.assertIn('audio_description.character_catalog.new_option', WINDOW)
        self.assertIn('audio_description.character_catalog.selection_label', WINDOW)
        self.assertIn('audio_description.character_catalog.new_name_label', WINDOW)
        self.assertIn('character_catalog_name_edit', WINDOW)
        self.assertIn('suggested_character_catalog_name', WINDOW)
        prepare = WINDOW[
            WINDOW.index('fn prepare_character_catalog'):
            WINDOW.index('fn persist_audio_description_preferences')
        ]
        self.assertIn('get_text(state.character_catalog_name_edit)', prepare)
        self.assertIn('state.character_catalog_name_edit', prepare)
        self.assertIn('show_audio_description_error_and_focus', prepare)
        self.assertNotIn('prompt_user(', prepare)

    def test_project_voice_change_reuses_real_scheduler_and_rebuilds_output_atomically(self):
        self.assertIn('change_audio_description_project_voice', PROJECT)
        self.assertIn('WM_PROJECT_VOICE_DONE', PROJECT)
        self.assertIn('restore_project_voice_selection', PROJECT)
        self.assertIn('show_project_info_with_title', PROJECT)
        validation = AUDIO[
            AUDIO.index('pub fn change_audio_description_project_voice'):
            AUDIO.index('pub fn delete_audio_description_project_description')
        ]
        synth = validation.index('synthesize_description_tasks_parallel(')
        schedule = validation.index('schedule_synthesized_descriptions(')
        export = validation.index('export_audio_description_output(')
        project_save = validation.index('save_audio_description_project(&temporary_project, &updated)')
        commit = validation.index('commit_audio_description_pair(')
        self.assertLess(synth, schedule)
        self.assertLess(schedule, export)
        self.assertLess(export, project_save)
        self.assertIn('project.output_is_video,', validation[export:project_save])
        self.assertLess(project_save, commit)
        self.assertIn('if let Some(first) = dropped.first()', validation)
        self.assertIn('AudioDescriptionProjectVoiceError::DoesNotFit', validation)
        self.assertIn('job.tts_engine = settings.engine;', validation)
        self.assertIn('job.tts_voice = voice.to_string();', validation)
        self.assertIn('let mix_cues: Vec<AudioDescriptionMixCue> = scheduled', validation)
        self.assertIn('temporary_sibling_path(&project.output_mp3_path, "voice")', validation)
        self.assertIn('build_audio_description_project(', validation)
        self.assertIn('&scheduled,', validation)
        self.assertIn('&[],', validation)
        self.assertNotIn('save_audio_description_project(project_path, &updated)', validation)

    def test_project_voice_change_reuses_verified_audio_without_second_synthesis_pass(self):
        validation = AUDIO[
            AUDIO.index('pub fn change_audio_description_project_voice'):
            AUDIO.index('pub fn delete_audio_description_project_description')
        ]
        self.assertEqual(validation.count('synthesize_description_tasks_parallel('), 1)
        schedule = validation.index('schedule_synthesized_descriptions(')
        export = validation.index('export_audio_description_output(')
        between = validation[schedule:export]
        self.assertNotIn('synthesize_description_tasks_parallel(', between)
        self.assertIn('samples: description.samples.clone()', between)

    def test_project_voice_change_is_explicit_and_tab_order_is_accessible(self):
        create = PROJECT[PROJECT.index('WM_CREATE =>'):PROJECT.index('WM_COMMAND =>')]
        edit = create.index('let edit = CreateWindowExW(')
        apply_button = create.index('let apply_button = CreateWindowExW(')
        engine_combo = create.index('let engine_combo = CreateWindowExW(')
        voice_combo = create.index('let voice_combo = CreateWindowExW(')
        change_button = create.index('let change_voice_button = CreateWindowExW(')
        self.assertLess(edit, apply_button)
        self.assertLess(apply_button, engine_combo)
        self.assertLess(engine_combo, voice_combo)
        self.assertLess(voice_combo, change_button)
        command = PROJECT[PROJECT.index('WM_COMMAND =>'):PROJECT.index('WM_CONTEXTMENU =>')]
        self.assertIn('ID_CHANGE_VOICE if !state.running => start_voice_change(hwnd, state)', command)
        self.assertNotIn('ID_VOICE if notification == CBN_SELCHANGE', command)
        self.assertIn('ID_ENGINE if notification == CBN_SELCHANGE', command)

    def test_project_voice_controls_include_speed_volume_and_preview(self):
        create = PROJECT[PROJECT.index('WM_CREATE =>'):PROJECT.index('WM_COMMAND =>')]
        engine_combo = create.index('let engine_combo = CreateWindowExW(')
        voice_combo = create.index('let voice_combo = CreateWindowExW(')
        rate_combo = create.index('let rate_combo = CreateWindowExW(')
        volume_combo = create.index('let volume_combo = CreateWindowExW(')
        test_button = create.index('let test_voice_button = CreateWindowExW(')
        change_button = create.index('let change_voice_button = CreateWindowExW(')
        self.assertLess(engine_combo, voice_combo)
        self.assertLess(voice_combo, rate_combo)
        self.assertLess(rate_combo, volume_combo)
        self.assertLess(volume_combo, test_button)
        self.assertLess(test_button, change_button)
        self.assertIn('tts_tuning.label_speed', PROJECT)
        self.assertIn('tts_tuning.label_volume', PROJECT)
        self.assertIn('audio_description.voice_settings.test', PROJECT)
        command = PROJECT[PROJECT.index('WM_COMMAND =>'):PROJECT.index('WM_CONTEXTMENU =>')]
        self.assertIn('ID_TEST_VOICE if !state.running => test_project_voice(state)', command)

    def test_project_voice_change_applies_and_persists_rate_and_volume_without_rerunning_ai(self):
        validation = AUDIO[
            AUDIO.index('pub fn change_audio_description_project_voice'):
            AUDIO.index('pub fn delete_audio_description_project_description')
        ]
        self.assertIn('job.tts_rate = settings.rate;', validation)
        self.assertIn('job.tts_volume = settings.volume;', validation)
        self.assertIn('.protected_intervals', validation)
        self.assertIn('visual_evidence_time_sec: description.visual_evidence_time_sec', validation)
        self.assertIn('build_audio_description_project(', validation)
        self.assertNotIn('run_audio_description_bridge', validation)
        self.assertNotIn('Gemini', validation)

    def test_project_engine_selection_only_reloads_voices_until_change_button_is_pressed(self):
        command = PROJECT[PROJECT.index('WM_COMMAND =>'):PROJECT.index('WM_CONTEXTMENU =>')]
        engine_branch = command[
            command.index('ID_ENGINE if notification == CBN_SELCHANGE'):
            command.index('ID_CHANGE_VOICE if !state.running')
        ]
        self.assertIn('load_project_voices(hwnd, engine_from_combo(state.engine_combo))', engine_branch)
        self.assertNotIn('start_voice_change', engine_branch)
        self.assertIn('EnableWindow(state.change_voice_button, false)', engine_branch)
        self.assertIn('audio_description.project.change_voice', PROJECT)

    def test_project_voice_loader_supports_all_creation_engines(self):
        loader = PROJECT[
            PROJECT.index('fn load_project_voices'):
            PROJECT.index('fn refill_project_voice_combo')
        ]
        self.assertIn('TtsEngine::Edge', loader)
        self.assertIn('TtsEngine::Sapi5', loader)
        self.assertIn('TtsEngine::Sapi4', loader)
        self.assertIn('TtsEngine::Google', loader)
        create = PROJECT[PROJECT.index('WM_CREATE =>'):PROJECT.index('WM_COMMAND =>')]
        self.assertIn('"options.engine.edge"', create)
        self.assertIn('"options.engine.sapi5"', create)
        self.assertIn('"options.engine.sapi4"', create)
        self.assertIn('"options.engine.google"', create)
        self.assertIn('let project_engine = state.project.tts_engine;', create)
        self.assertIn('load_project_voices(hwnd, project_engine);', create)

    def test_successful_apply_shows_modified_audio_description_dialog(self):
        apply_done = PROJECT[
            PROJECT.index('WM_PROJECT_APPLY_DONE =>'):
            PROJECT.index('WM_PROJECT_VOICES_LOADED =>')
        ]
        self.assertIn('edit_saved_title', apply_done)
        self.assertIn('show_project_info_with_title(', apply_done)
        self.assertIn('audio_description.project.edit_saved_title', PROJECT)

    def test_completion_message_is_owned_by_audio_description_window(self):
        marker = "crate::show_info_owned_by("
        start = WINDOW.index(marker, WINDOW.index("WM_AD_DONE =>"))
        block = WINDOW[start:start + 220]
        self.assertIn("hwnd,", block)
        self.assertIn("state.parent,", block)
        self.assertIn("state.language,", block)

    def test_completion_owned_info_keeps_main_state_separate_from_visual_owner(self):
        self.assertIn("pub(crate) fn show_info_owned_by(", MAIN)
        self.assertIn("show_blocking_modal_message_box_owned(", MAIN)
        self.assertIn("MessageBoxW(owner_hwnd, message, title, flags)", MAIN)
        self.assertIn("set_blocking_modal_active(app_hwnd, Some(kind))", MAIN)

    def test_completion_flow_after_ok_is_unchanged(self):
        start = WINDOW.index("crate::show_info_owned_by(", WINDOW.index("WM_AD_DONE =>"))
        block = WINDOW[start:start + 900]
        self.assertIn("recover_main_window_after_audio_description", block)
        self.assertIn(
            "open_result_in_player(hwnd, state.parent, outcome.output_path.clone());",
            block,
        )


if __name__ == "__main__":
    unittest.main()
