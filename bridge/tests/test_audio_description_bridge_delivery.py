import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
BRIDGE = (ROOT / "src" / "tools" / "audio_description_bridge.rs").read_text(encoding="utf-8")


class AudioDescriptionBridgeDeliveryTests(unittest.TestCase):
    def test_bridge_uses_faster_whisper_style_versioned_cache_without_manifest(self):
        self.assertIn(
            'const BRIDGE_CACHE_FILE_NAME: &str = "audio_description_bridge_v7.exe";',
            BRIDGE,
        )
        self.assertNotIn("audio_description_bridge_v1.exe", BRIDGE)
        self.assertNotIn("bridge-version.txt", BRIDGE)
        self.assertNotIn("audiodescription-version", BRIDGE)

    def test_github_asset_keeps_unversioned_name_and_cache_is_downloaded_only_when_needed(self):
        self.assertIn(
            "Sonarpad-Tools/releases/download/0.7/audio_description_bridge.exe",
            BRIDGE,
        )
        self.assertIn("if cached.exists()", BRIDGE)
        self.assertIn("Ok(true) =>", BRIDGE)
        self.assertIn("return Ok(cached);", BRIDGE)
        self.assertIn("download_bridge(&cached, cancel, download_progress)?;", BRIDGE)


    def test_bridge_cache_lives_in_sonarpad_tools_subfolder_like_faster_whisper(self):
        start = BRIDGE.index("fn bridge_install_path() -> PathBuf")
        end = BRIDGE.index("\n}", start) + 2
        install_fn = BRIDGE[start:end]
        install_fn_compact = "".join(install_fn.split())
        self.assertIn(
            'crate::settings::settings_dir().join("tools").join(BRIDGE_CACHE_FILE_NAME)',
            install_fn_compact,
        )

    def test_debug_build_can_still_use_local_unversioned_bridge(self):
        self.assertIn("#[cfg(debug_assertions)]", BRIDGE)
        self.assertIn(
            'const BRIDGE_DEBUG_FILE_NAME: &str = "audio_description_bridge.exe";',
            BRIDGE,
        )
        self.assertIn("using local debug worker", BRIDGE)

    def test_touched_sources_do_not_use_forbidden_let_underscore_assignments(self):
        sources = [
            ROOT / "src" / "app_windows" / "audio_description_project_window.rs",
            ROOT / "src" / "app_windows" / "audio_description_window.rs",
            ROOT / "src" / "app_windows" / "prompt_window.rs",
            ROOT / "src" / "main.rs",
        ]
        offenders = []
        for source in sources:
            for line_number, line in enumerate(
                source.read_text(encoding="utf-8").splitlines(), start=1
            ):
                if "let _ =" in line:
                    offenders.append(f"{source.relative_to(ROOT)}:{line_number}: {line.strip()}")
        self.assertEqual([], offenders)

    def test_audio_description_sources_do_not_use_forbidden_drop_calls(self):
        sources = [
            ROOT / "src" / "app_windows" / "audio_description_project_window.rs",
            ROOT / "src" / "app_windows" / "audio_description_window.rs",
            ROOT / "src" / "app_windows" / "youtube_transcript_window.rs",
            ROOT / "src" / "audio_description.rs",
            ROOT / "src" / "ffmpeg_export.rs",
            ROOT / "src" / "tools" / "audio_description_bridge.rs",
        ]
        offenders = []
        for source in sources:
            for line_number, line in enumerate(
                source.read_text(encoding="utf-8").splitlines(), start=1
            ):
                stripped = line.strip()
                if "drop(" in stripped and not stripped.startswith("fn drop("):
                    offenders.append(f"{source.relative_to(ROOT)}:{line_number}: {stripped}")
        self.assertEqual([], offenders)


    def test_recent_audio_description_sources_avoid_known_clippy_denied_patterns(self):
        audio_description = (ROOT / "src" / "audio_description.rs").read_text(encoding="utf-8")
        audio_window = (ROOT / "src" / "app_windows" / "audio_description_window.rs").read_text(encoding="utf-8")
        prompt_window = (ROOT / "src" / "app_windows" / "prompt_window.rs").read_text(encoding="utf-8")

        self.assertNotIn(
            "split_inclusive(|character: char| matches!(character, '.' | '!' | '?'))",
            audio_description,
        )
        self.assertIn(".sort_by_key(|catalog| catalog.name.to_lowercase())", audio_window.replace("\n", " "))
        self.assertNotIn(
            "if selected >= 0 {\n                        if crate::with_raw_mut_ptr_safe",
            prompt_window,
        )


if __name__ == "__main__":
    unittest.main()
