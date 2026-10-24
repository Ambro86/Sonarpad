import sys
import tempfile
import unittest
from pathlib import Path
from contextlib import ExitStack
from unittest import mock

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "audio_description_runtime"))
from audio_describer.core import audio_describer as ad


class NarrationRestartTests(unittest.TestCase):
    def test_threshold_and_short_pauses(self):
        self.assertEqual(ad._narration_restart_ranges([(10,55)], 180, 0, 180), [(0,55),(55,180)])
        self.assertIsNone(ad._narration_restart_ranges([(10,54.99)], 180, 0, 180))
        self.assertEqual(ad._narration_restart_ranges([(10,30),(31,50),(52,65)],180,0,180), [(0,65),(65,180)])

    def test_normal_chunks_and_existing_boundaries_unchanged(self):
        for intervals, start, end in [([],0,180), ([(0,180)],0,180),
                                      ([(10,30),(34,65)],0,180),
                                      ([(10,55)],55,180), ([(10,55)],40,180),
                                      ([(10,55)],0,57)]:
            self.assertIsNone(ad._narration_restart_ranges(intervals,180,start,end))

    def test_existing_clipping_path_uses_original_file_and_exact_offsets(self):
        settings = {"application_language":"it", "enable_character_glossary":False}
        with ExitStack() as stack:
            stack.enter_context(mock.patch.object(ad.config_model,"get_setting",side_effect=settings.get))
            prepare = stack.enter_context(mock.patch.object(ad,"_prepare_video_for_gemini", return_value=("part",None)))
            stack.enter_context(mock.patch.object(ad.gemini,"generate_content_with_retry", return_value=object()))
            stack.enter_context(mock.patch.object(ad.gemini,"process_gemini_response",return_value=("{}",True)))
            stack.enter_context(mock.patch.object(ad,"_parse_with_optional_json_repair",
                side_effect=[([("00:01","00:02","Prima.")],[]),([("00:01","00:02","Dopo.")],[])]))
            stack.enter_context(mock.patch.object(ad,"_correct_description_language",side_effect=lambda c,m,d,s:(d,None)))
            stack.enter_context(mock.patch.object(ad,"_correct_glossary_language",side_effect=lambda c,m,d,s:(d,None)))
            for name in ("build_generation_config","save_raw_ai_output","log_token_usage"):
                stack.enter_context(mock.patch.object(ad.gemini,name,return_value=None))
            statuses = []
            descriptions, _, _ = ad._generate_blocked_chunk_by_minutes(
                client=object(),model_name="test",video_path="movie.mp4",
                prepared_chunk_path="existing.mkv",chunk_start=390,chunk_end=570,
                chunk_number=3,total_chunks=8,clipped_windows=[],intensive_slots=[],
                extended_anchors=[],intensive_mode=False,extended_mode=False,
                user_prompt="",character_continuity={},prior_descriptions=[],
                status_update_callback=statuses.append,
                clip_ranges=[(390,484.7256875),(484.7256875,570)])
            self.assertEqual(descriptions, [(391,392,"Prima."),(485.7256875,486.7256875,"Dopo.")])
            self.assertEqual(prepare.call_count,2)
            self.assertEqual(prepare.call_args_list[0].args[1],"existing.mkv")
            self.assertEqual(prepare.call_args_list[1].args[1],"existing.mkv")
            self.assertEqual(prepare.call_args_list[0].kwargs["start_offset_sec"],0)
            self.assertAlmostEqual(prepare.call_args_list[0].kwargs["end_offset_sec"],94.7256875)
            self.assertAlmostEqual(prepare.call_args_list[1].kwargs["start_offset_sec"],94.7256875)
            self.assertEqual(prepare.call_args_list[1].kwargs["end_offset_sec"],180)
            self.assertFalse(any("blocked" in s for s in statuses))

    def test_blocked_custom_segment_uses_minutes_without_repeating_neighbors(self):
        settings = {"application_language": "it", "enable_character_glossary": False}
        original_ranges = [(390, 415), (415, 550), (550, 570)]
        blocked = ad.ContentBlockedError("blocked", reason="PROHIBITED_CONTENT")
        with ExitStack() as stack:
            stack.enter_context(mock.patch.object(ad.config_model, "get_setting", side_effect=settings.get))
            prepare = stack.enter_context(mock.patch.object(ad, "_prepare_video_for_gemini", return_value=("part", None)))
            generate = stack.enter_context(mock.patch.object(ad.gemini, "generate_content_with_retry",
                side_effect=[object(), blocked, object(), blocked, object(), object()]))
            stack.enter_context(mock.patch.object(ad.gemini, "process_gemini_response", return_value=("{}", True)))
            stack.enter_context(mock.patch.object(ad, "_parse_with_optional_json_repair",
                return_value=([("00:01", "00:02", "Scena.")], [])))
            stack.enter_context(mock.patch.object(ad, "_correct_description_language", side_effect=lambda c,m,d,s:(d,None)))
            stack.enter_context(mock.patch.object(ad, "_correct_glossary_language", side_effect=lambda c,m,d,s:(d,None)))
            for name in ("build_generation_config", "save_raw_ai_output", "log_token_usage"):
                stack.enter_context(mock.patch.object(ad.gemini, name, return_value=None))
            descriptions, _, _ = ad._generate_blocked_chunk_by_minutes(
                client=object(), model_name="test", video_path="movie.mp4",
                prepared_chunk_path="existing.mkv", chunk_start=390, chunk_end=570,
                chunk_number=3, total_chunks=8, clipped_windows=[], intensive_slots=[],
                extended_anchors=[], intensive_mode=False, extended_mode=False,
                user_prompt="", character_continuity={}, prior_descriptions=[],
                status_update_callback=lambda _: None, clip_ranges=original_ranges)
            self.assertEqual([(c.kwargs["start_offset_sec"], c.kwargs["end_offset_sec"])
                              for c in prepare.call_args_list],
                             [(0,25),(25,160),(25,85),(85,145),(145,160),(160,180)])
            self.assertEqual([d[0] for d in descriptions], [391,416,536,551])
            self.assertEqual(generate.call_count, 6)
            self.assertEqual(original_ranges, [(390,415),(415,550),(550,570)])
            self.assertTrue(all(c.args[1] == "existing.mkv" for c in prepare.call_args_list))

    def test_generation_reuses_existing_path_and_preserves_checkpoint_layout(self):
        settings = {"enable_character_glossary":False,
                    "enable_extended_audio_description":False,
                    "description_coverage_mode":"standard",
                    "intensive_min_silence_seconds":3.0}
        with tempfile.TemporaryDirectory() as directory, ExitStack() as stack:
            video = Path(directory)/"original.mkv"
            video.write_bytes(b"x")
            stack.enter_context(mock.patch.object(ad.config_model,"get_setting",side_effect=settings.get))
            stack.enter_context(mock.patch.object(ad.gemini,"get_gemini_client",return_value=object()))
            stack.enter_context(mock.patch.object(ad.gemini,"validate_model_for_generate_content",return_value="test"))
            stack.enter_context(mock.patch.object(ad.gemini,"build_generation_config",return_value=None))
            prepare = stack.enter_context(mock.patch.object(ad,"_prepare_video_for_gemini"))
            fallback = stack.enter_context(mock.patch.object(ad,"_generate_blocked_chunk_by_minutes",
                return_value=([(56,58,"Una strada.")],[],[])))
            checkpoint = mock.Mock()
            ad.generate_descriptions_chunked(
                str(video),180,dialogue_intervals=[(10,55)],
                prepared_chunks=[{"path":str(video),"start_sec":0,"end_sec":180}],
                total_duration_override=180,checkpoint_callback=checkpoint)
            prepare.assert_not_called()
            fallback.assert_called_once()
            self.assertEqual(fallback.call_args.kwargs["clip_ranges"],[(0,55),(55,180)])
            self.assertEqual(checkpoint.call_args.args[:2],(1,1))
