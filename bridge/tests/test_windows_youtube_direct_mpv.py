import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
MAIN = (ROOT / "src" / "main.rs").read_text(encoding="utf-8")
YOUTUBE = (ROOT / "src" / "app_windows" / "youtube_transcript_window.rs").read_text(encoding="utf-8")


class WindowsYoutubeIsolatedOpeningTests(unittest.TestCase):
    def _playback_block(self):
        marker = "if should_play_streaming_audio_with_mpv()"
        start = YOUTUBE.index(marker)
        return YOUTUBE[start : start + 13_000]

    def test_normal_youtube_playback_has_no_separate_preflight(self):
        block = self._playback_block()
        self.assertNotIn("probe_youtube_stream_playable(&ytdlp_path, &url)", block)

    def test_youtube_reuses_selected_list_title_without_extra_probe(self):
        block = self._playback_block()
        self.assertIn("let stream_title = if is_youtube", block)
        compact = "".join(block.split())
        self.assertIn("selected_title.clone().or_else(||selected_label.clone())", compact)
        self.assertIn("selected_title: Some(selected_title)", YOUTUBE)

    def test_mpv_forces_builtin_ytdl_hook_only_inside_player_command(self):
        self.assertIn('format!("ytdl://{url}")', MAIN)
        self.assertIn('command.arg("--ytdl=yes")', MAIN)
        self.assertIn('ytdl_hook-ytdl_path={}', MAIN)
        self.assertIn("youtube:player_client=android", MAIN)
        self.assertNotIn("youtube:player_client=android_vr", MAIN)
        self.assertIn("Sonarpad continues to store and", MAIN)


    def test_optimized_youtube_starts_unpaused_but_other_streams_keep_old_pause_flow(self):
        start = MAIN.index("fn launch_stream_url_in_mpv_with_options(")
        end = MAIN.index("pub(crate) fn launch_local_tv_recording_in_mpv", start)
        block = MAIN[start:end]
        self.assertIn('.arg(if is_youtube { "--pause=no" } else { "--pause=yes" })', block)
        self.assertIn("if !is_youtube", block)
        self.assertIn('r#"{"command":["set_property","pause",false]}"#', block)

    def test_youtube_save_context_is_explicitly_isolated_from_playback(self):
        block = self._playback_block()
        self.assertIn("let save_selected_audio_format = if is_youtube", block)
        self.assertIn("let save_prefer_video = if is_youtube", block)
        self.assertIn("selected_audio_format: save_selected_audio_format", block)
        self.assertIn("prefer_video: save_prefer_video", block)
        self.assertIn("YouTube save context isolated", block)

    def test_async_ytdl_errors_are_still_classified(self):
        self.assertIn("WM_YOUTUBE_MPV_PLAYBACK_FAILED", MAIN)
        self.assertIn("youtube_mpv_log_has_failure", MAIN)
        self.assertIn("youtube_mpv_playback_error_message", YOUTUBE)

    def test_drm_detection_is_not_generic(self):
        start = YOUTUBE.index("fn is_drm_not_supported_stream_error")
        end = YOUTUBE.index("fn youtube_mpv_error_detail", start)
        block = YOUTUBE[start:end]
        self.assertIn("this video is drm protected", block)
        self.assertIn("widevine", block)
        self.assertNotIn('contains("not be supported")', block)
        self.assertNotIn('contains("protected")', block)

    def test_download_fallback_experiment_is_absent(self):
        self.assertNotIn("is_transient_youtube_download_error", YOUTUBE)
        self.assertNotIn("youtube_download_fallback_format", YOUTUBE)
        self.assertNotIn("fresh-default", YOUTUBE)
        self.assertNotIn("web-safari", YOUTUBE)
        self.assertNotIn("format_override", YOUTUBE)

    def test_original_download_selector_is_preserved(self):
        self.assertIn('cmd.arg("-f").arg("bestaudio/best")', YOUTUBE)
        self.assertIn("selected_audio_format: Option<&str>", YOUTUBE)
        self.assertIn("fn configure_ytdlp_stream_download_command", YOUTUBE)

    def test_audio_description_shortcut_unchanged(self):
        self.assertIn("download_active_youtube_for_audio_description", YOUTUBE)
        self.assertIn("audio_description_window::open_with_input", YOUTUBE)


if __name__ == "__main__":
    unittest.main()
