"""Bounded, checksum-pinned downloads for maintainer CI fixtures only."""
import hashlib
from pathlib import Path
import secrets
import tempfile
import time
import urllib.error
import urllib.request
import urllib.parse


def download(url, destination, expected_sha256, *, timeout=60,
             max_bytes=1024 * 1024 * 1024, attempts=4):
    destination = Path(destination)
    if not url.startswith('https://'):
        raise ValueError('CI fixture downloads require HTTPS')
    if destination.exists():
        raise FileExistsError(destination)
    if attempts < 1 or max_bytes < 1:
        raise ValueError('Positive download bounds required')
    for attempt in range(attempts):
        temporary = None
        try:
            with tempfile.NamedTemporaryFile(dir=destination.parent, delete=False) as output:
                temporary = Path(output.name)
                digest, size = hashlib.sha256(), 0
                request_url = url
                if attempt:
                    parts = urllib.parse.urlsplit(url)
                    query = parts.query + ('&' if parts.query else '')
                    query += 'gamepkg_ci_retry=' + secrets.token_hex(8)
                    request_url = urllib.parse.urlunsplit(parts._replace(query=query))
                with urllib.request.urlopen(request_url, timeout=timeout) as response:
                    while True:
                        part = response.read(min(1024 * 1024, max_bytes - size + 1))
                        if not part:
                            break
                        size += len(part)
                        if size > max_bytes:
                            raise RuntimeError('CI fixture exceeds download size bound')
                        digest.update(part)
                        output.write(part)
            if digest.hexdigest() != expected_sha256:
                raise RuntimeError('Official CI fixture digest mismatch')
            temporary.replace(destination)
            return destination
        except urllib.error.HTTPError as error:
            if error.code not in (408, 429, 500, 502, 503, 504) or attempt + 1 == attempts:
                raise
        except (urllib.error.URLError, TimeoutError, ConnectionError):
            if attempt + 1 == attempts:
                raise
        finally:
            if temporary is not None:
                temporary.unlink(missing_ok=True)
        print('Temporary CI download failure; retrying verified fixture', flush=True)
        time.sleep(2 ** attempt)
    raise AssertionError('Unreachable download retry state')
