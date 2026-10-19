"""Maintainer check: native release notices include embedded Wasm dependencies."""
from pathlib import Path
import subprocess
import sys
sys.path.insert(0, str(Path(__file__).resolve().parents[1]/'tools'))
from package_release import dependency_metadata
host = next(line.split(': ', 1)[1] for line in subprocess.check_output(['rustc', '-vV'], text=True).splitlines() if line.startswith('host: '))
metadata = dependency_metadata(host)
resolved = {node['id'] for node in metadata['resolve']['nodes']}
packages = [p for p in metadata['packages'] if p['id'] in resolved and p['id'] not in metadata['workspace_members']]
assert len(metadata['packages']) == len({p['id'] for p in metadata['packages']})
for name in ['nanomp3', 'nanomp3-core']:
    package, = [p for p in packages if p['name'] == name]
    assert package['version'] == '0.2.0'
    root = Path(package['manifest_path']).parent
    assert any(p.is_file() and p.name.upper().startswith(('LICENSE', 'COPYING')) for p in root.iterdir())
    assert package['license'] == 'MIT OR Apache-2.0'
print('Embedded decoder dependencies are resolved once with their pinned source and license files.')
