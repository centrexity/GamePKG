"""Canonical bytes for maintainer checks of pinned native sources.

Accept the exact source-kit bytes, or the exact CRLF checkout of the pinned
Git object. Never normalize away edits or change the checkout.
"""
import hashlib
import subprocess


def canonical_reference_bytes(source, relative_path, checkout, expected_hash, expected_revision):
    if hashlib.sha256(checkout).hexdigest() == expected_hash:
        return checkout
    revision = subprocess.check_output(['git', '-C', str(source), 'rev-parse', 'HEAD'],
                                       text=True, encoding='utf-8').strip()
    if revision != expected_revision:
        raise RuntimeError('Unreviewed original runtime revision')
    canonical = subprocess.check_output(['git', '-C', str(source), 'show', 'HEAD:'+relative_path])
    if hashlib.sha256(canonical).hexdigest() != expected_hash:
        raise RuntimeError('Unreviewed pinned source')
    if checkout != canonical.replace(b'\n', b'\r\n'):
        raise RuntimeError('Checkout differs beyond Windows line endings')
    return canonical
