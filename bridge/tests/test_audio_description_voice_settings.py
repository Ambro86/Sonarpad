import json
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
WINDOW = (ROOT / "src" / "app_windows" / "audio_description_window.rs").read_text(
    encoding="utf-8"
)
DIALOG = (
    ROOT / "src" / "app_windows" / "audio_description_voice_window.rs"
).read_text(encoding="utf-8")
SETTINGS = (ROOT / "src" / "settings.rs").read_text(encoding="utf-8")


class AudioDescriptionVoiceSettingsTests(unittest.TestCase):
    def test_adjust_voice_button_precedes_project_and_create_buttons_in_tab_creation_order(self):
        adjust = WINDOW.index("let voice_settings_button = CreateWindowExW")
        modify = WINDOW.index("let modify_project_button = CreateWindowExW")
        resume = WINDOW.index("let continue_interrupted_button = CreateWindowExW")
        start = WINDOW.index("let start_button = CreateWindowExW")
        self.assertLess(adjust, modify)
        self.assertLess(modify, resume)
        self.assertLess(resume, start)
        self.assertIn("ID_VOICE_SETTINGS", WINDOW)

    def test_creation_window_hides_duplicate_engine_and_voice_controls(self):
        creation = WINDOW[
            WINDOW.index("let engine_label = create_label") : WINDOW.index("let progress = CreateWindowExW")
        ]
        self.assertIn("ShowWindow(engine_label, SW_HIDE);", creation)
        self.assertIn("ShowWindow(voice_label, SW_HIDE);", creation)
        self.assertEqual(creation.count("WS_CHILD | WINDOW_STYLE(CBS_DROPDOWNLIST as u32)"), 2)
        self.assertNotIn("WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(CBS_DROPDOWNLIST as u32)", creation)

    def test_dialog_has_requested_controls_and_voice_preview(self):
        for control in ("ID_ENGINE", "ID_VOICE", "ID_RATE", "ID_VOLUME", "ID_TEST"):
            self.assertIn(control, DIALOG)
        self.assertIn('"audio_description.voice_settings.test"', DIALOG)
        self.assertIn("preview_voice(hwnd);", DIALOG)

    def test_ok_applies_rate_and_volume_to_audio_description_job(self):
        self.assertIn("state.tts_rate = selected.rate;", WINDOW)
        self.assertIn("state.tts_volume = selected.volume;", WINDOW)
        self.assertIn("tts_rate: state.tts_rate", WINDOW)
        self.assertIn("tts_volume: state.tts_volume", WINDOW)

    def test_voice_adjustments_are_persisted_as_audio_description_settings(self):
        self.assertIn("pub audio_description_tts_rate: Option<i32>", SETTINGS)
        self.assertIn("pub audio_description_tts_volume: Option<i32>", SETTINGS)
        self.assertIn("app.settings.audio_description_tts_rate = Some(state.tts_rate);", WINDOW)
        self.assertIn("app.settings.audio_description_tts_volume = Some(state.tts_volume);", WINDOW)

    def test_legacy_users_fall_back_to_existing_global_tts_values(self):
        self.assertIn(".audio_description_tts_rate", WINDOW)
        self.assertIn(".unwrap_or(state.settings.tts_rate)", WINDOW)
        self.assertIn(".audio_description_tts_volume", WINDOW)
        self.assertIn(".unwrap_or(state.settings.tts_volume)", WINDOW)
        self.assertIn("tts_pitch: settings.tts_pitch", WINDOW)

    def test_focus_returns_to_adjust_voice_button(self):
        self.assertIn("SetFocus(state.voice_settings_button);", WINDOW)


    def test_audio_description_window_blocks_delayed_editor_focus_when_foreground(self):
        self.assertIn("if foreground == window", WINDOW)
        self.assertIn("== window.0", WINDOW)
        self.assertIn("blocks_parent_focus", WINDOW)

    def test_creation_errors_restore_focus_inside_audio_description_window(self):
        self.assertIn("fn show_audio_description_error_and_focus", WINDOW)
        self.assertIn("restore_audio_description_control_focus(hwnd, control);", WINDOW)
        self.assertIn("state.sonarpad_code_edit", WINDOW)
        self.assertIn("state.gemini_api_key_edit", WINDOW)
        self.assertIn("state.voice_settings_button", WINDOW)

    def test_all_locales_define_voice_settings_strings(self):
        for path in (ROOT / "i18n").glob("*.json"):
            if path.name.endswith(".bac.json"):
                continue
            data = json.loads(path.read_text(encoding="utf-8"))
            for key in (
                "audio_description.voice_settings",
                "audio_description.voice_settings.title",
                "audio_description.voice_settings.engine",
                "audio_description.voice_settings.voice",
                "audio_description.voice_settings.test",
            ):
                self.assertIn(key, data, f"{path.name}: missing {key}")


if __name__ == "__main__":
    unittest.main()
