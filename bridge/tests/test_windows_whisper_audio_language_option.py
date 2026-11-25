import json
import re
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]


class WindowsWhisperAudioLanguageOptionTests(unittest.TestCase):
    def test_settings_store_explicit_audio_language_instead_of_auto_detection_flag(self):
        source = (ROOT / "src" / "settings.rs").read_text(encoding="utf-8")
        self.assertIn("pub whisper_audio_language: String", source)
        self.assertIn("whisper_audio_language: String::new()", source)
        self.assertNotIn("whisper_keep_original_language", source)

    def test_options_use_combobox_with_all_sonarpad_languages(self):
        source = (ROOT / "src" / "app_windows" / "options_window.rs").read_text(
            encoding="utf-8"
        )
        self.assertIn("label_whisper_audio_language", source)
        self.assertIn("combo_whisper_audio_language", source)
        self.assertIn("WC_COMBOBOXW", source)
        self.assertIn("CBS_DROPDOWNLIST", source)
        self.assertIn("settings.whisper_audio_language =", source)
        self.assertNotIn("whisper_keep_original_language", source)
        self.assertIn("for code in TRANSCRIPTION_LANGUAGES", source)
        bridge = (ROOT / "src/tools/faster_whisper_bridge.rs").read_text(encoding="utf-8")
        catalog = bridge.split("const TRANSCRIPTION_LANGUAGES:", 1)[1].split("= &[", 1)[1].split("];", 1)[0]
        codes = re.findall(r'"([^"]+)"', catalog)
        self.assertEqual(len(codes), len(set(codes)))
        self.assertIn("he", codes)
        for path in (ROOT / "i18n").glob("*.json"):
            labels = json.loads(path.read_text(encoding="utf-8"))
            for code in codes:
                self.assertTrue(labels.get(f"voice.lang.{code}"), (path.name, code))

    def test_media_folder_and_dictation_force_selected_language(self):
        source = (ROOT / "src" / "main.rs").read_text(encoding="utf-8")
        self.assertNotIn("whisper_keep_original_language", source)
        self.assertGreaterEqual(
            source.count("whisper_audio_language_from_setting("),
            4,
        )
        self.assertGreaterEqual(source.count("let forced_language = Some("), 2)
        self.assertIn("Some(whisper_audio_language_from_setting(", source)

    def test_every_interface_language_has_audio_language_label(self):
        files = sorted((ROOT / "i18n").glob("*.json"))
        self.assertEqual(19, len(files))
        for path in files:
            data = json.loads(path.read_text(encoding="utf-8"))
            self.assertIn("options.label.whisper_audio_language", data, path.name)
            self.assertNotIn("options.label.whisper_keep_original_language", data, path.name)


if __name__ == "__main__":
    unittest.main()
