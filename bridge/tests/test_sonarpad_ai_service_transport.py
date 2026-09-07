import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SERVICE = (ROOT / 'bridge' / 'audio_description_runtime' / 'audio_describer' / 'core' / 'sonarpad_service.py').read_text(encoding='utf-8')
DESCRIBER = (ROOT / 'bridge' / 'audio_description_runtime' / 'audio_describer' / 'core' / 'audio_describer.py').read_text(encoding='utf-8')
GEMINI = (ROOT / 'bridge' / 'audio_description_runtime' / 'audio_describer' / 'core' / 'gemini_helpers.py').read_text(encoding='utf-8')
BACKEND = (ROOT / 'bridge' / 'audio_description_bridge.py').read_text(encoding='utf-8')
WINDOW = (ROOT / 'src' / 'app_windows' / 'audio_description_window.rs').read_text(encoding='utf-8')
SETTINGS = (ROOT / 'src' / 'settings.rs').read_text(encoding='utf-8')
RUST_AD = (ROOT / 'src' / 'audio_description.rs').read_text(encoding='utf-8')


class SonarpadAiServiceTransportTests(unittest.TestCase):
    def test_media_upload_goes_to_google_temporary_url_not_backend(self):
        self.assertIn('upload_url = str(value.get("upload_url") or "")', SERVICE)
        self.assertIn('urllib.request.Request(upload_url, data=data', SERVICE)
        self.assertNotIn('"media"', SERVICE)
        self.assertNotIn('"file_bytes"', SERVICE)

    def test_service_mode_disables_inline_video_and_inline_fallback(self):
        self.assertIn('not gemini.using_sonarpad_service()', DESCRIBER)
        self.assertIn('def using_sonarpad_service()', GEMINI)
        self.assertIn('if gemini.using_sonarpad_service():', DESCRIBER)
        self.assertIn('file_uri=video_file_obj.uri', DESCRIBER)
        self.assertIn('mime_type=video_file_obj.mime_type', DESCRIBER)

    def test_bridge_accepts_service_credentials_without_personal_gemini_key(self):
        self.assertIn('sonarpad_ai_service_url', BACKEND)
        self.assertIn('sonarpad_ai_access_code', BACKEND)
        self.assertIn('sonarpad_ai_device_id', BACKEND)

    def test_service_requires_https(self):
        self.assertIn('if not self.base_url.startswith("https://")', SERVICE)

    def test_windows_exposes_personal_key_and_sonarpad_service_as_separate_choices(self):
        self.assertIn('ID_AI_PERSONAL', WINDOW)
        self.assertIn('ID_AI_SONARPAD', WINDOW)
        self.assertIn('https://sonarpad.com/sonarpad-ai', WINDOW)
        self.assertIn('https://sonarpad.com/contact.php', WINDOW)

    def test_windows_exposes_sonarpad_code_visibility_and_current_credit(self):
        self.assertIn('ID_SONARPAD_SHOW_CODE', WINDOW)
        self.assertIn('update_sonarpad_code_visibility', WINDOW)
        self.assertIn('ID_SONARPAD_BALANCE', WINDOW)
        self.assertIn('refresh_sonarpad_balance', WINDOW)
        self.assertIn('/v1/account', WINDOW)

    def test_sonarpad_code_is_dpapi_protected_in_settings(self):
        self.assertIn('encrypt_sonarpad_ai_access_code', SETTINGS)
        self.assertIn('dpapi_protect(code.as_bytes())', SETTINGS)
        self.assertIn('decrypt_sonarpad_ai_access_code', SETTINGS)

    def test_personal_key_path_remains_available(self):
        self.assertIn(
            'gemini_api_key: if use_sonarpad_ai { String::new() } else { gemini_api_key }',
            ' '.join(WINDOW.split()),
        )
        self.assertIn('ai_access_mode', RUST_AD)
        self.assertIn('Sonarpad AI mode must not receive a personal Gemini API key', BACKEND)
        self.assertIn('pub gemini_api_key: String', RUST_AD)


if __name__ == '__main__':
    unittest.main()

class SonarpadAiServiceObjectTests(unittest.TestCase):
    def test_verified_upload_matches_google_file_attributes_used_by_audio_describer(self):
        from audio_describer.core.sonarpad_service import SonarpadServiceClient

        client = object.__new__(SonarpadServiceClient)
        client._uploads = {}
        client._request_json = lambda *args, **kwargs: {
            "upload_id": "upl_test",
            "file_name": "files/test123",
            "file_uri": "https://generativelanguage.googleapis.com/v1beta/files/test123",
            "state": "ACTIVE",
        }
        file_obj = client.complete_upload("upl_test", "files/test123", "video/mp4")
        self.assertEqual(file_obj.name, "files/test123")
        self.assertEqual(file_obj.uri, "https://generativelanguage.googleapis.com/v1beta/files/test123")
        self.assertEqual(file_obj.mime_type, "video/mp4")
        self.assertEqual(file_obj.state.name, "ACTIVE")


class SonarpadAiRestPayloadTests(unittest.TestCase):
    def test_sdk_convenience_contents_are_wrapped_as_rest_content(self):
        from audio_describer.core.sonarpad_service import _normalize_contents_for_rest
        value = _normalize_contents_for_rest([
            "Analyze this video",
            {
                "fileData": {
                    "fileUri": "https://generativelanguage.googleapis.com/v1beta/files/test123",
                    "mimeType": "video/x-matroska",
                }
            },
        ])
        self.assertEqual(
            value,
            [{
                "role": "user",
                "parts": [
                    {"text": "Analyze this video"},
                    {
                        "fileData": {
                            "fileUri": "https://generativelanguage.googleapis.com/v1beta/files/test123",
                            "mimeType": "video/x-matroska",
                        }
                    },
                ],
            }],
        )

    def test_structured_http_400_is_not_retryable(self):
        from audio_describer.core.gemini_helpers import is_retryable_transient_error
        from audio_describer.core.sonarpad_service import SonarpadServiceError
        exc = SonarpadServiceError("HTTP 400 Sonarpad AI request failed: Invalid value", 400)
        self.assertFalse(is_retryable_transient_error(exc))
