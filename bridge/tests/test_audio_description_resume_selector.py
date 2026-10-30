from __future__ import annotations

import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
SELECTOR = ROOT / "src" / "app_windows" / "audio_description_resume_window.rs"
WINDOW = ROOT / "src" / "app_windows" / "audio_description_window.rs"
SETTINGS = ROOT / "src" / "settings.rs"


class AudioDescriptionResumeSelectorTests(unittest.TestCase):
    def test_continue_uses_project_selector_instead_of_direct_file_dialog(self):
        window = WINDOW.read_text(encoding="utf-8")
        self.assertIn(
            "audio_description_resume_window::choose_resume_checkpoint",
            window,
        )
        self.assertNotIn(
            "fn choose_resume_checkpoint(parent: HWND, language: Language)",
            window,
        )

    def test_selector_lists_only_valid_partial_checkpoints_and_has_browse_fallback(self):
        selector = SELECTOR.read_text(encoding="utf-8")
        self.assertIn('const CHECKPOINT_SUFFIX: &str = ".sonarpad-ad.partial.json";', selector)
        self.assertIn("load_audio_description_resume_settings(path).ok()?", selector)
        self.assertIn("discover_resume_candidates(app_parent)", selector)
        self.assertIn('"audio_description.resume.browse_other"', selector)
        filter_start = selector.index("fn browse_checkpoint")
        filter_end = selector.index("pub(crate) fn choose_resume_checkpoint", filter_start)
        browse = selector[filter_start:filter_end]
        self.assertIn("*.sonarpad-ad.partial.json", browse)
        self.assertNotIn("*.*", browse)

    def test_recent_project_folders_are_persisted_and_used_for_discovery(self):
        selector = SELECTOR.read_text(encoding="utf-8")
        settings = SETTINGS.read_text(encoding="utf-8")
        window = WINDOW.read_text(encoding="utf-8")
        self.assertIn("audio_description_recent_project_folders", settings)
        self.assertIn("audio_description_recent_project_folders", selector)
        self.assertIn("truncate(MAX_RECENT_FOLDERS)", selector)
        self.assertIn("remember_project_folder", selector)
        self.assertIn(
            "audio_description_resume_window::remember_project_folder",
            window,
        )

    def test_empty_selector_disables_continue_and_focuses_browse(self):
        selector = SELECTOR.read_text(encoding="utf-8")
        self.assertIn("EnableWindow(continue_button, has_candidates);", selector)
        self.assertIn("SetFocus(if has_candidates { combo } else { browse_button });", selector)

    def test_cancelling_resume_selector_closes_creation_window(self):
        window = WINDOW.read_text(encoding="utf-8")
        start = window.index("fn continue_interrupted_from_window")
        end = window.index("fn open_window", start)
        resume = window[start:end]
        self.assertIn("resume selector cancelled; closing creation window", resume)
        self.assertIn("post_message_w_safe(hwnd, WM_CLOSE", resume)

    def test_selector_combines_project_and_model_before_single_continue(self):
        selector = SELECTOR.read_text(encoding="utf-8")
        window = WINDOW.read_text(encoding="utf-8")
        self.assertIn("pub(crate) struct ResumeSelection", selector)
        self.assertIn("pub gemini_model: String", selector)
        self.assertIn('"audio_description.resume.model"', selector)
        self.assertIn("model_combo", selector)
        self.assertIn("CBN_SELCHANGE", selector)
        start = window.index("fn continue_interrupted_from_window")
        end = window.index("fn open_window", start)
        resume = window[start:end]
        self.assertIn("combo_items(state.gemini_model_combo)", resume)
        self.assertIn("get_text(state.gemini_model_combo)", resume)
        self.assertIn("selection.gemini_model", resume)
        self.assertIn("start_job(hwnd, state);", resume)
        self.assertNotIn("WM_AD_SET_RESUME", resume)

    def test_running_progress_window_blocks_only_automatic_editor_focus(self):
        window = WINDOW.read_text(encoding="utf-8")
        start = window.index("fn is_running_audio_description_window")
        end = window.index("pub(crate) fn blocks_main_window_close", start)
        focus_guard = window[start:end]
        self.assertIn("GWLP_USERDATA", focus_guard)
        self.assertIn("(*pointer).running", focus_guard)
        self.assertIn("if is_running_audio_description_window(window)", focus_guard)
        self.assertIn("GetForegroundWindow() != parent", focus_guard)
        self.assertNotIn("if is_running_audio_description_window(window) {\n        return true;", focus_guard)

    def test_direct_resume_restores_progress_window_after_selector_closes(self):
        window = WINDOW.read_text(encoding="utf-8")
        start = window.index("fn continue_interrupted_from_window")
        end = window.index("fn open_window", start)
        resume = window[start:end]
        self.assertIn("start_job(hwnd, state);", resume)
        self.assertIn("WM_AD_RESTORE_RUNNING_FOCUS", resume)
        self.assertIn("if state.running", resume)

        handler = window.index("WM_AD_RESTORE_RUNNING_FOCUS =>")
        focus = window[handler:handler + 900]
        self.assertIn("ShowWindow(hwnd, SW_SHOW);", focus)
        self.assertIn("SetForegroundWindow(hwnd);", focus)
        self.assertIn("SetFocus(state.progress);", focus)

    def test_browse_keeps_selector_open_so_model_can_be_chosen(self):
        selector = SELECTOR.read_text(encoding="utf-8")
        command_start = selector.index("ID_BROWSE =>")
        command_end = selector.index("ID_CANCEL =>", command_start)
        browse = selector[command_start:command_end]
        self.assertIn("append_candidate(state, candidate)", browse)
        self.assertIn("SetFocus(state.model_combo)", browse)
        self.assertNotIn("*result = Some", browse)
        self.assertNotIn("DestroyWindow(hwnd)", browse)

    def test_resume_selector_labels_are_localized_in_all_windows_locales(self):
        locales = [
            "it", "en", "de", "es", "fr", "pt", "pt-BR", "cs", "pl",
            "ru", "vi", "sv", "sr", "uk", "lt", "zh", "hi",
        ]
        keys = [
            "audio_description.resume.choose_label",
            "audio_description.resume.choose_hint",
            "audio_description.resume.none_found",
            "audio_description.resume.browse_other",
        ]
        for locale in locales:
            text = (ROOT / "i18n" / f"{locale}.json").read_text(encoding="utf-8-sig")
            for key in keys:
                self.assertIn(f'"{key}"', text, f"{locale}: {key}")


if __name__ == "__main__":
    unittest.main()
