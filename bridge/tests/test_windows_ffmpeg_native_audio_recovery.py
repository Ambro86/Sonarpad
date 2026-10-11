"""Regression checks: severely damaged AVI audio never needs ffmpeg.exe.

These are source-level checks, not a replacement for a Windows integration run.
"""
from pathlib import Path
import unittest

ROOT = Path(__file__).resolve().parents[2]
DESC = (ROOT / "src/audio_description.rs").read_text(encoding="utf-8")
SOURCE = (ROOT / "src/ffmpeg_source.rs").read_text(encoding="utf-8")


class FfmpegNativeRecoveryTests(unittest.TestCase):
    def test_corruption_guard_has_cumulative_budget(self):
        self.assertIn("bad_packets_this_refill", SOURCE)
        self.assertIn("bad_packets_since_seek", SOURCE)
        self.assertIn("ffmpeg_corruption_budget_exceeded(", SOURCE)

    def test_normal_decoder_runs_before_recovery(self):
        self.assertLess(
            DESC.index("match write_pyannote_wav(source_path"),
            DESC.index("recover_pyannote_wav_with_ffmpeg_libraries(", DESC.index("match write_pyannote_wav(source_path")),
        )

    def test_recovery_uses_bundled_dlls_only(self):
        begin = DESC.index("fn recover_pyannote_wav_with_ffmpeg_libraries(")
        end = DESC.index("fn normalize_audio_description_source_duration(", begin)
        recovery = DESC[begin:end]
        self.assertIn("FfmpegSource::try_new_with_forced_channels(", recovery)
        self.assertIn("source.try_seek(Duration::from_secs_f64(next_sec))", recovery)
        self.assertIn("hound::WavWriter::create(output_path, spec)", recovery)
        self.assertNotIn("Command::new", recovery)
        self.assertNotIn("ffmpeg.exe", recovery)
        self.assertNotIn("locate_audio_description_ffmpeg_executable", recovery)

    def test_bounded_gaps_do_not_fabricate_long_audio(self):
        begin = DESC.index("fn recover_pyannote_wav_with_ffmpeg_libraries(")
        end = DESC.index("fn normalize_audio_description_source_duration(", begin)
        recovery = DESC[begin:end]
        self.assertIn("source_duration_sec * 0.02", recovery)
        self.assertIn("resync_count > 45", recovery)
        self.assertIn("skipped_seconds > max_skipped_seconds", recovery)
        self.assertIn("decoded_seconds < source_duration_sec * 0.8", recovery)
        self.assertIn("cancel.load(Ordering::Relaxed)", recovery)


if __name__ == "__main__":
    unittest.main()
