"""Regression checks for local MP4/AVI/MKV Go to Time in the Win32 MPV player.

Static source tests: functional MPV/Win32 testing requires a Windows build.
"""
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
MAIN = (ROOT / 'src/main.rs').read_text(encoding='utf-8')
MENU = (ROOT / 'src/menu.rs').read_text(encoding='utf-8')
DIALOG = (ROOT / 'src/app_windows/go_to_time_window.rs').read_text(encoding='utf-8')


class LocalVideoGoToTimeTests(unittest.TestCase):
    def test_local_start_keeps_actual_file_and_player_tab(self):
        begin = MAIN.index('let mpv_generation = next_mpv_playback_generation(hwnd, "start local");')
        local = MAIN[begin:MAIN.index('fn should_route_player_command_to_mpv', begin)]
        self.assertIn('ensure_audio_document_tab(hwnd, path)', local)
        self.assertIn('mark_current_document_prefer_mpv_playback(hwnd, true)', local)
        self.assertIn('state.active_podcast_episode_url = Some(path_text.clone())', local)
        self.assertIn('state.active_mpv_is_live_tv = false', local)

    def test_local_player_detection_handles_windows_path_spellings(self):
        begin = MAIN.index('fn local_mpv_source_matches_document(')
        end = MAIN.index('fn should_route_player_command_to_mpv', begin)
        helper = MAIN[begin:end]
        self.assertIn('eq_ignore_ascii_case', helper)
        self.assertIn("replace('/',", helper)
        self.assertIn('is_direct_stream_url_path(path)', helper)
        self.assertIn('source.starts_with("http', helper)

    def test_local_routing_refuses_wrong_tab_and_bass_session(self):
        begin = MAIN.index('fn should_route_player_command_to_mpv(')
        end = MAIN.index('#[derive(Clone, Copy)]', begin)
        body = MAIN[begin:end]
        self.assertIn('state.active_mpv_session.is_none()', body)
        self.assertIn('state.active_audiobook.is_some()', body)
        self.assertIn('matches!(doc.format, FileFormat::Audiobook)', body)
        self.assertIn('local_mpv_source_matches_document(path, source)', body)
        self.assertIn('active_podcast_episode_media_url.as_deref()', body)

    def test_time_dialog_and_live_tv_guards_remain(self):
        self.assertIn('should_route_player_command_to_mpv(hwnd) && !is_live_tv_playback_active(hwnd)', MAIN)
        self.assertIn('!has_player && !crate::can_go_to_time_in_managed_mpv(parent)', DIALOG)
        self.assertIn('crate::seek_managed_mpv_to_time(parent, seek_target)', DIALOG)
        self.assertIn('seek_audiobook_to(parent, seek_target)', DIALOG)
        self.assertIn('let seek_flags = if live_tv_playback', MENU)
        self.assertIn('|| raiplay_live_stream_playback', MENU)


if __name__ == '__main__':
    unittest.main()
