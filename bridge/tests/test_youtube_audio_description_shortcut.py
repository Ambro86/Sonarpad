import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
MENU = (ROOT / "src" / "menu.rs").read_text(encoding="utf-8")
MAIN = (ROOT / "src" / "main.rs").read_text(encoding="utf-8")
ACCESSIBILITY = (ROOT / "src" / "accessibility.rs").read_text(encoding="utf-8")
YOUTUBE = (ROOT / "src" / "app_windows" / "youtube_transcript_window.rs").read_text(
    encoding="utf-8"
)
WINDOW = (ROOT / "src" / "app_windows" / "audio_description_window.rs").read_text(
    encoding="utf-8"
)


class YouTubeAudioDescriptionShortcutTests(unittest.TestCase):
    def test_playback_menu_exposes_command_for_supported_video_contexts(self):
        self.assertIn("IDM_PLAYBACK_CREATE_AUDIO_DESCRIPTION", MENU)
        self.assertIn("stream_video_audio_description_available", MENU)
        self.assertIn("local_video_audio_description_available", MENU)
        self.assertIn("rai_la7_audio_description_available", MENU)
        self.assertIn("can_create_audio_description_from_active_stream", MENU)
        self.assertIn("can_create_audio_description_from_active_youtube", MENU)
        self.assertIn("RaiAudioOrigin::RaiPlay", MENU)
        self.assertIn("RaiAudioOrigin::La7Play", MENU)
        self.assertIn("file_handler::is_video_path", MENU)
        self.assertIn("IDM_PLAYBACK_CREATE_AUDIO_DESCRIPTION =>", MAIN)

    def test_youtube_context_refreshes_playback_menu_after_registration(self):
        marker = "set_active_stream_save_context(StreamSaveContext"
        start = YOUTUBE.index(marker)
        following = YOUTUBE[start : start + 1_200]
        self.assertIn("crate::menu::update_playback_menu(parent, true);", following)

    def test_shortcut_downloads_muxed_video_to_configured_media_folder(self):
        self.assertIn("state.settings.media_save_folder.clone()", YOUTUBE)
        self.assertIn("crate::settings::default_media_save_folder()", YOUTUBE)
        self.assertIn("download_options.format = StreamOutputFormat::Auto;", YOUTUBE)
        self.assertIn("prefer_video: true", YOUTUBE)
        self.assertIn("unique_stream_media_path", YOUTUBE)

    def test_downloaded_file_prefills_audio_description_window(self):
        self.assertIn(
            "audio_description_window::open_with_input(parent, target_path)", YOUTUBE
        )
        self.assertIn("const WM_AD_SET_INPUT", WINDOW)
        self.assertIn("set_path((*pointer).input, &input_path);", WINDOW)
        set_input = WINDOW[
            WINDOW.index("WM_AD_SET_INPUT =>"):
            WINDOW.index("WM_AD_SET_INPUT =>") + 3000
        ]
        self.assertRegex(
            "".join(set_input.split()),
            r"&default_output\(\(\*pointer\)\.parent,&input_path,checkbox_checked\(\(\*pointer\)\.create_video_checkbox\),?\)",
        )

    def test_local_video_prefills_window_without_downloading(self):
        self.assertIn("fn open_audio_description_from_current_context", MAIN)
        self.assertIn("filter(|path| file_handler::is_video_path(path.as_path()))", MAIN)
        self.assertIn("audio_description_window::open_with_input(hwnd, path)", MAIN)
        local_branch = MAIN.index("Audio description shortcut: opening local video")
        youtube_branch = MAIN.index("Audio description shortcut: downloading active YouTube video")
        self.assertLess(local_branch, youtube_branch)

    def test_single_unique_shortcut_is_shown_in_both_menus(self):
        self.assertGreaterEqual(MENU.count("Ctrl+Shift+I"), 2)
        self.assertIn("key: 'I' as u16", MAIN)
        self.assertIn("cmd: IDM_TOOLS_CREATE_AUDIO_DESCRIPTION as u16", MAIN)
        self.assertEqual(MAIN.count("cmd: IDM_TOOLS_CREATE_AUDIO_DESCRIPTION as u16"), 1)
        self.assertIn("key == 'I' as u16 && ctrl_down && shift_down && !alt_down", MAIN)
        options = (ROOT / "src" / "app_windows" / "options_window.rs").read_text(
            encoding="utf-8"
        )
        self.assertIn(
            "ShortcutBinding::new(true, true, false, 'I' as u16)", options
        )

    def test_generic_ytdlp_video_stream_reuses_stream_download_and_prefills_window(self):
        self.assertIn("can_create_audio_description_from_active_stream", YOUTUBE)
        self.assertIn("!is_youtube_stream_url(active_url)", YOUTUBE)
        self.assertIn("context.prefer_video", YOUTUBE)
        self.assertIn("download_active_streaming_video_for_audio_description", YOUTUBE)
        self.assertIn("download_stream_context_for_audio_description", YOUTUBE)
        self.assertIn('"stream_audio_description"', YOUTUBE)
        self.assertIn("prefer_video: true", YOUTUBE)
        self.assertIn("audio_description_window::open_with_input(parent, target_path)", YOUTUBE)
        self.assertIn("downloading active yt-dlp video stream", MAIN)

    def test_raiplay_and_la7_vod_reuse_existing_mp4_exporter(self):
        self.assertIn("fn download_active_rai_la7_for_audio_description", MAIN)
        self.assertIn("RaiAudioOrigin::RaiPlay | RaiAudioOrigin::La7Play", MAIN)
        self.assertIn("RaiPlaySaveMode::Mp4", MAIN)
        self.assertIn("remux_media_file_to_mp4_with_preferred_audio_stream", MAIN)
        self.assertIn("state.settings.media_save_folder.clone()", MAIN)
        self.assertIn("unique_stream_media_path", MAIN)
        self.assertIn("open_audio_description: true", MAIN)

    def test_raiplay_and_la7_open_prefilled_window_only_after_success(self):
        result_handler = MAIN.index("WM_PODCAST_EPISODE_SAVE_RESULT =>")
        following = MAIN[result_handler : result_handler + 3_500]
        error_branch = following.index("if let Some(err) = payload.error")
        open_branch = following.index("if payload.open_audio_description")
        self.assertLess(error_branch, open_branch)
        self.assertIn("audio_description_window::open_with_input", following)
        self.assertIn("payload.target_path", following)

    def test_live_raiplay_and_la7_are_not_offered(self):
        self.assertIn("state.active_mpv_is_live_tv", MAIN)
        self.assertIn("!state.raiplay_live_audio_variants.is_empty()", MAIN)
        main_compact = "".join(MAIN.split())
        self.assertIn("ifis_live||!matches!", main_compact)
        self.assertIn("!live_tv_playback", MENU)
        self.assertIn("state.raiplay_live_audio_variants.is_empty()", MENU)

    def test_player_keeps_ctrl_i_for_time_but_releases_ctrl_shift_i(self):
        self.assertIn(
            "ctrl_down && !alt_down && !shift_down && vk == 'I' as u32",
            ACCESSIBILITY,
        )
        self.assertNotIn(
            "ctrl_down && vk == 'I' as u32 => PlayerCommand::AnnounceTime",
            ACCESSIBILITY,
        )
        self.assertIn(
            "key == 'I' as u16 && ctrl_down && shift_down && !alt_down",
            MAIN,
        )


    def test_audio_only_context_opens_a_fresh_empty_creation_window(self):
        self.assertIn(
            "filter(|path| file_handler::is_video_path(path.as_path()))",
            MAIN,
        )
        self.assertIn(
            'log_debug("Audio description shortcut: opening empty window");',
            MAIN,
        )
        self.assertIn("SendMessageW(hwnd, WM_AD_RESET_NEW", WINDOW)
        self.assertIn('set_text(state.input, "");', WINDOW)
        self.assertIn('set_text(state.output, "");', WINDOW)

    def test_reused_creation_window_clears_stale_player_and_project_state(self):
        self.assertIn("clear_player_return_for_window(hwnd);", WINDOW)
        self.assertIn("state.return_to_editor_after_player = false;", WINDOW)
        self.assertIn("state.source_player_path = None;", WINDOW)
        self.assertIn("state.resume_checkpoint_path = None;", WINDOW)
        self.assertIn("state.resume_mode = false;", WINDOW)
        self.assertIn('set_text(state.character_catalog_name_edit, "");', WINDOW)
        self.assertGreaterEqual(WINDOW.count("ShowWindow(hwnd, SW_SHOW);"), 2)

    def test_prefill_api_rejects_audio_and_other_non_video_paths(self):
        self.assertIn(
            "!input_path.is_file() || !crate::file_handler::is_video_path(input_path.as_path())",
            WINDOW,
        )
        self.assertIn(
            "refusing automatic non-video input",
            WINDOW,
        )
        self.assertIn("state.source_player_path = Some(input_path.clone());", WINDOW)


if __name__ == "__main__":
    unittest.main()
