from __future__ import annotations

import unittest
from pathlib import Path


class WindowsYoutubeSaveArchitectureTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.root = Path(__file__).resolve().parents[2]
        cls.youtube = (cls.root / "src" / "app_windows" / "youtube_transcript_window.rs").read_text(
            encoding="utf-8"
        )

    def test_initial_stream_dialog_no_longer_contains_save_format_or_quality_controls(self):
        start = self.youtube.index("fn stream_dialog_wndproc_inner(")
        end = self.youtube.index("fn show_stream_dialog(", start)
        block = self.youtube[start:end]
        self.assertNotIn("STREAM_ID_FORMAT", block)
        self.assertNotIn("STREAM_ID_QUALITY", block)
        self.assertNotIn("let format_combo = CreateWindowExW(", block)
        self.assertNotIn("let quality_combo = CreateWindowExW(", block)
        self.assertIn("default_save_format: init.default_format", block)
        self.assertIn("quality = StreamQualitySelection::Original", block)

    def test_save_media_dialog_owns_format_and_quality_selection(self):
        start = self.youtube.index("fn stream_save_options_wndproc_inner(")
        end = self.youtube.index("fn choose_stream_save_options(", start)
        block = self.youtube[start:end]
        self.assertIn('"stream_audio.format_label"', block)
        self.assertIn('"stream_audio.quality_label"', block)
        self.assertIn("STREAM_SAVE_OPTIONS_ID_FORMAT", block)
        self.assertIn("STREAM_SAVE_OPTIONS_ID_QUALITY", block)
        self.assertGreaterEqual(block.count("WS_TABSTOP"), 4)
        self.assertIn("stream_save_format_items(init.language)", block)
        self.assertIn("set_combo_selection_for_quality(", block)

    def test_save_media_context_action_is_single_action_not_format_submenu(self):
        start = self.youtube.index("fn youtube_video_context_actions(")
        end = self.youtube.index("let transcribe_entries", start)
        block = self.youtube[start:end]
        self.assertNotIn("let mut save_children", block)
        self.assertNotIn("for format in youtube_save_media_formats()", block)
        self.assertIn("download_active_streaming_audio_media(parent, &active_url, language)", block)
        self.assertIn("children: Vec::new()", block)

    def test_all_save_surfaces_share_the_same_format_source(self):
        formats_start = self.youtube.index("fn youtube_save_media_formats()")
        formats_end = self.youtube.index("fn youtube_context_quality_for_format(", formats_start)
        formats_block = self.youtube[formats_start:formats_end]
        for format_name in ["Auto", "Webm", "Mp4", "Mp3", "M4a", "Opus", "Ogg", "Wav", "Flac"]:
            self.assertIn(f"StreamOutputFormat::{format_name}", formats_block)
        self.assertIn("fn stream_save_format_items(language: Language)", self.youtube)
        self.assertIn("youtube_save_media_formats()", self.youtube[self.youtube.index("fn stream_save_format_items(language: Language)"):])
        self.assertIn("let format_options = stream_save_format_items(language);", self.youtube)

    def test_playlist_selector_has_tab_reachable_format_and_quality_combos(self):
        start = self.youtube.index("fn playlist_download_selection_wndproc_inner(")
        end = self.youtube.index("WM_COMMAND =>", start)
        block = self.youtube[start:end]
        self.assertIn("PLAYLIST_DOWNLOAD_SELECT_ID_FORMAT", block)
        self.assertIn("PLAYLIST_DOWNLOAD_SELECT_ID_QUALITY", block)
        self.assertIn('"stream_audio.format_label"', block)
        self.assertIn('"stream_audio.quality_label"', block)
        self.assertLess(block.index("let list = CreateWindowExW("), block.index("let (format_label, format_combo)"))
        self.assertLess(block.index("let (format_label, format_combo)"), block.index("let (quality_label, quality_combo)"))
        self.assertLess(block.index("let (quality_label, quality_combo)"), block.index("let select_all = CreateWindowExW("))

    def test_playlist_returns_and_applies_both_format_and_quality(self):
        chooser_start = self.youtube.index("fn choose_playlist_download_entries(")
        chooser_end = self.youtube.index("fn selected_stream_save_options(", chooser_start)
        chooser = self.youtube[chooser_start:chooser_end]
        self.assertIn("Option<(Vec<usize>, PlaylistDownloadOptions)>", chooser)
        self.assertIn("Some((selected, PlaylistDownloadOptions { format, quality }))", chooser)

        start = self.youtube.index("fn choose_and_download_youtube_playlist_items(")
        end = self.youtube.index("pub(crate) fn download_active_streaming_audio_media(", start)
        block = self.youtube[start:end]
        self.assertIn("let Some((indices, selected_options))", block)
        self.assertIn("persist_stream_save_format(parent, selected_options.format)", block)
        self.assertIn("selected_options,", block)

    def test_transcription_download_path_does_not_prompt_for_save_format(self):
        start = self.youtube.index("pub(crate) fn download_active_streaming_audio_media_for_transcription(")
        end = self.youtube.index("pub fn play_streaming_audio_from_url(", start)
        block = self.youtube[start:end]
        self.assertNotIn("choose_stream_save_options(", block)

    def test_active_save_prompts_before_file_dialog_and_persists_format(self):
        start = self.youtube.index("pub(crate) fn download_active_streaming_audio_media(")
        end = self.youtube.index("pub(crate) fn download_active_streaming_audio_media_for_transcription(", start)
        block = self.youtube[start:end]
        self.assertIn("choose_stream_save_options(", block)
        self.assertLess(block.index("choose_stream_save_options("), block.index("save_podcast_episode_dialog("))
        self.assertIn("persist_stream_save_format(parent, selected_options.format)", block)
        self.assertIn("context.dialog_data.format = selected_options.format", block)
        self.assertIn("context.dialog_data.quality = selected_options.quality", block)


if __name__ == "__main__":
    unittest.main()
