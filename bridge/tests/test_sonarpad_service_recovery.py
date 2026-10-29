"""Regression tests for Sonarpad AI polling / cleanup (no real network calls)."""
from __future__ import annotations

import unittest
from unittest.mock import patch

from audio_describer.core.sonarpad_service import (
    SonarpadServiceClient,
    SonarpadServiceError,
    _FilesApi,
    _UploadRecord,
)


class ServiceRecoveryTests(unittest.TestCase):
    def make_client(self):
        client = object.__new__(SonarpadServiceClient)
        client.session_token = 'sst_test'
        client._uploads = {'files/demo': _UploadRecord('upl_demo', 'files/demo', 'video/mp4')}
        return client

    def test_generate_polls_pending_without_changing_key_or_body(self):
        client = self.make_client()
        calls = []

        def fake_request(method, path, payload, **kwargs):
            calls.append((method, path, payload, kwargs))
            if len(calls) < 4:
                raise SonarpadServiceError('HTTP 503 Sonarpad AI request failed: generation_in_progress', 503, retry_after=7)
            return {'candidates': [{'content': {'parts': [{'text': 'ok'}]}}]}

        client._request_json = fake_request
        with patch('audio_describer.core.sonarpad_service.time.sleep') as sleep:
            result = client.generate('prompt', None)
        self.assertTrue(result.candidates)
        self.assertEqual(4, len(calls))
        self.assertEqual([7, 7, 7], [c.args[0] for c in sleep.call_args_list])
        self.assertEqual(1, len({c[3]['headers']['X-Idempotency-Key'] for c in calls}))
        self.assertEqual(calls[0][2], calls[-1][2])

    def test_network_then_success_reuses_same_key(self):
        client = self.make_client()
        keys = []

        def fake_request(_method, _path, _body, **kwargs):
            keys.append(kwargs['headers']['X-Idempotency-Key'])
            if len(keys) <= 2:
                raise SonarpadServiceError('Sonarpad AI network error: timed out', 503)
            return {'candidates': []}

        client._request_json = fake_request
        with patch('audio_describer.core.sonarpad_service.time.sleep'):
            client.generate('prompt', None)
        self.assertEqual(3, len(keys))
        self.assertEqual(1, len(set(keys)))

    def test_generate_renews_same_device_session_once_preserving_key(self):
        client = self.make_client()
        keys = []
        activations = []
        client._activate = lambda: activations.append(True)

        def fake_request(*args, **kwargs):
            keys.append(kwargs['headers']['X-Idempotency-Key'])
            if len(keys) == 1:
                raise SonarpadServiceError('HTTP 401 Sonarpad AI authentication failed: invalid_session', 401)
            return {'candidates': []}

        client._request_json = fake_request
        client.generate('prompt', None)
        self.assertEqual(1, len(activations))
        self.assertEqual(2, len(keys))
        self.assertEqual(keys[0], keys[1])

    def test_generate_does_not_repeat_invalid_session_forever(self):
        client = self.make_client()
        activations = []
        client._activate = lambda: activations.append(True)
        client._request_json = lambda *args, **kwargs: (_ for _ in ()).throw(
            SonarpadServiceError('HTTP 401 Sonarpad AI authentication failed: invalid_session', 401))
        with self.assertRaises(SonarpadServiceError):
            client.generate('prompt', None)
        self.assertEqual(1, len(activations))

    def test_permanent_400_no_retry(self):
        client = self.make_client()
        calls = []

        def fake_request(*args, **kwargs):
            calls.append(args)
            raise SonarpadServiceError('HTTP 400 INVALID_ARGUMENT', 400)

        client._request_json = fake_request
        with self.assertRaises(SonarpadServiceError):
            client.generate('prompt', None)
        self.assertEqual(1, len(calls))

    def test_cleanup_refreshes_stale_session_once(self):
        client = self.make_client()
        actions = []

        def fake_request(*args, **kwargs):
            actions.append(('delete', args[1]))
            if actions.count(('delete', '/v1/upload/delete')) == 1:
                raise SonarpadServiceError('HTTP 401 Sonarpad AI authentication failed: invalid_session', 401)
            return {}

        client._request_json = fake_request
        client._activate = lambda: actions.append(('activate', None))
        _FilesApi(client).delete('files/demo')
        self.assertEqual([('delete', '/v1/upload/delete'), ('activate', None), ('delete', '/v1/upload/delete')], actions)
        self.assertNotIn('files/demo', client._uploads)

    def test_invalid_session_on_other_error_not_refreshed(self):
        client = self.make_client()
        client._request_json = lambda *a, **k: (_ for _ in ()).throw(
            SonarpadServiceError('HTTP 403 access denied', 403))
        activations = []
        client._activate = lambda: activations.append(True)
        with self.assertRaises(SonarpadServiceError):
            _FilesApi(client).delete('files/demo')
        self.assertFalse(activations)
        self.assertIn('files/demo', client._uploads)

    def test_html_404_classification_is_limited_to_our_api(self):
        import io
        import urllib.error
        from audio_describer.core.gemini_helpers import is_retryable_transient_error
        client = self.make_client()
        client.base_url = 'https://example.test/sonarpad-ai'
        cases = [
            ('/v1/upload/start', 404, '<!DOCTYPE HTML><html>Not Found</html>', True),
            ('/v1/generate', 404, '  <html>Not Found</html>', True),
            ('/v1/generate', 404, '{"error":"model_not_found"}', False),
            ('/v1/upload/complete', 404, '{"error":"file_not_found"}', False),
            ('/v1/generate', 404, 'Not Found', False),
            ('/v1/generate', 403, '<html>Forbidden</html>', False),
            ('https://other.test/v1/generate', 404, '<html>Not Found</html>', False),
        ]
        for path, status, body, expected in cases:
            with self.subTest(path=path, status=status, body=body):
                error = urllib.error.HTTPError(path, status, 'failure', {}, io.BytesIO(body.encode()))
                with patch('urllib.request.urlopen', side_effect=error):
                    with self.assertRaises(SonarpadServiceError) as caught:
                        client._request_json('POST', path, {})
                self.assertEqual(expected, caught.exception.temporary_html_not_found)
                self.assertEqual(expected, is_retryable_transient_error(caught.exception))

    def test_html_404_generation_keeps_key_after_more_than_six_retries(self):
        import io
        import urllib.error
        client = self.make_client()
        client.base_url = 'https://example.test/sonarpad-ai'
        calls = []

        def open_request(request, **kwargs):
            calls.append(request)
            if len(calls) <= 9:
                raise urllib.error.HTTPError(request.full_url, 404, 'Not Found', {},
                                             io.BytesIO(b'<html>Not Found</html>'))
            return io.BytesIO(b'{"candidates": []}')

        with patch('urllib.request.urlopen', side_effect=open_request), patch(
            'audio_describer.core.sonarpad_service.time.sleep'
        ) as sleep:
            client.generate('prompt', None)
        self.assertEqual(10, len(calls))
        self.assertEqual(9, sleep.call_count)
        self.assertEqual(1, len({r.get_header('X-idempotency-key') for r in calls}))
        self.assertTrue(calls[0].get_header('X-idempotency-key'))
        self.assertTrue(all(r.data == calls[0].data for r in calls))

    def test_html_404_upload_uses_existing_retry_and_can_be_cancelled(self):
        from audio_describer.core.gemini_helpers import run_with_retry, GeminiRetryCancelledError
        error = SonarpadServiceError('HTML 404', 404, temporary_html_not_found=True)
        with patch('audio_describer.core.gemini_helpers.time.sleep') as sleep:
            from unittest.mock import Mock
            upload = Mock(side_effect=[error, error, {'upload_id': 'ok'}])
            self.assertEqual({'upload_id': 'ok'}, run_with_retry(upload))
            self.assertEqual(2, sleep.call_count)
        upload = Mock(side_effect=error)
        with patch('audio_describer.core.gemini_helpers.time.sleep',
                   side_effect=GeminiRetryCancelledError('cancelled')):
            with self.assertRaises(GeminiRetryCancelledError):
                run_with_retry(upload)
        self.assertEqual(1, upload.call_count)

    def test_backend_retry_after_is_bounded(self):
        from audio_describer.core.sonarpad_service import _http_retry_after
        self.assertEqual(5, _http_retry_after('{}'))
        self.assertEqual(5, _http_retry_after('{"retry_after":999999}'))
        self.assertEqual(9, _http_retry_after('{"retry_after":9}'))
        self.assertEqual(5, _http_retry_after('garbage'))


if __name__ == '__main__':
    unittest.main()
