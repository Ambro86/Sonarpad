"""Regression checks: optional non-verbal vocals never weaken the default VAD."""

import json
import unittest
import numpy as np

from audio_describer.core import audio_describer, speech_detector


class OptionalNonverbalVoiceTests(unittest.TestCase):
    def test_low_confidence_frames_are_candidates_not_automatic_silence(self):
        counts = np.zeros(300, dtype=np.uint8)
        probs = np.zeros(300, dtype=np.float32)
        counts[20:135] = 1
        probs[20:135] = 0.68  # could be a yawn OR soft intelligible speech
        counts[150:270] = 1
        probs[150:270] = 0.98  # confident speech
        candidates = speech_detector.candidate_nonverbal_voice_intervals(
            counts, probs, 5.0,
        )
        self.assertEqual(len(candidates), 1)
        self.assertEqual(candidates[0]["id"], "NV-%010d" % round(
            20 * speech_detector.SEGMENTATION_FRAME_STEP_SEC * 1000
        ))
        self.assertLess(candidates[0]["p50"], 0.75)
        self.assertLess(candidates[0]["p90"], 0.80)

    def test_unverified_and_unknown_ids_keep_existing_dialogue_protection(self):
        original = [(0.5, 2.5), (4.0, 7.0)]
        candidates = [{"id": "NV-001", "start": 1.0, "end": 2.0}]
        self.assertEqual(
            speech_detector.protected_intervals_after_verified_nonverbal(
                original, candidates, [], 8.0
            ), original,
        )
        self.assertEqual(
            speech_detector.protected_intervals_after_verified_nonverbal(
                original, candidates, ["unknown"], 8.0
            ), original,
        )

    def test_verified_id_only_opens_its_precise_window(self):
        original = [(0.5, 2.5), (4.0, 7.0)]
        candidates = [{"id": "NV-001", "start": 1.0, "end": 2.0}]
        result = speech_detector.protected_intervals_after_verified_nonverbal(
            original, candidates, ["NV-001"], 8.0
        )
        self.assertEqual(result, [(0.5, 1.05), (1.95, 2.5), (4.0, 7.0)])

    def test_optional_verifications_do_not_break_json_parser(self):
        response = json.dumps({
            "character_glossary": [], "audio_descriptions": [],
            "verified_nonverbal_ids": ["NV-001", 23],
        })
        _, _, valid = audio_describer._parse_unified_response(response, None)
        self.assertTrue(valid)
        self.assertEqual(audio_describer._LAST_PARSED_VERIFIED_NONVERBAL_IDS,
                         ["NV-001"])
        _, _, valid = audio_describer._parse_unified_response(
            '{"character_glossary":[],"audio_descriptions":[]}', None
        )
        self.assertTrue(valid)
        self.assertEqual(audio_describer._LAST_PARSED_VERIFIED_NONVERBAL_IDS, [])


if __name__ == "__main__":
    unittest.main()
