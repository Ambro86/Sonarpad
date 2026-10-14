"""Guard the shared preview/export path and accessible volume notifications."""
import collections
import re
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
CORE = (ROOT / 'src/audio_description.rs').read_text('utf-8')
WINDOWS = [ROOT / 'src/app_windows' / name for name in (
    'audio_description_window.rs', 'audio_description_project_window.rs'
)]


class MixPreviewTests(unittest.TestCase):
    def test_volume_changes_are_selection_notifications_not_focus_notifications(self):
        for path in WINDOWS:
            with self.subTest(window=path.name):
                source = path.read_text('utf-8')
                self.assertRegex(source, r'ID_SOURCE_VOLUME\s+if notification == CBN_SELCHANGE && !state.running')
                self.assertIn('mix_preview::create_controls(', source)
                self.assertIn('mix_preview::TIMER_ID', source)
                self.assertIn('MixPreview::default()', source)

    def test_each_audio_description_window_has_distinct_control_ids(self):
        for path in WINDOWS + [ROOT / 'src/app_windows/audio_description_voice_window.rs']:
            with self.subTest(window=path.name):
                controls = collections.defaultdict(list)
                for name, value in re.findall(r'const (ID_\w+): (?:usize|i32) = (\d+);', path.read_text('utf-8')):
                    controls[int(value)].append(name)
                self.assertTrue(controls)
                self.assertEqual({k: v for k, v in controls.items() if len(v) > 1}, {})

    def test_preview_uses_export_synthesis_and_mixer(self):
        preview = CORE.split('fn render_audio_description_mix_preview(', 1)[1].split('fn write_audio_description_preview_wav(', 1)[0]
        self.assertIn('synthesize_description(', preview)
        self.assertIn('export_audio_description_mp3(', preview)
        self.assertIn('source_volume: normalize_source_volume(job.source_volume)', preview)
        self.assertIn('AUDIO_DESCRIPTION_DUCKING_DB', preview)
        self.assertIn('AUDIO_DESCRIPTION_FADE_MS', preview)
        self.assertIn('start_sec: 2.0', preview)

    def test_all_export_paths_preserve_the_selected_source_volume(self):
        options = re.findall(r'AudioDescriptionExportOptions\s*\{([^}]+)\}', CORE)
        self.assertGreaterEqual(len(options), 6)
        for block in options:
            self.assertIn('source_volume:', block)
        self.assertIn('source_volume: checkpoint.source_volume', CORE)
        self.assertIn('source_volume: project.source_volume', CORE)
        self.assertIn('source_volume: job.source_volume', CORE)

    def test_preview_worker_does_not_move_keyboard_focus(self):
        source = (ROOT / 'src/app_windows/audio_description_mix_preview.rs').read_text('utf-8')
        for forbidden in ('SetFocus(', 'SetForegroundWindow(', 'MessageBoxW('):
            self.assertNotIn(forbidden, source)

    def test_creation_bitrate_reaches_preview_export_and_checkpoint(self):
        source = WINDOWS[0].read_text('utf-8')
        self.assertIn('HMENU(ID_BITRATE as isize)', source)
        self.assertIn('output_bitrate_kbps: selected_bitrate(state.bitrate_combo)', source)
        self.assertIn('job.output_bitrate_kbps = selected_bitrate(state.bitrate_combo)', source)
        self.assertIn('select_bitrate(state.bitrate_combo, resume.output_bitrate_kbps)', source)
        self.assertIn('app.settings.audio_description_bitrate_kbps = selected_bitrate(state.bitrate_combo)', source)
        self.assertEqual(CORE.count('bitrate_kbps: normalize_audio_description_bitrate(job.output_bitrate_kbps)'), 5)


if __name__ == '__main__':
    unittest.main()
