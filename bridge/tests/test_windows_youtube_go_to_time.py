"""Static regression guards for the YouTube/MPV Go to Time integration.

These are source-level tests; running the interactive Win32 player is only
possible on Windows with its MPV bundle installed.
"""
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
MAIN = (ROOT / "src" / "main.rs").read_text(encoding="utf-8")
MENU = (ROOT / "src" / "menu.rs").read_text(encoding="utf-8")
DIALOG = (ROOT / "src" / "app_windows" / "go_to_time_window.rs").read_text(encoding="utf-8")


class YouTubeGoToTimeRegressionTests(unittest.TestCase):
    def test_ctrl_t_is_processed_by_mpv_dispatcher(self):
        start = MAIN.index("pub(crate) fn handle_player_command(")
        end = MAIN.index("let disable_seek_for_live_raiplay", start)
        mpv_dispatcher = MAIN[start:end]
        self.assertIn("PlayerCommand::GoToTime => {", mpv_dispatcher)
        self.assertIn("app_windows::go_to_time_window::open(hwnd)", mpv_dispatcher)
        self.assertIn("is_live_tv_playback_active(hwnd)", mpv_dispatcher)

    def test_dialog_accepts_mpv_only_for_seekable_recordings(self):
        self.assertIn("!has_player && !crate::can_go_to_time_in_managed_mpv(parent)", DIALOG)
        self.assertIn("should_route_player_command_to_mpv(hwnd) && !is_live_tv_playback_active(hwnd)", MAIN)

    def test_dialog_preserves_bass_path_and_uses_mpv_seek_when_active(self):
        self.assertIn("audiobook_duration_secs(&path)", DIALOG)
        self.assertIn("seek_audiobook_to(parent, seek_target)", DIALOG)
        self.assertIn("crate::managed_mpv_duration_for_go_to_time(parent)", DIALOG)
        self.assertIn("crate::seek_managed_mpv_to_time(parent, seek_target)", DIALOG)
        self.assertIn('r#"{{"command":["seek",{},"absolute"]}}"#', MAIN)
        self.assertIn('stop_mpv_subtitle_speech(hwnd, "go_to_time")', MAIN)

    def test_menu_does_not_disable_mpv_youtube_and_keeps_bass_protected(self):
        self.assertIn('let seekable_on_demand = crate::can_go_to_time_in_managed_mpv(hwnd)', MENU)
        self.assertIn('let seek_flags = if live_tv_playback', MENU)
        self.assertIn('direct_stream_playback && !seekable_on_demand', MENU)
        start = MENU.index('IDM_PLAYBACK_GO_TO_TIME,')
        self.assertIn('seek_flags,', MENU[start-100:start])
        self.assertIn('if !live_tv_playback && !raiplay_live_stream_playback {', MENU[start-230:start])

    def test_changelog_dates(self):
        for path in ROOT.glob('CHANGELOG*.md'):
            contents = path.read_text(encoding='utf-8')
            self.assertEqual(contents.splitlines()[2].split(' – ')[1], '2026-10-10', path.name)
        hindi = (ROOT / 'CHANGELOG-HI.txt').read_text(encoding='utf-8')
        self.assertIn('Version 0.9.13 – 2026-10-10', hindi)


if __name__ == '__main__':
    unittest.main()
