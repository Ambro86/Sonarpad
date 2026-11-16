import tempfile
import unittest
from pathlib import Path
from types import SimpleNamespace
from unittest.mock import Mock, patch

import audio_description_bridge as bridge
from audio_describer.core import audio_describer, tmdb_references as refs


def candidate(animation=False):
    return {"id": "tmdb_1", "name": "Anna", "description": "Cast candidate",
            "provisional": True,
            "tmdb": {"actor_name": "An actor", "person_id": 123, "animation": animation,
                     "profile_image_url": "https://image.tmdb.org/t/p/w185/photo.jpg"}}


class TmdbReferencesTests(unittest.TestCase):
    def test_old_catalog_and_metadata_roundtrip(self):
        old = {"id": "a", "name": "Anna", "description": "Black hair"}
        self.assertEqual(bridge._normalise_initial_character_glossary([old]), [old])
        self.assertEqual(bridge._normalise_initial_character_glossary([candidate()]), [candidate()])

    def test_full_existing_catalog_does_not_hide_reference_images(self):
        old = [{"id": str(index), "name": f"Person {index}", "description": "Visual description"}
               for index in range(100)]
        result = bridge._normalise_initial_character_glossary(old + [candidate()])
        self.assertEqual(len(result), 97)
        self.assertEqual(result[-1]["tmdb"]["person_id"], 123)

    def test_animation_never_downloads_voice_actor(self):
        with patch.object(refs, "download_image") as download:
            parts, uploaded = refs.prepare(Mock(), [candidate(True)])
        download.assert_not_called()
        self.assertEqual(uploaded, [])
        self.assertIn("NOT verified identification", parts[0])

    def test_actual_image_part_and_video_fallback_enrichment(self):
        from google.genai import types
        client = Mock()
        client.files.upload.return_value = SimpleNamespace(uri="https://example.test/reference", name="files/reference")
        with patch.object(refs, "download_image", return_value=Path("portrait.jpg")):
            parts, uploaded = refs.prepare(client, [candidate()])
        self.assertEqual(len(uploaded), 1)
        self.assertEqual(parts[-1].file_data.mime_type, "image/jpeg")
        client._sonarpad_character_references = parts
        content = ["Analyze", types.Part.from_uri(file_uri="https://example.test/video", mime_type="video/mp4")]
        self.assertEqual(refs.with_references(client, content), content + parts)
        inline = ["Analyze", types.Part.from_bytes(data=b"video bytes", mime_type="video/mp4")]
        self.assertEqual(refs.with_references(client, inline), inline + parts)
        self.assertEqual(refs.with_references(client, ["Text-only repair"]), ["Text-only repair"])

    def test_unavailable_images_keep_textual_candidates(self):
        with patch.object(refs, "download_image", side_effect=TimeoutError):
            parts, uploaded = refs.prepare(Mock(), [candidate()])
        self.assertEqual(len(parts), 1)
        self.assertEqual(uploaded, [])
        self.assertIn("Anna", parts[0])

    def test_all_cast_photos_without_duplicates(self):
        client = Mock()
        client.files.upload.return_value = SimpleNamespace(uri="https://example.test/reference")
        rows = []
        for index in range(96):
            row = candidate()
            row["tmdb"]["profile_image_url"] = f"https://image.tmdb.org/t/p/w185/{index//2}.jpg"
            rows.append(row)
        with patch.object(refs, "download_image", return_value=Path("portrait.jpg")) as download:
            parts, uploaded = refs.prepare(client, rows)
        self.assertEqual(download.call_count, 48)
        self.assertEqual(len(uploaded), 48)
        self.assertEqual(len(parts), 97)

    def test_rejects_arbitrary_hosts_and_oversized_downloads(self):
        for url in ["http://image.tmdb.org/t/p/w185/a.jpg", "https://evil.test/t/p/w185/a.jpg",
                    "https://image.tmdb.org.evil.test/t/p/w185/a.jpg", "file:///private.jpg",
                    "https://image.tmdb.org/t/p/w185/../a.jpg"]:
            self.assertFalse(refs.valid_image_url(url))
        response = Mock()
        response.__enter__ = Mock(return_value=response)
        response.__exit__ = Mock(return_value=False)
        response.geturl.return_value = candidate()["tmdb"]["profile_image_url"]
        response.read.return_value = b"\xff\xd8\xff" + b"x" * refs.MAX_IMAGE_BYTES
        with tempfile.TemporaryDirectory() as cache, patch.object(refs.urllib.request, "urlopen", return_value=response):
            with self.assertRaises(ValueError):
                refs.download_image(response.geturl(), cache)

    def test_provisional_identity_can_be_corrected(self):
        known = {}
        audio_describer._update_character_continuity(known, [{"id": "a", "name": "Anna", "description": "Red hair"}], provisional=True)
        audio_describer._update_character_continuity(known, [{"id": "a", "name": "Maria", "description": "Black hair"}], provisional=True)
        value = list(known.values())[0]
        self.assertEqual(value["name"], "Maria")
        self.assertEqual(value["description"], "Black hair")
        self.assertTrue(value["provisional"])

    def test_user_catalog_identity_remains_authoritative(self):
        known = {}
        audio_describer._update_character_continuity(known, [{"id": "a", "name": "Anna", "description": "Black hair"}])
        audio_describer._update_character_continuity(known, [{"id": "a", "name": "Maria", "description": "New outfit"}], provisional=True)
        value = list(known.values())[0]
        self.assertEqual(value["name"], "Anna")
        self.assertFalse(value.get("provisional", False))

    def test_visual_identity_exception_is_opt_in_and_reaches_system_prompt(self):
        from audio_describer.models import config_model
        with patch.object(config_model, "get_setting", side_effect=lambda key: {
            "enable_character_glossary": True, "application_language": "it",
        }.get(key)):
            standard, _ = audio_describer._build_unified_prompts("", "model")
            enriched, _ = audio_describer._build_unified_prompts("", "model", tmdb_reference_context=True)
        self.assertNotIn("TMDB EXCEPTION", standard)
        self.assertIn("TMDB EXCEPTION", enriched)
        self.assertIn("weak resemblance alone is never sufficient", enriched)
        self.assertNotIn("ACTIVE CAST MATCHING", standard)
        self.assertIn("FIRST visible appearance", enriched)
        self.assertIn("The TMDB cast may be incomplete", enriched)
        self.assertIn("never assign the closest-looking available character", enriched)
        self.assertIn("otherwise use a neutral identifying visual description", enriched)
        self.assertIn("if the face is unclear or the match is uncertain, retain a neutral description", enriched)

    def test_tmdb_name_localization_follows_requested_language(self):
        from audio_describer.models import config_model
        for code, name in [("it", "Italian"), ("fr", "French"), ("de", "German")]:
            with self.subTest(code=code), patch.object(config_model, "get_setting", side_effect=lambda key: {
                "enable_character_glossary": True, "application_language": code,
            }.get(key)):
                standard, _ = audio_describer._build_unified_prompts("", "model")
                system, prompt = audio_describer._build_unified_prompts("", "model", tmdb_reference_context=True)
            self.assertNotIn("CHARACTER NAME LOCALIZATION", standard)
            self.assertIn("CHARACTER NAME LOCALIZATION", system)
            self.assertIn("Target language: " + name, system)
            self.assertIn("explicitly user-confirmed names", system)
            self.assertIn("IDs and JSON keys remain unchanged", prompt)
