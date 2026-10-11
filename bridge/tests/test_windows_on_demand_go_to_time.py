"""Source-level regression checks for all on-demand MPV/BASS time seeking.

An actual Windows MPV/RaiPlay URL is required for end-to-end playback tests.
"""
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
MAIN = (ROOT / 'src/main.rs').read_text(encoding='utf-8')
MENU = (ROOT / 'src/menu.rs').read_text(encoding='utf-8')
DIALOG = (ROOT / 'src/app_windows/go_to_time_window.rs').read_text(encoding='utf-8')
RAI_AD = (ROOT / 'src/app_windows/rai_audiodescrizioni_window.rs').read_text(encoding='utf-8')
SONAR_AD = (ROOT / 'src/app_windows/sonarpad_audiodescrizioni_window.rs').read_text(encoding='utf-8')

class AllOnDemandTimeSeekingTests(unittest.TestCase):
    def test_both_audio_description_catalogues_use_mpv(self):
        self.assertIn('crate::launch_stream_url_in_mpv(', RAI_AD)
        self.assertIn('crate::launch_stream_url_in_mpv(', SONAR_AD)

    def test_source_url_mismatch_uses_mpv_document_identity(self):
        start = MAIN.index('fn should_route_player_command_to_mpv(')
        end = MAIN.index('#[derive(Clone, Copy)]', start)
        body = MAIN[start:end]
        self.assertIn('doc.prefer_mpv_playback', body)
        self.assertIn('state.active_audiobook.is_some()', body)
        self.assertIn('matches!(doc.format, FileFormat::Audiobook)', body)
        self.assertIn('state.active_mpv_session.is_none()', body)
        self.assertIn('path.to_string_lossy() == active_url.as_str()', body)

    def test_raiplay_live_flag_is_propagated_from_resolver(self):
        start=MAIN.index('pub(crate) fn launch_raiplay_in_mpv_with_resume(')
        end=MAIN.index('pub(crate) fn active_mpv_process_id',start)
        segment=MAIN[start:end]
        self.assertIn('source_is_live', segment)
        self.assertIn('is_live,', segment)
        self.assertIn('state.active_mpv_is_live_tv = source_is_live;', segment)

    def test_live_tv_and_live_raiplay_stay_disabled(self):
        self.assertIn('should_route_player_command_to_mpv(hwnd) && !is_live_tv_playback_active(hwnd)',MAIN)
        self.assertIn('let seek_flags = if live_tv_playback',MENU)
        self.assertIn('|| raiplay_live_stream_playback', MENU)
        self.assertIn('if !live_tv_playback && !raiplay_live_stream_playback {',MENU)

    def test_remote_bass_only_for_known_vod_origins(self):
        start=MAIN.index('pub(crate) fn can_go_to_time_in_bass_on_demand(')
        end=MAIN.index('/// A duration is optional',start)
        segment=MAIN[start:end]
        self.assertIn('state.active_audiobook.as_ref()',segment)
        self.assertIn('RaiAudioOrigin::RaiPlay | RaiAudioOrigin::La7Play', segment)
        self.assertIn('state.raiplay_live_audio_variants.is_empty()',segment)
        self.assertIn('is_direct_stream_url_path(&player.path)',segment)
        self.assertIn('can_go_to_time_in_bass_on_demand(hwnd)', MAIN)
        self.assertIn('crate::can_go_to_time_in_bass_on_demand(hwnd)', MENU)

    def test_time_dialog_preserves_existing_absolute_seek(self):
        self.assertIn('crate::seek_managed_mpv_to_time(parent, seek_target)', DIALOG)
        self.assertIn('seek_audiobook_to(parent, seek_target)',DIALOG)
        self.assertIn('audiobook_duration_secs(&path)',DIALOG)

if __name__=='__main__':
    unittest.main()
