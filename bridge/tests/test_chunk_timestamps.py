import unittest
import json
from unittest import mock
from types import SimpleNamespace

from audio_describer.core import gemini_helpers
from audio_describer.core.audio_describer import (
    _TransientGeminiFileProcessingError,
    _description_belongs_to_gaps,
    _effective_chunk_duration,
    _extract_upload_chunk,
    _find_large_chunk_gaps,
    _build_unified_prompts,
    _normalize_blocked_minute_timestamps,
    _normalize_chunk_timestamps,
    _mmss_to_total_seconds,
    _post_process_mmss_timestamps,
    _parse_unified_response,
    _register_visual_evidence_records,
    _retime_visual_evidence_records,
    _reset_visual_evidence_registry,
    get_visual_evidence_time,
    _requires_per_chunk_upload,
    _should_use_per_chunk_uploads,
    _suppress_repeated_leading_character_names,
    _format_character_continuity,
    _format_recent_description_context,
    _generate_blocked_chunk_by_minutes,
    generate_descriptions_chunked,
    _items_for_minute,
    _video_metadata_kwargs,
    _update_character_continuity,
    _upload_and_wait_for_active,
)


class ChunkTimestampTests(unittest.TestCase):
    def test_repeated_character_name_filter_is_generic_and_context_safe(self):
        glossary = [
            {"name": "Janet"},
            {"name": "Mark"},
            {"name": "Bruce Nolan"},
        ]
        descriptions = [
            (0.0, 2.0, "Janet apre la porta."),
            (3.0, 5.0, "Janet entra nella stanza."),
            (6.0, 8.0, "Janet guarda fuori dalla finestra."),
            (9.0, 11.0, "Janet e Mark scendono le scale."),
            (12.0, 14.0, "Mark accende la luce."),
            (15.0, 17.0, "La bambina osserva Janet."),
            (18.0, 20.0, "Janet si ritrae."),
            (21.0, 23.0, "Bruce Nolan prende la collana."),
            (24.0, 26.0, "Bruce Nolan la lancia via."),
            (60.0, 62.0, "Bruce Nolan torna alla porta."),
        ]

        result = _suppress_repeated_leading_character_names(
            descriptions, glossary
        )

        self.assertEqual([item[2] for item in result], [
            "Janet apre la porta.",
            "Entra nella stanza.",
            "Guarda fuori dalla finestra.",
            "Janet e Mark scendono le scale.",
            "Mark accende la luce.",
            "La bambina osserva Janet.",
            "Janet si ritrae.",
            "Bruce Nolan prende la collana.",
            "La lancia via.",
            "Bruce Nolan torna alla porta.",
        ])

    def test_video_metadata_offsets_remove_float_artifacts(self):
        with mock.patch(
            "audio_describer.core.audio_describer.config_model.get_setting",
            return_value=None,
        ):
            metadata = _video_metadata_kwargs(180.0, 180.06900000000041)

        self.assertEqual(metadata["start_offset"], "180.0s")
        self.assertEqual(metadata["end_offset"], "180.069s")

    def test_minute_fallback_assigns_slot_by_midpoint_and_resets_timeline(self):
        slots = [
            {"id": "first", "start": 58.0, "end": 62.0, "max_words": 8},
            {"id": "second", "start": 75.0, "end": 80.0, "max_words": 10},
        ]

        result = _items_for_minute(slots, 60.0, 120.0, 60.0)

        self.assertEqual([item["id"] for item in result], ["first", "second"])
        self.assertEqual((result[0]["start"], result[0]["end"]), (0.0, 2.0))
        self.assertEqual((result[1]["start"], result[1]["end"]), (15.0, 20.0))

    def test_blocked_minute_accepts_parent_chunk_timestamps_without_double_offset(self):
        result = _normalize_blocked_minute_timestamps(
            [(180.0, 181.5, "Sam punta la pistola.")],
            minute_start=540.189,
            minute_end=541.857,
            chunk_start=360.189,
            chunk_number=3,
            minute_number=4,
        )

        self.assertEqual(len(result), 1)
        self.assertAlmostEqual(result[0][0], 540.189)
        self.assertAlmostEqual(result[0][1], 541.689)
        self.assertEqual(result[0][2], "Sam punta la pistola.")

    def test_blocked_minute_still_accepts_requested_minute_local_timestamps(self):
        result = _normalize_blocked_minute_timestamps(
            [(5.0, 7.0, "Scena locale.")],
            minute_start=420.189,
            minute_end=480.189,
            chunk_start=360.189,
            chunk_number=3,
            minute_number=2,
        )

        self.assertEqual(len(result), 1)
        self.assertAlmostEqual(result[0][0], 425.189)
        self.assertAlmostEqual(result[0][1], 427.189)

    def test_blocked_minute_accepts_full_video_timestamps(self):
        result = _normalize_blocked_minute_timestamps(
            [(425.0, 427.0, "Scena assoluta.")],
            minute_start=420.189,
            minute_end=480.189,
            chunk_start=360.189,
            chunk_number=3,
            minute_number=2,
        )

        self.assertEqual(len(result), 1)
        self.assertAlmostEqual(result[0][0], 425.0)
        self.assertAlmostEqual(result[0][1], 427.0)

    def test_blocked_chunk_keeps_successful_minutes_and_skips_blocked_one(self):
        responses = [object(), gemini_helpers.ContentBlockedError(
            "blocked", reason="PROHIBITED_CONTENT"
        ), object()]
        parsed = [
            ([('00:10', '00:12', 'Prima scena.')], []),
            ([('00:05', '00:07', 'Terza scena.')], []),
        ]
        settings = {
            "application_language": "it",
            "enable_character_glossary": False,
            "gemini_description_verbosity": "standard",
        }
        statuses = []

        with (
            mock.patch(
                "audio_describer.core.audio_describer.config_model.get_setting",
                side_effect=lambda key: settings.get(key),
            ),
            mock.patch(
                "audio_describer.core.audio_describer._prepare_video_for_gemini",
                side_effect=[("part1", None), ("part2", None), ("part3", None)],
            ),
            mock.patch(
                "audio_describer.core.audio_describer.gemini.generate_content_with_retry",
                side_effect=responses,
            ) as generate,
            mock.patch(
                "audio_describer.core.audio_describer.gemini.process_gemini_response",
                return_value=("{}", True),
            ),
            mock.patch(
                "audio_describer.core.audio_describer._parse_with_optional_json_repair",
                side_effect=parsed,
            ),
            mock.patch(
                "audio_describer.core.audio_describer._correct_description_language",
                side_effect=lambda _client, _model, descriptions, _status: (
                    descriptions, None
                ),
            ),
            mock.patch(
                "audio_describer.core.audio_describer.gemini.build_generation_config",
                return_value=object(),
            ),
            mock.patch(
                "audio_describer.core.audio_describer.gemini.save_raw_ai_output"
            ),
            mock.patch(
                "audio_describer.core.audio_describer.gemini.log_token_usage",
                return_value=None,
            ),
            mock.patch("audio_describer.core.audio_describer.os.unlink"),
        ):
            descriptions, glossary, usage = _generate_blocked_chunk_by_minutes(
                client=object(),
                model_name="gemini-test",
                video_path="movie.mp4",
                prepared_chunk_path="prepared_chunk.mp4",
                chunk_start=0.0,
                chunk_end=180.069,
                chunk_number=8,
                total_chunks=28,
                clipped_windows=[],
                intensive_slots=[],
                extended_anchors=[],
                intensive_mode=False,
                extended_mode=False,
                user_prompt="",
                character_continuity={},
                prior_descriptions=[],
                status_update_callback=statuses.append,
            )

        self.assertEqual(descriptions, [
            (10.0, 12.0, "Prima scena."),
            (125.0, 127.0, "Terza scena."),
        ])
        self.assertEqual(glossary, [])
        self.assertEqual(usage, [])
        self.assertEqual(generate.call_count, 3)
        self.assertFalse(any("minute 4" in status for status in statuses))
        self.assertTrue(all(
            call.kwargs["prohibited_content_max_attempts"] == 1
            for call in generate.call_args_list
        ))
        self.assertTrue(any("original audio" in status for status in statuses))

    def test_gemini_colon_milliseconds_preserve_exact_timestamp(self):
        self.assertAlmostEqual(_mmss_to_total_seconds("01:13:473"), 73.473)
        self.assertAlmostEqual(_mmss_to_total_seconds("02:17:160"), 137.160)
        self.assertAlmostEqual(_mmss_to_total_seconds("00:13:026"), 13.026)

    def test_hour_timestamps_remain_unambiguous(self):
        self.assertAlmostEqual(_mmss_to_total_seconds("01:02:03.500"), 3723.5)
        self.assertAlmostEqual(_mmss_to_total_seconds("01:02:03"), 3723.0)
        self.assertAlmostEqual(_mmss_to_total_seconds("01:02:03:500"), 3723.5)

    def test_local_recovery_repairs_two_digit_colon_fraction_only_in_window(self):
        corrected = _post_process_mmss_timestamps(
            [
                ("02:02:18", "02:19:52", "Cena sulla zattera."),
                ("02:54:53", "03:00:23", "Orizzonte serale."),
            ],
            local_timeline_window=(0.0, 182.110),
        )

        self.assertEqual(corrected, [
            (122.18, 139.52, "Cena sulla zattera."),
            (174.53, 180.23, "Orizzonte serale."),
        ])

    def test_logged_chunk_two_recovery_entries_cover_all_three_missing_ranges(self):
        corrected = _post_process_mmss_timestamps(
            [
                ("01:54:29", "02:08:43", "Cena illuminata da una candela."),
                ("02:22:55", "02:37:08", "Zattera al tramonto."),
                ("02:54:03", "02:57:12", "Costa frastagliata."),
            ],
            local_timeline_window=(0.0, 182.110),
        )
        gaps = [
            (114.498, 128.712),
            (142.925, 157.139),
            (174.053, 177.203),
        ]

        self.assertTrue(all(
            _description_belongs_to_gaps(item, [gap])
            for item, gap in zip(corrected, gaps)
        ))

    def test_locke_key_s2e2_recovery_colon_fractions_stay_inside_local_chunks(self):
        chunk_three = _post_process_mmss_timestamps(
            [
                ("02:20:09", "02:39:57", "Josh guarda altrove."),
                ("02:53:52", "02:55:32", "L'insegnante guarda la classe."),
            ],
            local_timeline_window=(0.0, 181.0),
        )
        chunk_eleven = _post_process_mmss_timestamps(
            [
                ("02:05:08", "02:15:29", "Gabe guarda intorno."),
                ("02:28:49", "02:38:55", "Bode beve."),
                ("02:38:55", "02:50:21", "L'attrezzo viene afferrato."),
            ],
            local_timeline_window=(0.0, 184.088),
        )

        self.assertEqual(
            [(140.09, 159.57), (173.52, 175.32)],
            [(start, end) for start, end, _text in chunk_three],
        )
        self.assertEqual(
            [(125.08, 135.29), (148.49, 158.55), (158.55, 170.21)],
            [(start, end) for start, end, _text in chunk_eleven],
        )

    def test_local_recovery_does_not_reinterpret_valid_hour_timestamp(self):
        corrected = _post_process_mmss_timestamps(
            [("01:02:03", "01:02:08", "Video lungo.")],
            local_timeline_window=(3600.0, 3900.0),
        )

        self.assertEqual(corrected, [(3723.0, 3728.0, "Video lungo.")])

    def test_local_recovery_keeps_out_of_window_typo_for_range_audit(self):
        corrected = _post_process_mmss_timestamps(
            [("09:30:20", "09:31:20", "Fuori dal chunk.")],
            local_timeline_window=(0.0, 182.110),
        )

        self.assertEqual(corrected, [(34220.0, 34280.0, "Fuori dal chunk.")])

    def test_recovery_visual_evidence_follows_contextually_repaired_timestamp(self):
        records = [
            (7338.0, 8392.0, "Cena sulla zattera.", 120.5),
        ]
        descriptions = [
            (304.62, 321.96, "Cena sulla zattera."),
        ]

        retimed = _retime_visual_evidence_records(
            records, descriptions, timeline_offset_sec=182.44
        )

        self.assertEqual(len(retimed), 1)
        self.assertAlmostEqual(retimed[0][0], 122.18)
        self.assertAlmostEqual(retimed[0][1], 139.52)
        self.assertEqual(retimed[0][2:], ("Cena sulla zattera.", 120.5))

    def test_normalized_colon_milliseconds_still_face_chunk_range_audit(self):
        corrected = _post_process_mmss_timestamps([
            ("01:13:473", "01:17:210", "Dentro il chunk."),
            ("99:13:473", "99:17:210", "Fuori dal chunk."),
        ])

        self.assertEqual(len(corrected), 2)
        normalized = _normalize_chunk_timestamps(
            corrected,
            chunk_start=360.138,
            chunk_end=543.689,
            chunk_number=3,
            force_mode="relative",
        )
        self.assertEqual(len(normalized), 1)
        self.assertAlmostEqual(normalized[0][0], 433.611)
        self.assertAlmostEqual(normalized[0][1], 437.348)
        self.assertEqual(normalized[0][2], "Dentro il chunk.")

    def test_invalid_colon_millisecond_seconds_are_rejected(self):
        with self.assertRaises(ValueError):
            _mmss_to_total_seconds("01:73:473")

    def test_out_of_order_gemini_entries_keep_their_scene_timestamps(self):
        raw = [
            ("01:31.500", "01:35.997", "Jasmine con la tigre."),
            ("01:04.259", "01:05.666", "Una lanterna oscilla."),
            ("01:07.567", "01:11.000", "Jafar percorre il corridoio."),
            (
                "01:11.000",
                "01:15.200",
                "Jafar mostra l'anello scintillante.",
            ),
        ]

        corrected = _post_process_mmss_timestamps(raw)

        self.assertEqual(
            [item[2] for item in corrected],
            [
                "Una lanterna oscilla.",
                "Jafar percorre il corridoio.",
                "Jafar mostra l'anello scintillante.",
                "Jasmine con la tigre.",
            ],
        )
        ring = corrected[2]
        self.assertAlmostEqual(ring[0], 71.0)
        self.assertAlmostEqual(ring[1], 75.2)

    def test_overlapping_gemini_entries_preserve_visual_start_times(self):
        corrected = _post_process_mmss_timestamps([
            ("00:10.000", "00:16.000", "Prima azione."),
            ("00:12.000", "00:14.000", "Seconda azione."),
        ])

        self.assertEqual(len(corrected), 2)
        self.assertAlmostEqual(corrected[0][0], 10.0)
        self.assertAlmostEqual(corrected[1][0], 12.0)
        self.assertAlmostEqual(corrected[1][1], 14.0)

    def test_code_13_file_processing_retries_until_upload_succeeds(self):
        client = object()
        success = object()
        failures = [
            _TransientGeminiFileProcessingError(
                "Code 13", SimpleNamespace(name=f"files/failed-{index}")
            )
            for index in range(2)
        ]
        statuses = []

        with (
            mock.patch(
                "audio_describer.core.audio_describer._upload_and_wait_for_active_once",
                side_effect=[*failures, success],
            ) as upload_once,
            mock.patch(
                "audio_describer.core.audio_describer._cleanup_uploaded_file"
            ) as cleanup,
            mock.patch("audio_describer.core.audio_describer.time.sleep") as sleep,
        ):
            result = _upload_and_wait_for_active(
                client, "chunk.mp4", statuses.append
            )

        self.assertIs(result, success)
        self.assertEqual(upload_once.call_count, 3)
        self.assertEqual(cleanup.call_count, 2)
        self.assertEqual(sleep.call_count, 2)
        self.assertIn("retry 2", statuses[-1].lower())

    def test_character_continuity_keeps_only_named_characters(self):
        known = {}
        _update_character_continuity(known, [
            {"id": "stanley", "name": "Stanley", "description": "Uomo magro."},
            {"id": "stranger", "name": None, "description": "Uomo alto."},
        ])

        self.assertEqual(list(known), ["id:stanley"])
        self.assertIn("Stanley", _format_character_continuity(known))
        self.assertNotIn("Uomo alto", _format_character_continuity(known))

    def test_character_continuity_deduplicates_and_keeps_best_description(self):
        known = {}
        _update_character_continuity(known, [
            {"name": "Stanley", "description": "Capelli castani e abito grigio."},
        ])
        _update_character_continuity(known, [
            {"name": " stanley ", "description": "Uomo."},
        ])

        self.assertEqual(len(known), 1)
        self.assertEqual(
            known["name:stanley"]["description"], "Capelli castani e abito grigio."
        )

    def test_character_continuity_is_injected_into_next_chunk_prompt(self):
        settings = {
            "application_language": "it",
            "enable_character_glossary": True,
            "gemini_description_verbosity": "standard",
        }
        continuity = (
            '[{"name":"Stanley","description":"Uomo magro con capelli castani."}]'
        )
        with mock.patch(
            "audio_describer.core.audio_describer.config_model.get_setting",
            side_effect=lambda key: settings.get(key),
        ):
            _system, prompt = _build_unified_prompts(
                "", "gemini-test", character_continuity_text=continuity
            )

        self.assertIn("ESTABLISHED CHARACTER CONTINUITY", prompt)
        self.assertIn("Stanley", prompt)
        self.assertIn("may be reused without being spoken again", prompt)
        self.assertIn("when uncertain, use a generic label", prompt)

    def test_prompt_uses_audio_for_identity_but_never_repeats_dialogue(self):
        settings = {
            "application_language": "it",
            "enable_character_glossary": True,
            "gemini_description_verbosity": "standard",
        }
        with mock.patch(
            "audio_describer.core.audio_describer.config_model.get_setting",
            side_effect=lambda key: settings.get(key),
        ):
            system, _prompt = _build_unified_prompts("", "gemini-test")

        self.assertIn("USE AUDIO ONLY AS PRIVATE CONTEXT", system)
        self.assertIn("spoken names, titles", system)
        self.assertIn("identify or disambiguate visible characters", system)
        self.assertIn("do not transcribe, quote, translate, paraphrase", system)
        self.assertIn("complete, echo, answer, or restate any spoken line", system)
        self.assertIn("identity label", system)
        self.assertIn("facts learned only from speech", system)
        self.assertIn("describe only new visual information", system)

    def test_audio_context_rule_also_protects_recovery_and_json_repair(self):
        from pathlib import Path
        import audio_describer.core.audio_describer as audio_describer_module

        source = Path(audio_describer_module.__file__).read_text(encoding="utf-8")
        self.assertGreaterEqual(source.count("_AUDIO_CONTEXT_ONLY_RULE"), 4)
        self.assertIn(
            '"state. " + recovery_subject_rule + _AUDIO_CONTEXT_ONLY_RULE +',
            source,
        )
        self.assertIn(
            '"- " + _AUDIO_CONTEXT_ONLY_RULE + "\\n" +',
            source,
        )

    def test_intensive_prompt_forbids_borrowing_actions_from_other_times(self):
        settings = {
            "application_language": "it",
            "enable_character_glossary": True,
            "gemini_description_verbosity": "detailed",
        }
        with mock.patch(
            "audio_describer.core.audio_describer.config_model.get_setting",
            side_effect=lambda key: settings.get(key),
        ):
            system, prompt = _build_unified_prompts(
                "",
                "gemini-test",
                intensive_slots_text="S0001=79.000-82.000 (max 6 words)",
                intensive_mode=True,
            )

        self.assertIn("GROUND EVERY DESCRIPTION IN ITS EXACT TIME RANGE", system)
        self.assertIn("Never borrow, move, or repeat an action", system)
        self.assertIn("Temporal grounding has absolute priority", system)
        self.assertIn("static or mundane description", system)
        self.assertIn("logo, title card", system)
        self.assertIn("inspect only the frames inside that slot", prompt)
        self.assertIn("never pull an action from a preceding or following scene", prompt)
        self.assertIn("including a logo or title card", prompt)
        self.assertIn("Temporal correctness is more important", prompt)

    def test_prompt_reads_narrative_on_screen_text_without_bypassing_silences(self):
        settings = {
            "application_language": "it",
            "enable_character_glossary": False,
            "gemini_description_verbosity": "standard",
            "recognize_screen_text": True,
        }
        with mock.patch(
            "audio_describer.core.audio_describer.config_model.get_setting",
            side_effect=lambda key: settings.get(key),
        ):
            system, prompt = _build_unified_prompts(
                "", "gemini-test", dialogue_free_windows="10.000-13.000"
            )

        self.assertIn("READ NARRATIVELY IMPORTANT ON-SCREEN TEXT", system)
        self.assertIn("Three years later", system)
        self.assertIn("dates, locations", system)
        self.assertIn("letters or messages", system)
        self.assertIn("persistent channel logos/watermarks", system)
        self.assertIn("subtitles/", system)
        self.assertIn("repeat audible dialogue", system)
        self.assertIn("NEVER overrides dialogue", system)
        self.assertIn("dialogue-free window", system)
        self.assertIn("visual_evidence_time_seconds", system)
        self.assertIn("Authoritative dialogue-free windows", prompt)
        self.assertIn("never place a description outside them", prompt)
        self.assertIn('three top-level keys:', system)
        self.assertIn('"on_screen_text"', system)
        self.assertIn('"narratively_relevant"', system)
        self.assertIn("including text visible during dialogue", system)
        self.assertIn("Otherwise keep it only in on_screen_text", system)
        example = json.loads(system.split("**EXAMPLE OUTPUT:**", 1)[1])
        self.assertEqual(example["on_screen_text"], [])

    def test_prompt_keeps_legacy_behavior_when_screen_text_recognition_is_disabled(self):
        settings = {
            "application_language": "it",
            "enable_character_glossary": False,
            "gemini_description_verbosity": "standard",
            "recognize_screen_text": False,
        }
        with mock.patch(
            "audio_describer.core.audio_describer.config_model.get_setting",
            side_effect=lambda key: settings.get(key),
        ):
            system, _prompt = _build_unified_prompts(
                "", "gemini-test", dialogue_free_windows="10.000-13.000"
            )

        self.assertNotIn("READ NARRATIVELY IMPORTANT ON-SCREEN TEXT", system)
        self.assertNotIn("Three years later", system)
        self.assertNotIn("persistent channel logos/watermarks", system)
        self.assertNotIn("on_screen_text", system)
        self.assertIn("two top-level keys:", system)
        example = json.loads(system.split("**EXAMPLE OUTPUT:**", 1)[1])
        self.assertEqual(set(example), {"character_glossary", "audio_descriptions"})

    def test_screen_text_metadata_never_changes_narration_or_parse_success(self):
        description = {"start_time_mmss": "00:10.000", "end_time_mmss": "00:12.000",
                       "visual_evidence_time_seconds": 11.25, "description_text": "Una libreria"}
        metadata_cases = [None, "invalid", [], [None], [
            {"text": "LIBRI", "visual_evidence_time_seconds": 8.0,
             "narratively_relevant": True}]]
        for enabled in (False, True):
            for metadata in metadata_cases:
                with self.subTest(enabled=enabled, metadata=metadata), mock.patch(
                    "audio_describer.core.audio_describer.config_model.get_setting",
                    return_value=enabled,
                ), mock.patch("audio_describer.core.audio_describer.app_logger.info") as log:
                    response = {"character_glossary": [], "audio_descriptions": [description]}
                    if metadata is not None:
                        response["on_screen_text"] = metadata
                    descs, glossary, ok = _parse_unified_response(json.dumps(response), None)
                    self.assertTrue(ok)
                    self.assertEqual(descs, [("00:10.000", "00:12.000", "Una libreria")])
                    self.assertEqual(glossary, [])
                    self.assertEqual(log.called, enabled)

    def test_recent_descriptions_preserve_subject_flow_across_chunk_boundary(self):
        recent = _format_recent_description_context([
            (170.0, 174.0, "Bruce Nolan raccoglie una collanina."),
            (176.0, 179.0, "Bruce Nolan la stringe nel pugno."),
        ], 180.0)
        settings = {
            "application_language": "it",
            "enable_character_glossary": False,
            "gemini_description_verbosity": "standard",
        }
        with mock.patch(
            "audio_describer.core.audio_describer.config_model.get_setting",
            side_effect=lambda key: settings.get(key),
        ):
            _system, prompt = _build_unified_prompts(
                "", "gemini-test", recent_descriptions_text=recent
            )

        self.assertIn("RECENT DESCRIPTIONS IMMEDIATELY BEFORE THIS CLIP", prompt)
        self.assertIn("Bruce Nolan la stringe", prompt)
        self.assertIn("repeating that subject's name", prompt)

    def test_physical_chunk_preparation_is_delegated_to_sonarpad_rust(self):
        with self.assertRaises(gemini_helpers.GeminiAPIError) as raised:
            _extract_upload_chunk("movie.mp4", 180, 360, 2)
        self.assertIn("Sonarpad's Rust FFmpeg backend", str(raised.exception))

    def test_files_at_gemini_two_gib_limit_use_per_chunk_uploads(self):
        with mock.patch(
            "audio_describer.core.audio_describer.os.path.getsize",
            return_value=2 * 1024 * 1024 * 1024,
        ):
            self.assertTrue(_requires_per_chunk_upload("large.mp4"))

        with mock.patch(
            "audio_describer.core.audio_describer.os.path.getsize",
            return_value=2 * 1024 * 1024 * 1024 - 1,
        ):
            self.assertFalse(_requires_per_chunk_upload("small-enough.mp4"))

    def test_multi_chunk_video_uses_physical_upload_chunks(self):
        with mock.patch(
            "audio_describer.core.audio_describer.os.path.getsize",
            return_value=100 * 1024 * 1024,
        ):
            self.assertTrue(_should_use_per_chunk_uploads("movie.mp4", 2))
            self.assertFalse(_should_use_per_chunk_uploads("short.mp4", 1))

    def test_single_chunk_over_media_limit_keeps_safety_fallback(self):
        with mock.patch(
            "audio_describer.core.audio_describer.os.path.getsize",
            return_value=2 * 1024 * 1024 * 1024,
        ):
            self.assertTrue(_should_use_per_chunk_uploads("huge.mp4", 1))

    def test_all_long_chunks_are_capped_to_three_minutes(self):
        self.assertEqual(_effective_chunk_duration(600), 180.0)
        self.assertEqual(_effective_chunk_duration(180), 180.0)
        self.assertEqual(_effective_chunk_duration(120), 120.0)

    def test_late_first_relative_timestamp_gets_chunk_offset(self):
        # This is the case the old first-timestamp heuristic could misclassify:
        # the first event is late in a 600-second chunk.
        result = _normalize_chunk_timestamps(
            [(500.0, 503.0, "late event"), (550.0, 552.0, "later event")],
            chunk_start=600.0,
            chunk_end=1200.0,
            chunk_number=2,
        )
        self.assertEqual(result[0][:2], (1100.0, 1103.0))
        self.assertEqual(result[1][:2], (1150.0, 1152.0))

    def test_absolute_timestamps_are_not_shifted(self):
        result = _normalize_chunk_timestamps(
            [(720.0, 723.0, "absolute event"), (900.0, 902.0, "another")],
            chunk_start=600.0,
            chunk_end=1200.0,
            chunk_number=2,
        )
        self.assertEqual(result[0][:2], (720.0, 723.0))

    def test_extracted_clip_forces_local_timestamps_even_when_ambiguous(self):
        result = _normalize_chunk_timestamps(
            [(36.0, 39.0, "scene in extracted clip")],
            chunk_start=180.0,
            chunk_end=360.0,
            chunk_number=2,
            force_mode="relative",
        )
        self.assertEqual(result, [(216.0, 219.0, "scene in extracted clip")])

    def test_out_of_range_timestamp_is_rejected(self):
        result = _normalize_chunk_timestamps(
            [(620.0, 622.0, "valid"), (1800.0, 1802.0, "invalid")],
            chunk_start=600.0,
            chunk_end=1200.0,
            chunk_number=2,
        )
        self.assertEqual(result, [(620.0, 622.0, "valid")])

    def test_small_boundary_drift_is_clamped(self):
        result = _normalize_chunk_timestamps(
            [(599.5, 602.0, "boundary")],
            chunk_start=600.0,
            chunk_end=1200.0,
            chunk_number=2,
        )
        self.assertEqual(result, [(600.0, 602.0, "boundary")])

    def test_large_uncovered_chunk_tail_is_detected(self):
        descriptions = [
            (4.5, 10.0, "inizio"),
            (382.0, 388.0, "ultima descrizione"),
        ]

        self.assertEqual(
            _find_large_chunk_gaps(descriptions, 0.0, 600.0),
            [(10.0, 382.0), (388.0, 600.0)],
        )

    def test_completely_empty_chunk_is_recovered(self):
        self.assertEqual(
            _find_large_chunk_gaps([], 600.0, 1200.0),
            [(600.0, 1200.0)],
        )

    def test_short_gaps_are_not_recovered(self):
        descriptions = [
            (4.0, 10.0, "uno"),
            (80.0, 85.0, "due"),
            (160.0, 170.0, "tre"),
        ]

        self.assertEqual(_find_large_chunk_gaps(descriptions, 0.0, 200.0), [])

    def test_recovery_result_must_belong_to_requested_gap(self):
        gaps = [(388.0, 600.0)]
        self.assertTrue(
            _description_belongs_to_gaps((450.0, 455.0, "recuperata"), gaps)
        )
        self.assertFalse(
            _description_belongs_to_gaps((704.0, 708.0, "fuori intervallo"), gaps)
        )

    def test_intensive_prompt_requires_every_numbered_slot(self):
        settings = {
            "application_language": "it",
            "enable_character_glossary": False,
            "gemini_description_verbosity": "standard",
        }
        with mock.patch(
            "audio_describer.core.audio_describer.config_model.get_setting",
            side_effect=lambda key: settings.get(key),
        ):
            system, prompt = _build_unified_prompts(
                "", "gemini-test", "0.000-10.000",
                "S0001=0.000-4.000 (max 8 words), "
                "S0002=6.000-10.000 (max 8 words)",
            )

        self.assertIn("FILL EVERY USABLE SILENCE", system)
        self.assertIn("at least one", prompt)
        self.assertIn("may add further entries inside a slot", prompt)
        self.assertNotIn("exactly one", prompt)
        self.assertIn("S0002=6.000-10.000", prompt)
        self.assertIn("NEVER REPEAT AN ACTION", system)
        self.assertIn("does not permit repeating or paraphrasing", prompt)
        self.assertIn("slot boundaries are only the allowed container", prompt)
        self.assertIn("Never delay an action to a later part of the same slot", prompt)
        self.assertIn("returned start/end must coincide with the moment", system)
        self.assertIn("AVOID REPETITIVE SUBJECT LABELS", system)
        self.assertIn("Do not introduce a name", system)

    def test_intensive_prompt_explains_long_silence_partitions_without_second_pass(self):
        settings = {
            "application_language": "it",
            "enable_character_glossary": False,
            "gemini_description_verbosity": "standard",
        }
        with mock.patch(
            "audio_describer.core.audio_describer.config_model.get_setting",
            side_effect=lambda key: settings.get(key),
        ):
            _system, prompt = _build_unified_prompts(
                "", "gemini-test", "0.000-30.000",
                "S0001P001=0.000-15.000 (max 30 words; LONG_SILENCE_PART 1/2; inspect this part independently), "
                "S0001P002=15.000-30.000 (max 30 words; LONG_SILENCE_PART 2/2; inspect this part independently)",
                intensive_mode=True,
            )

        self.assertIn("LONG-SILENCE PARTITIONS", prompt)
        self.assertIn("artificial balanced subdivision", prompt)
        self.assertIn("independent visual checkpoint", prompt)
        self.assertIn("Never carry an action", prompt)
        self.assertIn("never borrow an action from a later part", prompt)
        self.assertIn("visual_evidence_time_seconds", _system)
        self.assertIn("exact evidence frame", prompt)
        self.assertIn("MUST fall inside", _system)

    def test_visual_evidence_metadata_is_captured_without_changing_timestamps(self):
        _reset_visual_evidence_registry()
        response = (
            '{"character_glossary":[],"audio_descriptions":['
            '{"start_time_mmss":"00:10.000","end_time_mmss":"00:12.000",'
            '"visual_evidence_time_seconds":11.25,"description_text":"Azione visibile"}'
            ']}'
        )
        descriptions, _glossary, parse_ok = _parse_unified_response(response, None)
        self.assertTrue(parse_ok)
        self.assertEqual(descriptions, [("00:10.000", "00:12.000", "Azione visibile")])
        from audio_describer.core import audio_describer as describer_module
        _register_visual_evidence_records(
            list(describer_module._LAST_PARSED_VISUAL_EVIDENCE_RAW),
            timeline_offset_sec=100.0,
        )
        self.assertAlmostEqual(
            get_visual_evidence_time(110.0, "Azione visibile"), 111.25
        )

    def test_intensive_prompt_with_no_slots_forbids_invented_timestamps(self):
        settings = {
            "application_language": "it",
            "enable_character_glossary": False,
            "gemini_description_verbosity": "standard",
        }
        with mock.patch(
            "audio_describer.core.audio_describer.config_model.get_setting",
            side_effect=lambda key: settings.get(key),
        ):
            _system, prompt = _build_unified_prompts(
                "", "gemini-test", "", "", intensive_mode=True
            )
        self.assertIn("empty `audio_descriptions` array", prompt)

    def test_intensive_short_gap_anchors_do_not_require_media_pauses(self):
        settings = {
            "application_language": "it",
            "enable_character_glossary": False,
            "gemini_description_verbosity": "standard",
        }
        with mock.patch(
            "audio_describer.core.audio_describer.config_model.get_setting",
            side_effect=lambda key: settings.get(key),
        ):
            _system, prompt = _build_unified_prompts(
                "", "gemini-test", intensive_mode=True,
                extended_anchors_text="E0001=PAUSE 4.000-5.200 -> IMMEDIATE_SCENE 5.200-9.200",
            )

        self.assertIn("OPTIONAL INTENSIVE SHORT-GAP", prompt)
        self.assertIn("not mandatory", prompt)
        self.assertIn("will NOT be paused", prompt)
        self.assertIn("cannot fit naturally", prompt)

    def test_extended_mode_allows_pause_only_as_short_gap_fallback(self):
        settings = {
            "application_language": "it",
            "enable_character_glossary": False,
            "gemini_description_verbosity": "standard",
        }
        with mock.patch(
            "audio_describer.core.audio_describer.config_model.get_setting",
            side_effect=lambda key: settings.get(key),
        ):
            _system, prompt = _build_unified_prompts(
                "", "gemini-test", intensive_mode=True,
                extended_mode=True,
                extended_anchors_text="E0001=PAUSE 4.000-5.200 -> IMMEDIATE_SCENE 5.200-9.200",
            )

        self.assertIn("OPTIONAL INTENSIVE SHORT-GAP", prompt)
        self.assertIn("may pause", prompt)
        self.assertIn("only if", prompt)
        self.assertIn("IMMEDIATE_SCENE", prompt)
        self.assertIn("scene beginning directly after the pause", prompt)
        self.assertIn("NEVER use E0001", prompt)
        self.assertIn("Never use an extended pause as storage", _system)

    def test_completed_checkpoint_reuses_descriptions_without_new_gemini_chunk_calls(self):
        import tempfile
        from pathlib import Path

        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            video = root / "movie.mkv"
            chunk1 = root / "chunk1.mkv"
            chunk2 = root / "chunk2.mkv"
            for path in (video, chunk1, chunk2):
                path.write_bytes(b"x")
            settings = {
                "enable_character_glossary": False,
                "enable_extended_audio_description": False,
                "description_coverage_mode": "standard",
                "intensive_min_silence_seconds": 3.0,
            }
            resumed = [
                {"start_sec": 10.0, "end_sec": 12.0, "text": "Prima."},
                {"start_sec": 200.0, "end_sec": 202.0, "text": "Seconda."},
            ]
            with (
                mock.patch(
                    "audio_describer.core.audio_describer.config_model.get_setting",
                    side_effect=lambda key: settings.get(key),
                ),
                mock.patch(
                    "audio_describer.core.audio_describer.gemini.get_gemini_client",
                    return_value=object(),
                ),
                mock.patch(
                    "audio_describer.core.audio_describer.gemini.validate_model_for_generate_content",
                    return_value="gemini-test",
                ),
                mock.patch(
                    "audio_describer.core.audio_describer._prepare_video_for_gemini"
                ) as prepare_video,
            ):
                descriptions, glossary, usage = generate_descriptions_chunked(
                    str(video),
                    180,
                    prepared_chunks=[
                        {"path": str(chunk1), "start_sec": 0.0, "end_sec": 180.0},
                        {"path": str(chunk2), "start_sec": 180.0, "end_sec": 360.0},
                    ],
                    total_duration_override=360.0,
                    resume_completed_chunks=2,
                    resume_descriptions=resumed,
                )

            self.assertEqual([item[2] for item in descriptions], ["Prima.", "Seconda."])
            self.assertEqual(glossary, [])
            self.assertEqual(usage, [])
            prepare_video.assert_not_called()

if __name__ == "__main__":
    unittest.main()
