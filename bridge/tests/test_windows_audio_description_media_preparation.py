from __future__ import annotations

import unittest
from pathlib import Path


class WindowsAudioDescriptionMediaPreparationTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.root = Path(__file__).resolve().parents[2]
        cls.audio_source = (cls.root / "src" / "audio_description.rs").read_text(
            encoding="utf-8"
        )
        cls.ffmpeg_source = (cls.root / "src" / "ffmpeg_export.rs").read_text(
            encoding="utf-8"
        )

    def test_gemini_chunks_adapt_to_inline_safe_size(self):
        self.assertIn(
            "const GEMINI_INLINE_TARGET_CHUNK_BYTES: u64 = 40 * 1024 * 1024;",
            self.audio_source,
        )
        self.assertIn("GEMINI_SEGMENT_RETRY_LIMIT", self.audio_source)
        self.assertIn("adaptive Gemini chunk retry", self.audio_source)
        self.assertIn("max_chunk_bytes <= GEMINI_INLINE_TARGET_CHUNK_BYTES", self.audio_source)
        self.assertIn("file_size >= GEMINI_MAX_CHUNK_BYTES", self.audio_source)

    def test_large_source_is_split_instead_of_rejected_wholesale(self):
        self.assertIn("if input_size == 0 {", self.audio_source)
        self.assertNotIn(
            "if input_size == 0 || input_size >= GEMINI_MAX_CHUNK_BYTES {",
            self.audio_source,
        )

    def test_native_ffmpeg_repairs_missing_avi_and_mkv_timestamps(self):
        self.assertIn('"fflags", "+genpts+discardcorrupt"', self.ffmpeg_source)
        self.assertIn("struct SegmentTimestampState", self.ffmpeg_source)
        self.assertIn("fn repair_segment_packet_timestamps", self.ffmpeg_source)
        self.assertIn("pts_missing", self.ffmpeg_source)
        self.assertIn("dts_missing", self.ffmpeg_source)
        self.assertIn("state.last_dts", self.ffmpeg_source)
        self.assertIn("state.next_dts", self.ffmpeg_source)
        self.assertIn("(*packet).dts <= last_dts", self.ffmpeg_source)
        self.assertIn("(*packet).pts < (*packet).dts", self.ffmpeg_source)
        self.assertIn("repaired_timestamp_packets", self.ffmpeg_source)
        segment_inner = self.ffmpeg_source.index("fn segment_media_file_inner")
        rescale_pos = self.ffmpeg_source.index(
            "av_packet_rescale_ts_safe", segment_inner
        )
        repair_pos = self.ffmpeg_source.index(
            "repair_segment_packet_timestamps(", rescale_pos
        )
        self.assertLess(rescale_pos, repair_pos)

    def test_audio_description_segmentation_tolerates_bounded_invalid_mkv_packets(self):
        self.assertIn(
            "segment_media_file_for_analysis(",
            self.audio_source,
        )
        self.assertIn(
            "pub(crate) fn segment_media_file_for_analysis(",
            self.ffmpeg_source,
        )
        self.assertIn("tolerate_invalid_analysis_packets", self.ffmpeg_source)
        self.assertIn("write_ret == -libc::EINVAL", self.ffmpeg_source)
        self.assertIn("skipped_invalid_analysis_packets < 8", self.ffmpeg_source)
        self.assertIn(
            "skipping invalid analysis packet",
            self.ffmpeg_source,
        )

    def test_segment_muxer_errors_are_not_silently_ignored(self):
        self.assertIn("segment_write_error = Some(format!", self.ffmpeg_source)
        self.assertIn("if let Some(error) = segment_write_error", self.ffmpeg_source)
        self.assertIn("failed to finalize segmented output", self.ffmpeg_source)

    def test_gemini_header_failure_uses_video_only_fallback_without_changing_silence_logic(self):
        self.assertIn(
            "pub(crate) fn segment_media_file_for_analysis_video_only(",
            self.ffmpeg_source,
        )
        self.assertIn("video_only: true", self.ffmpeg_source)
        self.assertIn(
            'primary_error.starts_with("FFmpeg: failed to write segment header:")',
            self.audio_source,
        )
        self.assertIn(
            "retrying video-only analysis chunks",
            self.audio_source,
        )
        self.assertIn(
            "dialogue/silence analysis remains unchanged",
            self.audio_source,
        )
        self.assertIn(
            "segment_media_file_for_analysis_video_only(",
            self.audio_source,
        )
        self.assertIn("ffmpeg_error_text(api, header_ret)", self.ffmpeg_source)

    def test_changed_chunk_layout_does_not_make_resume_fatal(self):
        self.assertIn("ignoring resume checkpoint after chunk layout change", self.audio_source)
        self.assertIn("ignoring invalid resume checkpoint", self.audio_source)


if __name__ == "__main__":
    unittest.main()
