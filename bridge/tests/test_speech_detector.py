import hashlib
import unittest
from types import SimpleNamespace
from unittest import mock

import numpy as np

from audio_describer.core import speech_detector
from audio_describer.core.speech_detector import (
    DialogueDetectionError,
    align_descriptions,
    align_descriptions_prioritizing_slots,
    align_descriptions_with_extended_pauses,
    detect_dialogue_intervals,
    intensive_description_slots,
    extended_description_anchors,
    get_bundled_model_path,
    merge_intervals,
    schedule_audio_segments,
    schedule_audio_segments_with_extended_pauses,
    speech_free_intervals,
    uncovered_intensive_slots,
)


class _Segment:
    def __init__(self, duration_seconds):
        self.duration_seconds = duration_seconds


class SpeechDetectorIntervalTests(unittest.TestCase):
    def test_enabled_detection_requires_bundled_onnx_model(self):
        with mock.patch(
            "audio_describer.core.speech_detector.get_bundled_model_path",
            return_value=None,
        ):
            with self.assertRaises(DialogueDetectionError):
                detect_dialogue_intervals("unused.mp4", "unused-token")

    def test_bundled_model_snapshot_is_complete(self):
        model_path = get_bundled_model_path()

        self.assertIsNotNone(model_path)
        normalized = model_path.replace("\\", "/")
        self.assertTrue(normalized.endswith("pyannote-segmentation/model.onnx"))
        with open(model_path, "rb") as model_file:
            self.assertEqual(
                hashlib.sha256(model_file.read()).hexdigest(),
                "6575e57e9375c114545391ffecda0096060df55ae544d40472cdf412d115d35d",
            )

    def test_onnx_session_is_cached(self):
        statuses = []
        fake_session = object()
        with (
            mock.patch.object(speech_detector, "_ONNX_SESSION", None),
            mock.patch.object(speech_detector, "_ONNX_SESSION_PATH", None),
            mock.patch.object(
                speech_detector,
                "get_bundled_model_path",
                return_value="C:\\bundled-pyannote\\model.onnx",
            ),
            mock.patch.dict(
                "sys.modules",
                {"onnxruntime": SimpleNamespace(
                    SessionOptions=lambda: SimpleNamespace(intra_op_num_threads=0),
                    InferenceSession=mock.Mock(return_value=fake_session),
                )},
            ),
        ):
            import sys
            load = sys.modules["onnxruntime"].InferenceSession
            first = speech_detector._get_onnx_session(statuses.append)
            second = speech_detector._get_onnx_session(statuses.append)

        self.assertIs(first, second)
        load.assert_called_once()
        self.assertTrue(any("cached" in status.lower() for status in statuses))

    def test_segmentation_chunks_match_pyannote_padded_tail(self):
        sample_rate = speech_detector.SEGMENTATION_SAMPLE_RATE

        self.assertEqual(
            speech_detector._segmentation_chunk_starts(5 * sample_rate, sample_rate),
            [0],
        )
        self.assertEqual(
            speech_detector._segmentation_chunk_starts(10 * sample_rate, sample_rate),
            [0],
        )
        self.assertEqual(
            speech_detector._segmentation_chunk_starts(
                int(10.5 * sample_rate), sample_rate
            ),
            [0, sample_rate],
        )
        self.assertEqual(
            speech_detector._segmentation_chunk_starts(11 * sample_rate, sample_rate),
            [0, sample_rate],
        )

    def test_short_audio_is_zero_padded_for_onnx(self):
        class _Input:
            name = "waveforms"

        class _Session:
            def __init__(self):
                self.shapes = []

            def get_inputs(self):
                return [_Input()]

            def run(self, _outputs, inputs):
                batch = inputs["waveforms"]
                self.shapes.append(batch.shape)
                scores = np.zeros((len(batch), 589, 7), dtype=np.float32)
                scores[..., 1] = 1.0
                return [scores]

        session = _Session()
        sample_rate = speech_detector.SEGMENTATION_SAMPLE_RATE
        counts = speech_detector._run_onnx_segmentation(
            session, np.ones(5 * sample_rate, dtype=np.float32), sample_rate
        )

        self.assertEqual(session.shapes, [(1, 1, 10 * sample_rate)])
        self.assertEqual(counts.shape, (1, 589))
        self.assertTrue(np.all(counts == 1.0))

    def test_numpy_postprocessing_produces_frame_intervals(self):
        chunks = np.zeros((1, 589), dtype=np.float32)
        chunks[0, 10:20] = 1.0

        counts = speech_detector._aggregate_segmentation_counts(chunks)
        intervals = speech_detector._counts_to_intervals(counts)

        self.assertEqual(len(intervals), 1)
        self.assertAlmostEqual(
            intervals[0][0], 10 * speech_detector.SEGMENTATION_FRAME_STEP_SEC
        )
        self.assertAlmostEqual(
            intervals[0][1],
            19 * speech_detector.SEGMENTATION_FRAME_STEP_SEC
            + speech_detector.SEGMENTATION_FRAME_DURATION_SEC,
        )

    def test_merge_padding_and_clamping(self):
        self.assertEqual(
            merge_intervals([(1, 2), (2.2, 3)], padding_sec=0.2, duration_sec=3),
            [(0.8, 3.0)],
        )

    def test_free_intervals(self):
        self.assertEqual(
            speech_free_intervals([(1, 2), (4, 5)], 6),
            [(0.0, 1.0), (2.0, 4.0), (5.0, 6.0)],
        )

    def test_description_moves_out_of_dialogue(self):
        aligned, dropped = align_descriptions(
            [(1.2, 2.2, "Apre la porta")], [(1, 1.4)], 10
        )
        self.assertEqual(dropped, 0)
        self.assertGreaterEqual(aligned[0][0], 1.4)
        self.assertLessEqual(aligned[0][1], 10.0)

    def test_alignment_uses_spoken_text_duration_not_gemini_window_length(self):
        aligned, dropped = align_descriptions(
            [(10.0, 45.0, "L'aereo scompare tra le nuvole.")],
            [],
            60.0,
        )
        self.assertEqual(dropped, 0)
        self.assertAlmostEqual(aligned[0][0], 10.0)
        self.assertAlmostEqual(aligned[0][1], 13.0)

    def test_description_can_move_up_to_five_seconds(self):
        aligned, dropped = align_descriptions(
            [(823.0, 829.0, "Ernest e Franz entrano nella stanza.")],
            [(822.0, 825.3)],
            900.0,
        )
        self.assertEqual(dropped, 0)
        self.assertAlmostEqual(aligned[0][0], 825.3)

    def test_description_is_not_moved_more_than_five_seconds(self):
        aligned, dropped = align_descriptions(
            [(10.0, 12.0, "Una persona apre la porta.")],
            [(0.0, 15.1)],
            30.0,
        )
        self.assertEqual(aligned, [])
        self.assertEqual(dropped, 1)

    def test_priority_alignment_reserves_mandatory_slot_before_optional_cue(self):
        descriptions = [
            (2.0, 3.0, "uno due tre quattro cinque sei sette otto nove dieci"),
            (5.0, 6.0, "azione visiva importante dentro lo slot obbligatorio ora"),
        ]
        required_slots = [{"id": "S1", "start": 5.0, "end": 10.0}]
        aligned, dropped = align_descriptions_prioritizing_slots(
            descriptions, [], 10.0, required_slots
        )
        self.assertEqual(dropped, 0)
        self.assertEqual(len(aligned), 2)
        mandatory = next(item for item in aligned if "obbligatorio" in item[2])
        self.assertGreaterEqual(mandatory[0], 5.0)
        self.assertLessEqual(mandatory[1], 10.0)

    def test_priority_alignment_keeps_mandatory_when_estimate_exceeds_slot(self):
        descriptions = [
            (5.0, 8.0, "uno due tre quattro cinque sei sette otto nove dieci undici dodici"),
        ]
        required_slots = [{"id": "S1", "start": 5.0, "end": 8.0}]
        aligned, dropped = align_descriptions_prioritizing_slots(
            descriptions, [], 10.0, required_slots
        )
        self.assertEqual(dropped, 0)
        self.assertEqual(len(aligned), 1)
        self.assertGreaterEqual(aligned[0][0], 5.0)
        self.assertLessEqual(aligned[0][1], 8.0)
        self.assertEqual(aligned[0][2], descriptions[0][2])

    def test_priority_alignment_keeps_mandatory_near_visual_timestamp(self):
        descriptions = [
            (19.5, 20.0, "uno due tre quattro cinque sei sette otto nove dieci undici dodici tredici quattordici quindici sedici diciassette diciotto diciannove venti"),
        ]
        required_slots = [{"id": "S1", "start": 5.0, "end": 20.0}]
        aligned, dropped = align_descriptions_prioritizing_slots(
            descriptions, [], 25.0, required_slots
        )
        self.assertEqual(dropped, 0)
        self.assertEqual(len(aligned), 1)
        self.assertGreaterEqual(aligned[0][0], 14.5)
        self.assertLessEqual(aligned[0][1], 20.0)
        self.assertEqual(aligned[0][2], descriptions[0][2])

    def test_description_is_dropped_without_nearby_room(self):
        aligned, dropped = align_descriptions(
            [(10, 11, "Una descrizione molto lunga da pronunciare")],
            [(0, 20)], 20,
        )
        self.assertEqual(aligned, [])
        self.assertEqual(dropped, 1)

    def test_extended_alignment_keeps_long_description_at_one_second_anchor(self):
        descriptions = [(5.0, 6.0, "Bruce raccoglie una piccola collana da terra.")]
        protected = [(0.0, 5.0), (6.0, 20.0)]
        aligned, dropped, extended = align_descriptions_with_extended_pauses(
            descriptions, protected, 20.0
        )
        self.assertEqual(dropped, 0)
        self.assertEqual(extended, 1)
        self.assertEqual(aligned[0][:2], (5.0, 6.0))

    def test_exact_tts_duration_is_rescheduled(self):
        details = [{"start_sec": 1.0, "segment": _Segment(2.0)}]
        scheduled, dropped = schedule_audio_segments(details, [(0.5, 1.2)], 8)
        self.assertEqual(dropped, 0)
        self.assertGreaterEqual(scheduled[0]["start_sec"], 1.2)

    def test_one_second_intensive_candidate_is_kept_only_when_exact_tts_fits(self):
        protected = [(0.0, 5.0), (6.0, 10.0)]
        fitting = [{"start_sec": 5.0, "segment": _Segment(0.8)}]
        too_long = [{"start_sec": 5.0, "segment": _Segment(1.2)}]

        scheduled, dropped = schedule_audio_segments(fitting, protected, 10.0)
        self.assertEqual(len(scheduled), 1)
        self.assertEqual(dropped, 0)

        scheduled, dropped = schedule_audio_segments(too_long, protected, 10.0)
        self.assertEqual(scheduled, [])
        self.assertEqual(dropped, 1)

    def test_extended_tts_plan_uses_pause_when_narration_cannot_fit(self):
        details = [{"start_sec": 5.0, "segment": _Segment(3.0)}]
        normal, pauses, dropped = schedule_audio_segments_with_extended_pauses(
            details, [(0.0, 5.0), (6.0, 10.0)], 10.0
        )
        self.assertEqual(normal, [])
        self.assertEqual(dropped, 0)
        self.assertEqual(pauses[0]["start_sec"], 5.0)
        self.assertTrue(pauses[0]["extended_pause"])

    def test_extended_anchors_include_only_short_speech_free_gaps(self):
        anchors = extended_description_anchors(
            [(1.5, 3.0), (5.0, 9.0)], duration_sec=12.0,
            normal_min_duration_sec=3.0,
        )
        self.assertEqual(
            [(item["start"], item["end"]) for item in anchors],
            [(0.0, 1.5), (3.0, 5.0)],
        )
        self.assertEqual(
            [(item["scene_start"], item["scene_end"]) for item in anchors],
            [(1.5, 5.5), (5.0, 9.0)],
        )
        formatted = speech_detector.format_extended_anchors_for_prompt(anchors)
        self.assertIn("E0001=PAUSE 0.000-1.500 -> IMMEDIATE_SCENE 1.500-5.500", formatted)
        self.assertIn("E0002=PAUSE 3.000-5.000 -> IMMEDIATE_SCENE 5.000-9.000", formatted)

    def test_extended_anchor_at_chunk_tail_is_not_offered_without_following_scene(self):
        anchors = extended_description_anchors(
            [(0.0, 8.0)], duration_sec=10.0,
            normal_min_duration_sec=3.0, range_start=0.0, range_end=10.0,
        )
        self.assertEqual(anchors, [])

    def test_two_point_five_second_slots_keep_existing_word_budget(self):
        slots = intensive_description_slots(
            [(2.49, 3.0), (5.5, 6.0), (9.0, 10.0)],
            duration_sec=10.0, min_duration_sec=2.5,
        )
        self.assertEqual(
            [(slot["start"], slot["end"], slot["max_words"]) for slot in slots],
            [(3.0, 5.5, 5), (6.0, 9.0, 6)],
        )

    def test_intensive_slots_are_clipped_to_chunk_and_have_word_budget(self):
        slots = intensive_description_slots(
            [(4.0, 8.0), (12.0, 14.0)],
            duration_sec=20.0,
            min_duration_sec=3.0,
            range_start=5.0,
            range_end=16.0,
        )
        self.assertEqual(
            [(slot["start"], slot["end"], slot["max_words"]) for slot in slots],
            [(8.0, 12.0, 8)],
        )

    def test_cross_chunk_intensive_slot_has_unique_ids_without_losing_either_part(self):
        first_chunk = intensive_description_slots(
            [(0.0, 8.0), (22.0, 40.0)],
            duration_sec=40.0,
            min_duration_sec=3.0,
            range_start=0.0,
            range_end=15.0,
            id_suffix="C0001",
        )
        second_chunk = intensive_description_slots(
            [(0.0, 8.0), (22.0, 40.0)],
            duration_sec=40.0,
            min_duration_sec=3.0,
            range_start=15.0,
            range_end=30.0,
            id_suffix="C0002",
        )

        self.assertEqual(
            [(slot["id"], slot["start"], slot["end"]) for slot in first_chunk],
            [("S0002C0001", 8.0, 15.0)],
        )
        self.assertEqual(
            [(slot["id"], slot["start"], slot["end"]) for slot in second_chunk],
            [("S0002C0002", 15.0, 22.0)],
        )

    def test_long_silence_is_split_into_mandatory_subslots(self):
        slots = intensive_description_slots(
            [], duration_sec=185.682, min_duration_sec=3.0,
            range_start=0.0, range_end=109.637, id_suffix="C0016",
        )

        self.assertEqual(len(slots), 8)
        self.assertEqual(slots[0]["start"], 0.0)
        self.assertEqual(slots[-1]["end"], 109.637)
        self.assertTrue(all(slot["end"] - slot["start"] <= 15.0 for slot in slots))
        self.assertEqual(len({slot["id"] for slot in slots}), len(slots))
        self.assertTrue(all(slot["partition_count"] == 8 for slot in slots))
        self.assertEqual(slots[0]["partition_index"], 1)
        self.assertEqual(slots[-1]["partition_index"], 8)
        formatted = speech_detector.format_intensive_slots_for_prompt(slots)
        self.assertIn("LONG_SILENCE_PART 1/8; inspect this part independently", formatted)
        self.assertIn("LONG_SILENCE_PART 8/8; inspect this part independently", formatted)
        missing = uncovered_intensive_slots(
            [(1.0, 2.0, "Copre soltanto la prima parte")], slots
        )
        self.assertEqual(len(missing), 7)

    def test_intensive_coverage_reports_only_missing_slots(self):
        slots = [
            {"id": "S0001", "start": 0.0, "end": 4.0, "max_words": 8},
            {"id": "S0002", "start": 8.0, "end": 12.0, "max_words": 8},
        ]
        missing = uncovered_intensive_slots(
            [(1.0, 2.0, "Prima descrizione")], slots
        )
        self.assertEqual([slot["id"] for slot in missing], ["S0002"])


if __name__ == "__main__":
    unittest.main()
