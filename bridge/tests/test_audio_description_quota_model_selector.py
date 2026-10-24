import json
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
WINDOW = (
    ROOT / "src" / "app_windows" / "audio_description_window.rs"
).read_text(encoding="utf-8")
PROMPT = (
    ROOT / "src" / "app_windows" / "prompt_window.rs"
).read_text(encoding="utf-8")


class AudioDescriptionQuotaModelSelectorTests(unittest.TestCase):
    def test_quota_switch_uses_combo_selector_not_free_text_prompt(self):
        quota_start = WINDOW.index("WM_AD_QUOTA =>")
        quota_end = WINDOW.index("WM_AD_DONE =>", quota_start)
        quota_block = WINDOW[quota_start:quota_end]

        self.assertIn("prompt_user_choice(", quota_block)
        self.assertNotIn("prompt_user(\n", quota_block)
        self.assertIn("available_models.retain", quota_block)
        self.assertIn("canonical_gemini_model_id", quota_block)
        self.assertIn("exhausted_gemini_models", quota_block)
        self.assertIn("AudioDescriptionQuotaDecision::SwitchModel(model)", quota_block)
        self.assertIn("AudioDescriptionQuotaDecision::Stop", quota_block)


    def test_quota_selector_tracks_all_exhausted_models_for_current_job(self):
        self.assertIn("exhausted_gemini_models: Vec<String>", WINDOW)
        self.assertIn("state.exhausted_gemini_models.clear();", WINDOW)
        quota_start = WINDOW.index("WM_AD_QUOTA =>")
        quota_end = WINDOW.index("WM_AD_DONE =>", quota_start)
        quota_block = WINDOW[quota_start:quota_end]
        self.assertIn("state.exhausted_gemini_models.push(exhausted_model)", quota_block)
        self.assertIn("!state.exhausted_gemini_models.contains(&candidate)", quota_block)
        self.assertIn("strip_prefix(\"models/\")", WINDOW)


    def test_quota_selector_always_refreshes_full_model_list_before_choice(self):
        quota_start = WINDOW.index("WM_AD_QUOTA =>")
        quota_end = WINDOW.index("WM_AD_DONE =>", quota_start)
        quota_block = WINDOW[quota_start:quota_end]
        self.assertIn("fetch_gemini_models_for_key", quota_block)
        self.assertNotIn("if !has_alternative", quota_block)
        self.assertIn("using cached list", quota_block)
        self.assertIn("refreshed {} compatible Gemini model(s)", quota_block)


    def test_quota_refresh_does_not_mutate_main_model_combo(self):
        quota_start = WINDOW.index("WM_AD_QUOTA =>")
        quota_end = WINDOW.index("WM_AD_DONE =>", quota_start)
        quota_block = WINDOW[quota_start:quota_end]
        self.assertIn("available_models = models;", quota_block)
        self.assertIn("without mutating the main model combo", quota_block)
        self.assertNotIn("refill_gemini_model_combo(\n                                    state.gemini_model_combo,\n                                    models,", quota_block)

    def test_main_model_combo_refreshes_full_list_on_window_open(self):
        create_start = WINDOW.index("WM_CREATE =>")
        create_end = WINDOW.index("WM_COMMAND =>", create_start)
        create_block = WINDOW[create_start:create_end]
        self.assertIn("refreshing full Gemini model list on window open", create_block)
        self.assertIn("refresh_gemini_models(hwnd, &*state_pointer, false);", create_block)

    def test_quota_dialog_suspends_watchdog_while_modal(self):
        quota_start = WINDOW.index("WM_AD_QUOTA =>")
        quota_end = WINDOW.index("WM_AD_DONE =>", quota_start)
        quota_block = WINDOW[quota_start:quota_end]
        enter = quota_block.index("crate::watchdog::enter_modal_dialog();")
        message_box = quota_block.index("MessageBoxW(")
        exit_ = quota_block.index("crate::watchdog::exit_modal_dialog();")
        self.assertLess(enter, message_box)
        self.assertGreater(exit_, message_box)

    def test_default_model_is_preferred_when_it_is_still_available(self):
        helper_start = WINDOW.index("fn suggested_alternative_model")
        helper_end = WINDOW.index("fn set_text", helper_start)
        helper = WINDOW[helper_start:helper_end]
        self.assertIn("DEFAULT_AUDIO_DESCRIPTION_GEMINI_MODEL", helper)
        self.assertIn("canonical_gemini_model_id", helper)

    def test_choice_prompt_is_a_keyboard_accessible_dropdown(self):
        self.assertIn("pub fn prompt_user_choice(", PROMPT)
        self.assertIn("SonarpadChoicePrompt", PROMPT)
        self.assertIn("CBS_DROPDOWNLIST", PROMPT)
        self.assertIn("crate::set_focus_safe(combo);", PROMPT)
        self.assertIn("VK_TAB", PROMPT)
        self.assertIn("VK_RETURN", PROMPT)
        self.assertIn("VK_ESCAPE", PROMPT)

    def test_quota_prompt_is_selection_wording_in_all_locales(self):
        for path in sorted((ROOT / "i18n").glob("*.json")):
            values = json.loads(path.read_text(encoding="utf-8"))
            prompt = values["audio_description.quota.model_prompt"]
            self.assertTrue(prompt.strip(), path.name)
            self.assertIn("audio_description.quota.no_alternative_models", values)

        italian = json.loads((ROOT / "i18n" / "it.json").read_text(encoding="utf-8"))
        self.assertEqual(
            italian["audio_description.quota.model_prompt"],
            "Seleziona il modello Gemini da provare:",
        )
        self.assertNotIn("identificatore", italian["audio_description.quota.model_prompt"].lower())


if __name__ == "__main__":
    unittest.main()
