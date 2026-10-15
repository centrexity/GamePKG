"""Render repository documentation as standalone static HTML pages.

Requires Python-Markdown (Debian/Ubuntu package: python3-markdown).
Run from any directory: python3 tools/render_site_docs.py
"""
from pathlib import Path
import html
import re
import markdown

ROOT = Path(__file__).resolve().parents[1]
TEMPLATE = (ROOT / 'html/guide/index.html').read_text().split('<body>')[0]
PAGES = [
    ('docs/DEVELOPING_GAMES.md', 'html/guide/developers.html', 'Developer instructions',
     'Create GamePKG games with pinned crates, portable input, scoped APIs, and verified local builds.', 'index.html'),
    ('html/guide/security.md', 'html/guide/security.html', 'Security model',
     'How GamePKG verifies source packages, isolates compilation, and limits running games.',
     'index.html'),
    ('html/spec/index.md', 'html/spec/index.html', 'Format specification',
     'GamePKG package signatures, runtime profiles, limits, catalogs, and portable stores.',
     '../guide/index.html'),
]
for source, destination, title, description, quickstart in PAGES:
    head = re.sub(r'<title>.*?</title>', f'<title>{html.escape(title)} — GamePKG</title>', TEMPLATE)
    head = re.sub(r'<meta name="description" content="[^"]*">',
                  f'<meta name="description" content="{html.escape(description, quote=True)}">', head)
    body = markdown.markdown((ROOT / source).read_text(), extensions=['fenced_code', 'tables'])
    output = head + '<body><main>\n'
    output += f'<nav aria-label="Main"><a href="../index.html">← GamePKG</a> · <a href="{quickstart}">Start playing</a></nav>\n'
    output += body + '\n<footer>'
    output += '<a href="../support/index.html">Support GamePKG</a> · <a href="../guide/allowlist.html">Approved game libraries</a> · '
    output += f'<a href="https://github.com/centrexity/GamePKG/blob/main/{source}">Documentation source</a> · GamePKG · Apache 2.0'
    output += '</footer>\n</main></body></html>\n'
    (ROOT / destination).write_text(output)
    print(destination)
