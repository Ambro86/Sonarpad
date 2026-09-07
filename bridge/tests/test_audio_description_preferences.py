import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
SETTINGS = (ROOT / "src" / "settings.rs").read_text(encoding="utf-8")
WINDOW = (
    ROOT / "src" / "app_windows" / "audio_description_window.rs"
).read_text(encoding="utf-8")


class AudioDescriptionPreferenceTests(unittest.TestCase):
    def test_settings_store_module_specific_preferences(self):
        for field in (
            "audio_description_language",
            "audio_description_tts_engine",
            "audio_description_tts_voice",
            "audio_description_verbosity",
            "audio_description_extended_pauses",
            "audio_description_recognize_characters",
            "audio_description_recognize_screen_text",
            "audio_description_keep_character_catalog",
            "audio_description_character_catalog",
            "audio_description_save_project",
            "audio_description_delete_video_after",
        ):
            self.assertIn(f"pub {field}", SETTINGS)

    def test_window_restores_saved_language_engine_voice_and_checks(self):
        self.assertIn("audio_description_language", WINDOW)
        self.assertIn("audio_description_tts_engine", WINDOW)
        self.assertIn("audio_description_tts_voice.clone()", WINDOW)
        self.assertIn("audio_description_extended_pauses", WINDOW)
        self.assertIn("audio_description_recognize_characters", WINDOW)
        self.assertIn("audio_description_recognize_screen_text", WINDOW)
        self.assertIn("audio_description_keep_character_catalog", WINDOW)
        self.assertIn("audio_description_character_catalog", WINDOW)
        self.assertIn("audio_description_save_project", WINDOW)
        self.assertIn("audio_description_delete_video_after", WINDOW)
        self.assertIn("preferred_voice: tts_voice", WINDOW)
        self.assertIn("load_voices(hwnd, tts_engine);", WINDOW)

    def test_preferences_are_saved_on_each_user_selection(self):
        self.assertIn("fn persist_audio_description_preferences", WINDOW)
        self.assertIn("ID_LANGUAGE if notification == CBN_SELCHANGE", WINDOW)
        self.assertIn("ID_VERBOSITY if notification == CBN_SELCHANGE", WINDOW)
        self.assertIn("ID_ENGINE if notification == CBN_SELCHANGE", WINDOW)
        self.assertIn("ID_VOICE if notification == CBN_SELCHANGE", WINDOW)
        self.assertIn("ID_RECOGNIZE_CHARACTERS if !state.running", WINDOW)
        self.assertIn("ID_KEEP_CHARACTER_CATALOG if !state.running", WINDOW)
        self.assertIn(
            "ID_CHARACTER_CATALOG if notification == CBN_SELCHANGE && !state.running",
            WINDOW,
        )
        self.assertIn("ID_EXTENDED | ID_SAVE_PROJECT if !state.running", WINDOW)
        self.assertIn("ID_DELETE_VIDEO_AFTER if !state.running", WINDOW)
        self.assertIn("ID_RECOGNIZE_SCREEN_TEXT if !state.running", WINDOW)
        self.assertIn("save_settings(app.settings.clone());", WINDOW)

    def test_delete_video_option_is_hidden_and_blocked_when_project_is_saved(self):
        self.assertIn("fn update_delete_video_visibility", WINDOW)
        self.assertIn("if save_project { SW_HIDE } else { SW_SHOW }", WINDOW)
        self.assertIn("delete_requested && !save_project", WINDOW)
        self.assertIn("move_input_video_to_recycle_bin", WINDOW)

    def test_saved_voice_is_preferred_then_falls_back_by_language(self):
        self.assertIn("eq_ignore_ascii_case(&state.preferred_voice)", WINDOW)
        self.assertIn("starts_with(&code.to_ascii_lowercase())", WINDOW)
        self.assertIn("persist_audio_description_preferences(state);", WINDOW)


if __name__ == "__main__":
    unittest.main()
