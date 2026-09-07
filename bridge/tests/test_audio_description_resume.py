from __future__ import annotations

import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]


class AudioDescriptionResumeTests(unittest.TestCase):
    def test_checkpoint_is_written_only_after_completed_chunk_processing(self):
        source = (
            ROOT
            / "bridge"
            / "audio_description_runtime"
            / "audio_describer"
            / "core"
            / "audio_describer.py"
        ).read_text(encoding="utf-8")
        extend = source.index("all_descriptions.extend(normalized_chunk)")
        callback = source.index("checkpoint_callback(", extend)
        cleanup = source.index("_cleanup_uploaded_file(client, current_chunk_file_obj", callback)
        self.assertLess(extend, callback)
        self.assertLess(callback, cleanup)
        self.assertIn("if i < resume_completed_chunks:", source)
        self.assertIn("Gemini call skipped", source)

    def test_worker_protocol_carries_resume_state_and_checkpoint_events(self):
        worker = (ROOT / "bridge" / "audio_description_bridge.py").read_text(encoding="utf-8")
        rust_bridge = (ROOT / "src" / "tools" / "audio_description_bridge.rs").read_text(
            encoding="utf-8"
        )
        self.assertIn('"CHECKPOINT"', worker)
        self.assertIn("resume_completed_chunks=", worker)
        self.assertIn('line.strip_prefix("CHECKPOINT:")', rust_bridge)
        self.assertIn("AudioDescriptionBridgeResume", rust_bridge)

    def test_host_keeps_partial_checkpoint_until_success_and_exposes_continue_button(self):
        host = (ROOT / "src" / "audio_description.rs").read_text(encoding="utf-8")
        window = (ROOT / "src" / "app_windows" / "audio_description_window.rs").read_text(
            encoding="utf-8"
        )
        menu = (ROOT / "src" / "menu.rs").read_text(encoding="utf-8")
        main = (ROOT / "src" / "main.rs").read_text(encoding="utf-8")
        self.assertIn('"sonarpad-audio-description-partial"', host)
        self.assertIn('set_extension("sonarpad-ad.partial.json")', host)
        self.assertIn("save_audio_description_partial_checkpoint", host)
        self.assertIn("remove completed partial checkpoint", host)
        self.assertIn("ID_CONTINUE_INTERRUPTED", window)
        self.assertIn("continue_interrupted_from_window", window)
        self.assertNotIn("IDM_TOOLS_CONTINUE_AUDIO_DESCRIPTION", menu)
        self.assertNotIn("IDM_TOOLS_CONTINUE_AUDIO_DESCRIPTION", main)

        modify = window.index("let modify_project_button = CreateWindowExW")
        resume = window.index("let continue_interrupted_button = CreateWindowExW", modify)
        start = window.index("let start_button = CreateWindowExW", resume)
        self.assertLess(modify, resume)
        self.assertLess(resume, start)
        self.assertIn("PCWSTR(to_wide(&labels.resume_title).as_ptr())", window[resume:start])

    def test_manual_cancel_preserves_last_completed_checkpoint(self):
        host = (ROOT / "src" / "audio_description.rs").read_text(encoding="utf-8")
        bridge = (ROOT / "src" / "tools" / "audio_description_bridge.rs").read_text(
            encoding="utf-8"
        )
        window = (ROOT / "src" / "app_windows" / "audio_description_window.rs").read_text(
            encoding="utf-8"
        )
        success_cleanup = host.index('if checkpoint_path.exists() {')
        completion = host.index('notify_status(\n        &mut callbacks,\n        "complete"')
        self.assertGreater(success_cleanup, completion)
        self.assertIn('return Err("cancelled".to_string());', bridge)
        self.assertIn('Err(error) if error == "cancelled"', window)
        cancelled_branch = window.index('Err(error) if error == "cancelled"')
        cancelled_tail = window[cancelled_branch:cancelled_branch + 600]
        self.assertNotIn("remove_file", cancelled_tail)

    def test_resume_selector_owns_model_choice_and_starts_immediately(self):
        window = (ROOT / "src" / "app_windows" / "audio_description_window.rs").read_text(
            encoding="utf-8"
        )
        selector = (
            ROOT / "src" / "app_windows" / "audio_description_resume_window.rs"
        ).read_text(encoding="utf-8")
        self.assertIn('"audio_description.resume.model"', selector)
        self.assertIn("model_combo", selector)
        resume_start = window.index("fn continue_interrupted_from_window")
        resume_end = window.index("fn open_window", resume_start)
        resume = window[resume_start:resume_end]
        self.assertIn("selection.gemini_model", resume)
        self.assertIn("start_job(hwnd, state);", resume)
        self.assertNotIn("post_boxed_message(hwnd, WM_AD_SET_RESUME", resume)



    def test_screen_text_preference_is_saved_in_checkpoint_and_restored_on_resume(self):
        host = (ROOT / "src" / "audio_description.rs").read_text(encoding="utf-8")
        window = (ROOT / "src" / "app_windows" / "audio_description_window.rs").read_text(
            encoding="utf-8"
        )
        self.assertIn("recognize_screen_text: job.recognize_screen_text", host)
        self.assertIn("recognize_screen_text: checkpoint.recognize_screen_text", host)
        self.assertIn("pub recognize_screen_text: bool", host)
        self.assertIn("resume.recognize_screen_text", window)
        self.assertIn("state.recognize_screen_text_checkbox", window)

    def test_resume_model_label_is_localized_in_all_windows_locales(self):
        i18n_dir = ROOT / "i18n"
        locales = [
            "it", "en", "de", "es", "fr", "pt", "pt-BR", "cs", "pl",
            "ru", "vi", "sv", "sr", "uk", "lt", "zh", "hi",
        ]
        for locale in locales:
            text = (i18n_dir / f"{locale}.json").read_text(encoding="utf-8-sig")
            self.assertIn('"audio_description.resume.model"', text, locale)


if __name__ == "__main__":
    unittest.main()
