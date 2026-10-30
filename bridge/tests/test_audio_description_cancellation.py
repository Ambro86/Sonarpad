import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
BRIDGE = (ROOT / "src" / "tools" / "audio_description_bridge.rs").read_text(encoding="utf-8")
WINDOW = (ROOT / "src" / "app_windows" / "audio_description_window.rs").read_text(encoding="utf-8")
DESCRIBER = (
    ROOT / "bridge" / "audio_description_runtime" / "audio_describer" / "core" / "audio_describer.py"
).read_text(encoding="utf-8")
TTS = (ROOT / "src" / "tts_engine.rs").read_text(encoding="utf-8")
SAPI5 = (ROOT / "src" / "sapi5_engine.rs").read_text(encoding="utf-8")


class AudioDescriptionCancellationTests(unittest.TestCase):
    def test_bridge_cancel_force_stops_worker_without_blocking_wait_or_join(self):
        helper_start = BRIDGE.index("fn terminate_bridge_process_tree")
        helper_end = BRIDGE.index("fn temporary_request_path", helper_start)
        helper = BRIDGE[helper_start:helper_end]
        self.assertIn("child.kill()", helper)
        self.assertIn('Command::new("taskkill")', helper)
        self.assertIn('["/PID", &pid.to_string(), "/T", "/F"]', helper)

        loop_start = BRIDGE.index("loop {", BRIDGE.index("pub fn run_audio_description_bridge"))
        quota_start = BRIDGE.index('line.strip_prefix("QUOTA:")', loop_start)
        loop_prefix = BRIDGE[loop_start:quota_start]
        cancel_start = loop_prefix.index("if cancel.load(Ordering::SeqCst)")
        cancel_block = loop_prefix[cancel_start:]
        self.assertIn("terminate_bridge_process_tree(&mut child)", cancel_block)
        self.assertNotIn("child.wait()", cancel_block)
        self.assertNotIn("stdout_thread.join()", cancel_block)
        self.assertNotIn("stderr_thread.join()", cancel_block)

    def test_dialog_cancel_uses_strong_flag_and_reports_completion(self):
        start = WINDOW.index("fn request_cancel(hwnd: HWND)")
        end = WINDOW.index("fn set_text", start)
        request = WINDOW[start:end]
        self.assertIn('"audio_description.cancel_confirm"', request)
        self.assertIn("MB_YESNO | MB_ICONQUESTION | MB_DEFBUTTON2", request)
        self.assertIn("Arc::ptr_eq(c, &pending.0)", request)
        confirmation = request.index("if answer == IDYES")
        self.assertNotIn("store(true, Ordering::SeqCst)", request[:confirmation])
        self.assertIn("pending.0.store(true, Ordering::SeqCst)", request[confirmation:])
        self.assertIn("Audio description: cancellation confirmed", request)
        self.assertIn("Audio description: cancellation completed; worker stopped", WINDOW)



    def test_tts_synthesis_reacts_to_cancel_while_a_voice_is_busy(self):
        self.assertIn("cancel: Some(config.cancel.as_ref())", TTS)
        self.assertIn("timeout(Duration::from_millis(100), read.next())", TTS)
        self.assertIn("async_sleep_with_cancellation", TTS)
        self.assertIn("SPF_ASYNC.0 | SPF_IS_XML.0", SAPI5)
        self.assertIn("SAPI5 export purge after cancellation failed", SAPI5)
        self.assertIn("SpeakCompleteEvent()", SAPI5)
        self.assertIn("WaitForSingleObject(completion_event, 25)", SAPI5)

    def test_user_cancel_is_logged_as_normal_control_flow_without_critical_traceback(self):
        cancel_start = DESCRIBER.index("except gemini.GeminiRetryCancelledError as e:")
        generic_start = DESCRIBER.index("except Exception as e:", cancel_start)
        cancel_block = DESCRIBER[cancel_start:generic_start]
        self.assertIn("Chunked generation cancelled by user", cancel_block)
        self.assertNotIn("failed critically", cancel_block)
        self.assertNotIn("exc_info=True", cancel_block)


if __name__ == "__main__":
    unittest.main()
