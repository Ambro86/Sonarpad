import json
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
        for label in (
            "lang_it",
            "lang_en",
            "lang_es",
            "lang_pt",
            "lang_pt_br",
            "lang_sv",
            "lang_vi",
            "lang_cs",
            "lang_pl",
            "lang_fr",
            "lang_sr",
            "lang_uk",
            "lang_lt",
            "lang_ru",
            "lang_zh",
            "lang_hi",
            "lang_de",
        ):
            self.assertIn(f"labels.{label}.as_str()", source)

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
        self.assertEqual(18, len(files))
        for path in files:
            data = json.loads(path.read_text(encoding="utf-8"))
            self.assertIn("options.label.whisper_audio_language", data, path.name)
            self.assertNotIn("options.label.whisper_keep_original_language", data, path.name)


if __name__ == "__main__":
    unittest.main()
