"""Validate and stage public documentation; the authenticated library stays local."""
import argparse
import hashlib
from html.parser import HTMLParser
from pathlib import Path
import shutil
from urllib.parse import unquote, urlsplit

PROJECT = Path(__file__).resolve().parents[1]
ROOT = PROJECT / 'html'
DEMO_NAMES = ['collect-stars.gamepkg', 'shape-playground.gamepkg', 'png-sprite-playground.gamepkg', 'zelda3.gamepkg']
DEMOS = [ROOT / 'demos' / name for name in DEMO_NAMES]
PUBLIC = [ROOT / 'index.html', *sorted((ROOT / 'guide').glob('*')), *sorted((ROOT / 'spec').glob('*')), *DEMOS, ROOT / 'demos/packages.sha256']

class Links(HTMLParser):
    def __init__(self):
        super().__init__()
        self.links = []
        self.ids = set()
    def handle_starttag(self, tag, attrs):
        attrs = dict(attrs)
        if 'id' in attrs:
            assert attrs['id'] not in self.ids, 'Duplicate HTML id'
            self.ids.add(attrs['id'])
        for name in ('href', 'src'):
            if name in attrs:
                self.links.append(attrs[name])

def check():
    recorded = dict((name, digest) for digest, name in (line.split() for line in (ROOT / 'demos/packages.sha256').read_text().splitlines()))
    assert set(recorded) == set(DEMO_NAMES), 'Unexpected public demo checksum entries'
    for package in DEMOS:
        assert package.is_file() and not package.is_symlink() and package.stat().st_size <= 1024 * 1024
        assert hashlib.sha256(package.read_bytes()).hexdigest() == recorded[package.name], 'Public demo checksum mismatch'
    pages = {}
    for path in PUBLIC:
        if path.suffix == '.html':
            parser = Links()
            parser.feed(path.read_text())
            pages[path.resolve()] = parser
    allowed = {p.resolve() for p in PUBLIC}
    for path, parser in pages.items():
        for link in parser.links:
            url = urlsplit(link)
            if url.scheme or url.netloc:
                continue
            assert not url.path.startswith('/'), f'{path}: root-relative URL breaks project hosting: {link}'
            target = (path.parent / unquote(url.path)).resolve() if url.path else path
            if target.is_dir():
                target /= 'index.html'
            assert target in allowed, f'{path}: unpublished local target: {link}'
            if url.fragment and target in pages:
                assert unquote(url.fragment) in pages[target].ids, f'{path}: missing fragment: {link}'
    return pages

if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', type=Path)
    args = parser.parse_args()
    pages = check()
    if args.output:
        args.output.mkdir(parents=True, exist_ok=False)
        for source in PUBLIC:
            target = args.output / source.relative_to(ROOT)
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(source, target)
        (args.output / '.nojekyll').touch()
    print(f'Validated {len(pages)} public HTML pages and their local links.')
