from __future__ import annotations

import io
import json
import tempfile
import unittest
from pathlib import Path
from unittest import mock

import audio_description_bridge as bridge


class BridgeProtocolTests(unittest.TestCase):
    def test_checkpoint_keeps_visual_evidence_for_resumed_descriptions(self):
        with mock.patch.object(
            bridge.audio_describer, "get_visual_evidence_time", return_value=11.25
        ):
            rows = bridge._checkpoint_descriptions([(10.0, 12.0, "Azione")])
        restored = json.loads(json.dumps(rows))
        self.assertEqual(restored[0], {
            "start_sec": 10.0, "end_sec": 12.0, "text": "Azione",
            "visual_evidence_time_sec": 11.25,
        })

    def test_checkpoint_without_visual_evidence_remains_compatible(self):
        with mock.patch.object(
            bridge.audio_describer, "get_visual_evidence_time", return_value=None
        ):
            rows = bridge._checkpoint_descriptions([None, (1,), (10.0, 12.0, "Azione")])
        self.assertEqual(rows, [{"start_sec": 10.0, "end_sec": 12.0, "text": "Azione"}])

    def test_quota_switch_returns_new_model_without_ending_worker(self):
        emitted = []
        reply = json.dumps({"action": "switch", "model": "gemini-3.5-flash"}) + "\n"
        with mock.patch.object(bridge.sys, "stdin", io.StringIO(reply)), mock.patch.object(
            bridge, "_emit", side_effect=lambda prefix, value: emitted.append((prefix, value))
        ):
            decision = bridge._quota_decision_handler(
                "gemini-3.5-flash-lite", RuntimeError("RESOURCE_EXHAUSTED")
            )
        self.assertEqual(decision, "gemini-3.5-flash")
        self.assertEqual(emitted[0][0], "QUOTA")
        self.assertEqual(emitted[0][1]["model"], "gemini-3.5-flash-lite")

    def test_quota_wait_keeps_same_request_pending(self):
        reply = json.dumps({"action": "wait"}) + "\n"
        with mock.patch.object(bridge.sys, "stdin", io.StringIO(reply)), mock.patch.object(
            bridge, "_emit"
        ):
            self.assertIsNone(
                bridge._quota_decision_handler("gemini-test", RuntimeError("quota"))
            )

    def test_quota_stop_or_closed_input_cancels_request(self):
        for input_text in (json.dumps({"action": "stop"}) + "\n", ""):
            with self.subTest(input_text=input_text), mock.patch.object(
                bridge.sys, "stdin", io.StringIO(input_text)
            ), mock.patch.object(bridge, "_emit"):
                self.assertFalse(
                    bridge._quota_decision_handler("gemini-test", RuntimeError("quota"))
                )


    def test_overload_wait_keeps_same_request_pending(self):
        emitted = []
        reply = json.dumps({"action": "wait"}) + "\n"
        with mock.patch.object(bridge.sys, "stdin", io.StringIO(reply)), mock.patch.object(
            bridge, "_emit", side_effect=lambda prefix, value: emitted.append((prefix, value))
        ):
            decision = bridge._overload_decision_handler(
                "gemini-flash-lite-latest", RuntimeError("503 high demand")
            )
        self.assertTrue(decision)
        self.assertEqual(emitted[0][0], "OVERLOAD")
        self.assertEqual(emitted[0][1]["model"], "gemini-flash-lite-latest")

    def test_overload_stop_or_closed_input_cancels_request(self):
        for input_text in (json.dumps({"action": "stop"}) + "\n", ""):
            with self.subTest(input_text=input_text), mock.patch.object(
                bridge.sys, "stdin", io.StringIO(input_text)
            ), mock.patch.object(bridge, "_emit"):
                self.assertFalse(
                    bridge._overload_decision_handler(
                        "gemini-test", RuntimeError("503 high demand")
                    )
                )

    def test_character_glossary_setting_is_forwarded_both_ways(self):
        for enabled in (True, False):
            captured = {}
            with self.subTest(enabled=enabled), mock.patch.object(
                bridge.config_model,
                "configure",
                side_effect=lambda values: captured.update(values),
            ), mock.patch.object(bridge.audio_describer, "reset_gemini_client"), mock.patch.object(
                bridge.gemini_helpers, "set_quota_decision_handler"
            ), mock.patch.object(
                bridge.gemini_helpers, "set_overload_decision_handler"
            ):
                bridge._configure_omni(
                    {
                        "language": "it",
                        "gemini_api_key": "test-key",
                        "gemini_model": "gemini-test",
                        "verbosity": "detailed",
                        "allow_extended_pauses": True,
                        "recognize_characters": enabled,
                    }
                )
            self.assertIs(captured["enable_character_glossary"], enabled)

    def test_audio_description_keeps_three_minute_chunks_and_default_frame_rate(self):
        captured = {}
        with mock.patch.object(
            bridge.config_model,
            "configure",
            side_effect=lambda values: captured.update(values),
        ), mock.patch.object(bridge.audio_describer, "reset_gemini_client"), mock.patch.object(
            bridge.gemini_helpers, "set_quota_decision_handler"
        ), mock.patch.object(
            bridge.gemini_helpers, "set_overload_decision_handler"
        ):
            bridge._configure_omni(
                {
                    "language": "it",
                    "gemini_api_key": "test-key",
                    "gemini_model": "gemini-test",
                    "verbosity": "detailed",
                }
            )

        self.assertEqual(bridge.CHUNK_DURATION_SECONDS, 180)
        self.assertEqual(captured["video_chunk_duration_seconds"], 180)
        self.assertEqual(captured["frame_rate_for_ai"], 0)

    def test_normalise_descriptions_filters_invalid_rows_and_sorts(self):
        normalized = bridge._normalise_descriptions(
            [
                (9, 10, "Seconda"),
                (2, 1, "Fine corretta"),
                (3, 4, ""),
                ("bad", 5, "Ignora"),
                (1, 2, "Prima"),
            ]
        )
        self.assertEqual([item["text"] for item in normalized], ["Prima", "Fine corretta", "Seconda"])
        self.assertEqual(normalized[1]["start_sec"], normalized[1]["end_sec"])

    def test_normalise_descriptions_preserves_visual_origin_after_alignment(self):
        normalized = bridge._normalise_descriptions(
            [(15.0, 17.0, "Azione")],
            source_descriptions=[(10.0, 12.0, "Azione")],
        )

        self.assertEqual(len(normalized), 1)
        self.assertAlmostEqual(normalized[0]["start_sec"], 15.0)
        self.assertAlmostEqual(normalized[0]["visual_start_sec"], 10.0)

    def test_normalise_descriptions_keeps_visual_evidence_as_passive_metadata(self):
        with mock.patch.object(
            bridge.audio_describer, "get_visual_evidence_time", return_value=11.25
        ):
            normalized = bridge._normalise_descriptions(
                [(15.0, 17.0, "Azione")],
                source_descriptions=[(10.0, 12.0, "Azione")],
            )
        self.assertAlmostEqual(normalized[0]["start_sec"], 15.0)
        self.assertAlmostEqual(normalized[0]["visual_start_sec"], 10.0)
        self.assertAlmostEqual(normalized[0]["visual_evidence_time_sec"], 11.25)

    def test_validate_request_accepts_host_prepared_media_and_rejects_bad_timeline(self):
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            media = root / "film.mkv"
            wav = root / "audio.wav"
            chunk1 = root / "chunk1.mp4"
            chunk2 = root / "chunk2.mp4"
            for path in (media, wav, chunk1, chunk2):
                path.write_bytes(b"x")
            request = {
                "input_path": str(media),
                "duration_sec": 360.0,
                "audio_wav_path": str(wav),
                "gemini_api_key": "key",
                "verbosity": "detailed",
                "chunks": [
                    {"path": str(chunk1), "start_sec": 0.0, "end_sec": 180.0},
                    {"path": str(chunk2), "start_sec": 180.0, "end_sec": 360.0},
                ],
            }
            bridge._validate_request(request)
            request["chunks"][1]["start_sec"] = 190.0
            with self.assertRaisesRegex(ValueError, "timeline"):
                bridge._validate_request(request)


    def test_gemini_status_uses_stable_localizable_stage_ids(self):
        emitted = []
        with mock.patch.object(
            bridge, "_emit", side_effect=lambda prefix, value: emitted.append((prefix, value))
        ):
            bridge._gemini_status("Chunk 2/5: asking Gemini for descriptions…")
        status = next(value for prefix, value in emitted if prefix == "STATUS")
        self.assertEqual(status["stage"], "gemini_chunk")
        self.assertEqual(json.loads(status["message"]), {"current": 2, "total": 5})

    def test_non_chunk_gemini_status_hides_english_worker_text(self):
        emitted = []
        with mock.patch.object(
            bridge, "_emit", side_effect=lambda prefix, value: emitted.append((prefix, value))
        ):
            bridge._gemini_status("Uploading video to Gemini...")
        status = next(value for prefix, value in emitted if prefix == "STATUS")
        self.assertEqual(status, {"stage": "gemini_uploading", "message": ""})

    def test_gemini_status_categorizes_worker_messages_without_forwarding_english(self):
        cases = {
            "Uploading video to Gemini...": "gemini_uploading",
            "Waiting for Gemini — watching scenes...": "gemini_waiting",
            "Contacting Gemini API (model: gemini-test)...": "gemini_contacting",
            "Gemini response received. Parsing...": "gemini_response",
            "Invalid JSON from AI — asking Gemini to reformat it correctly...": "gemini_repair",
            "Transient error; retrying request attempt 2...": "gemini_retry",
        }
        for message, expected_stage in cases.items():
            emitted = []
            with mock.patch.object(
                bridge, "_emit", side_effect=lambda prefix, value: emitted.append((prefix, value))
            ):
                bridge._gemini_status(message)
            status = next(value for prefix, value in emitted if prefix == "STATUS")
            self.assertEqual(status, {"stage": expected_stage, "message": ""})

    def test_windows_host_handles_overload_separately_from_quota(self):
        root = Path(__file__).resolve().parents[2]
        bridge_source = (root / "src" / "tools" / "audio_description_bridge.rs").read_text(encoding="utf-8")
        ui_source = (root / "src" / "app_windows" / "audio_description_window.rs").read_text(encoding="utf-8")
        self.assertIn('line.strip_prefix("OVERLOAD:")', bridge_source)
        self.assertIn("AudioDescriptionOverloadDecision::Wait", bridge_source)
        self.assertIn("WM_AD_OVERLOAD", ui_source)
        self.assertIn("MB_YESNO | MB_ICONQUESTION", ui_source)

    def test_windows_build_script_passes_python_selector_explicitly(self):
        script = (Path(__file__).resolve().parents[1] / "build_audio_description_bridge.ps1").read_text(
            encoding="utf-8"
        )
        self.assertNotIn("@launcherArgs", script)
        self.assertIn("& $Python $pythonSelector -m pip", script)
        self.assertIn("& $Python $pythonSelector -m PyInstaller", script)


    def test_validate_request_accepts_resume_and_rejects_progress_past_chunk_count(self):
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            media = root / "film.mkv"
            chunk1 = root / "chunk1.mkv"
            chunk2 = root / "chunk2.mkv"
            for path in (media, chunk1, chunk2):
                path.write_bytes(b"x")
            request = {
                "input_path": str(media),
                "duration_sec": 360.0,
                "gemini_api_key": "key",
                "verbosity": "detailed",
                "chunks": [
                    {"path": str(chunk1), "start_sec": 0.0, "end_sec": 180.0},
                    {"path": str(chunk2), "start_sec": 180.0, "end_sec": 360.0},
                ],
                "resume": {
                    "completed_chunks": 1,
                    "descriptions": [
                        {"start_sec": 10.0, "end_sec": 12.0, "text": "Scena."}
                    ],
                    "character_glossary": [],
                },
            }
            bridge._validate_request(request)
            request["resume"]["completed_chunks"] = 3
            with self.assertRaisesRegex(ValueError, "completed_chunks"):
                bridge._validate_request(request)

if __name__ == "__main__":
    unittest.main()
