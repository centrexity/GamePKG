"""Asset-free CI download retries must never bypass integrity or size checks."""
from pathlib import Path
import hashlib
import io
import sys
import tempfile
import unittest
from unittest.mock import patch
from urllib.error import HTTPError, URLError
sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'tools'))
from ci_download import download


class Downloads(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.output = self.root / 'component.zip'
        self.payload = b'verified fixture bytes'
        self.sha = hashlib.sha256(self.payload).hexdigest()
        self.url = 'https://example.invalid/pinned.zip'

    def run_download(self, **kwargs):
        return download(self.url, self.output, self.sha, **kwargs)

    def test_transient_gateway_then_valid_bytes(self):
        with patch('ci_download.urllib.request.urlopen', side_effect=[
                HTTPError(self.url, 504, 'Gateway timeout', {}, None),
                io.BytesIO(self.payload)]) as fetch, patch('ci_download.time.sleep') as sleep:
            self.assertEqual(self.run_download(), self.output)
            self.assertEqual(fetch.call_count, 2)
            self.assertEqual(fetch.call_args_list[0].args[0], self.url)
            self.assertTrue(fetch.call_args_list[1].args[0].startswith(self.url + '?gamepkg_ci_retry='))
            sleep.assert_called_once_with(1)
        self.assertEqual(self.output.read_bytes(), self.payload)
        self.assertEqual(list(self.root.iterdir()), [self.output])

    def test_transport_failures_stop_at_attempt_bound(self):
        with patch('ci_download.urllib.request.urlopen', side_effect=URLError('offline')) as fetch, patch('ci_download.time.sleep'):
            with self.assertRaises(URLError):
                self.run_download(attempts=3)
            self.assertEqual(fetch.call_count, 3)
        self.assertEqual(list(self.root.iterdir()), [])

    def test_partial_transport_failure_restarts_hash_and_file(self):
        partial = io.BytesIO(self.payload[:4])
        original_read = partial.read
        calls = 0

        def broken_read(size):
            nonlocal calls
            calls += 1
            if calls > 1:
                raise ConnectionResetError('connection reset mid-download')
            return original_read(size)

        partial.read = broken_read
        with patch('ci_download.urllib.request.urlopen', side_effect=[partial, io.BytesIO(self.payload)]), patch('ci_download.time.sleep'):
            self.run_download()
        self.assertEqual(self.output.read_bytes(), self.payload)
        self.assertEqual(list(self.root.iterdir()), [self.output])

    def test_exact_size_bound_is_allowed(self):
        with patch('ci_download.urllib.request.urlopen', return_value=io.BytesIO(self.payload)):
            self.run_download(max_bytes=len(self.payload))
        self.assertEqual(self.output.read_bytes(), self.payload)

    def test_not_found_is_not_retried(self):
        with patch('ci_download.urllib.request.urlopen', side_effect=HTTPError(self.url, 404, 'Missing', {}, None)) as fetch:
            with self.assertRaises(HTTPError):
                self.run_download()
            self.assertEqual(fetch.call_count, 1)
        self.assertEqual(list(self.root.iterdir()), [])

    def test_corrupt_bytes_are_rejected_without_retry(self):
        with patch('ci_download.urllib.request.urlopen', return_value=io.BytesIO(b'changed')) as fetch:
            with self.assertRaisesRegex(RuntimeError, 'digest mismatch'):
                self.run_download()
            self.assertEqual(fetch.call_count, 1)
        self.assertEqual(list(self.root.iterdir()), [])

    def test_oversized_payload_rejected_before_installing(self):
        with patch('ci_download.urllib.request.urlopen', return_value=io.BytesIO(self.payload)):
            with self.assertRaisesRegex(RuntimeError, 'size bound'):
                self.run_download(max_bytes=3)
        self.assertEqual(list(self.root.iterdir()), [])

    def test_existing_file_is_preserved(self):
        self.output.write_bytes(b'keep')
        with patch('ci_download.urllib.request.urlopen') as fetch:
            with self.assertRaises(FileExistsError):
                self.run_download()
            fetch.assert_not_called()
        self.assertEqual(self.output.read_bytes(), b'keep')

    def test_insecure_url_and_invalid_bounds_do_not_fetch(self):
        with patch('ci_download.urllib.request.urlopen') as fetch:
            with self.assertRaises(ValueError):
                download('http://example.invalid', self.output, self.sha)
            with self.assertRaises(ValueError):
                self.run_download(attempts=0)
            fetch.assert_not_called()


if __name__ == '__main__':
    unittest.main()
