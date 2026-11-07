from __future__ import annotations
import tempfile
import unittest
from pathlib import Path
from unittest import mock
import audio_description_bridge as bridge

class DialogueOverlapTests(unittest.TestCase):
    def _run(self, option):
        with tempfile.TemporaryDirectory() as tmp:
            media = Path(tmp) / 'source.mp4'
            media.write_bytes(b'test')
            request = dict(input_path=str(media), audio_wav_path=str(media), duration_sec=20.0,
                           chunks=[dict(path=str(media), start_sec=0.0, end_sec=20.0)],
                           language='it', gemini_api_key='test', allow_extended_pauses=False)
            if option is not None:
                request['allow_dialogue_overlap'] = option
            with mock.patch.object(bridge, '_configure_omni'), mock.patch.object(bridge, '_status'), \
                 mock.patch.object(bridge.speech_detector, 'detect_dialogue_intervals_from_wav', return_value=[(0.0, 20.0)]), \
                 mock.patch.object(bridge.audio_describer, 'generate_descriptions_chunked', return_value=([(2.0, 4.0, 'Il lottatore solleva il rivale.')], [], [])) as generate, \
                 mock.patch.object(bridge.audio_describer, '_remove_consecutive_duplicates', side_effect=lambda rows, cb: rows):
                result = bridge.run(request)
            return result, generate.call_args.kwargs

    def test_missing_and_disabled_option_keep_dialogue_protection(self):
        original, args = self._run(None)
        disabled, disabled_args = self._run(False)
        self.assertEqual(original['protected_intervals'], [dict(start_sec=0.0, end_sec=20.0)])
        self.assertEqual(original['descriptions'], disabled['descriptions'])
        self.assertEqual(original['protected_intervals'], disabled['protected_intervals'])
        self.assertEqual(args['dialogue_intervals'], disabled_args['dialogue_intervals'])
        self.assertEqual(original['descriptions'], [])
        self.assertFalse(original['allow_dialogue_overlap'])

    def test_enabled_option_uses_spoken_intervals_for_generation_and_scheduling(self):
        result, args = self._run(True)
        self.assertEqual(args['dialogue_intervals'], [])
        self.assertEqual(result['protected_intervals'], [])
        self.assertTrue(result['descriptions'])
        self.assertTrue(result['allow_dialogue_overlap'])

    def test_string_true_does_not_enable_the_option(self):
        result, args = self._run('true')
        self.assertFalse(result['allow_dialogue_overlap'])
        self.assertEqual(args['dialogue_intervals'], [(0.0, 20.0)])

    def test_prompt_override_exists_only_when_explicitly_enabled(self):
        original = bridge.config_model.load_settings()
        try:
            prompts = []
            for value in (None, False, True):
                bridge.config_model.configure({'allow_dialogue_overlap': value})
                prompts.append(bridge.audio_describer._build_unified_prompts('', 'gemini-test'))
            self.assertEqual(prompts[0], prompts[1])
            self.assertNotIn('EXPLICIT USER OPTION', prompts[0][0])
            self.assertIn('EXPLICIT USER OPTION', prompts[2][0])
            self.assertIn('spacing between descriptions', prompts[2][1])
        finally:
            bridge.config_model.configure(original)
