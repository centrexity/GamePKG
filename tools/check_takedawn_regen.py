"""Maintainer reference only: generate pinned Takedawn code from a private ISO.

This is not a player build path. The GamePKG player must not require Python.
Only the XBE is copied into a sealed RAM file; no disc file is extracted to
disk. Generated game-derived code stays in the selected private checkout.
"""
import argparse
import fcntl
import hashlib
import json
import os
from pathlib import Path
import signal
import subprocess
import sys

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--disc', type=Path, required=True)
parser.add_argument('--source', type=Path, required=True)
parser.add_argument('--toolkit', type=Path, required=True)
parser.add_argument('--log', type=Path, required=True)
args = parser.parse_args()
project = Path(__file__).resolve().parents[1]
lock = json.loads((project / 'engines/takedawn/upstream-lock.json').read_text())
for path, key in [(args.source, 'game'), (args.toolkit, 'toolkit')]:
    commit = subprocess.check_output(['git', '-C', str(path), 'rev-parse', 'HEAD'], text=True).strip()
    if commit != lock[key]['commit']:
        raise RuntimeError(f'{key} checkout is not the pinned revision')
report = json.loads(subprocess.check_output([
    str(project / 'target/release/gamepkg'), 'check-xbox-disc', str(args.disc), '--json'
], text=True))
if report['verified_files'] != 716 or report['manifest_id'] != lock['disc_manifest']['id']:
    raise RuntimeError('disc verification did not qualify the complete supported image')
sys.path.insert(0, str(args.toolkit))
from tools.xiso.xdvdfs import Xiso
with Xiso(args.disc) as disc:
    executable = disc.read(disc.find('default.xbe'))
if hashlib.sha256(executable).hexdigest() != lock['disc_manifest']['executable_sha256']:
    raise RuntimeError('independent original reader did not return the expected XBE')
descriptor = os.memfd_create('gamepkg-private-takedawn-xbe', os.MFD_ALLOW_SEALING)
try:
    with os.fdopen(os.dup(descriptor), 'wb') as output:
        output.write(executable)
    del executable
    fcntl.fcntl(descriptor, fcntl.F_ADD_SEALS,
                fcntl.F_SEAL_SHRINK | fcntl.F_SEAL_GROW | fcntl.F_SEAL_WRITE | fcntl.F_SEAL_SEAL)
    folder = args.source / 'Burnout 3 Takedown'
    made_folder = not folder.exists()
    folder.mkdir(exist_ok=True)
    link = folder / 'default.xbe'
    if link.exists() or link.is_symlink():
        raise RuntimeError('private checkout already has an XBE; refusing to replace it')
    link.symlink_to(f'/proc/{os.getpid()}/fd/{descriptor}')
    try:
        environment = dict(os.environ, XBOXRECOMP_DIR=str(args.toolkit.resolve()),
                           TAKEDAWN_DEPS=str(args.source / 'build/private-deps'))
        with args.log.open('w') as output:
            child = subprocess.Popen(['bash', 'tools/release/gen.sh'], cwd=args.source,
                                     env=environment, stdout=output, stderr=subprocess.STDOUT,
                                     start_new_session=True)
            try:
                status = child.wait(timeout=900)
            except subprocess.TimeoutExpired:
                os.killpg(child.pid, signal.SIGKILL)
                child.wait()
                raise
            if status:
                raise subprocess.CalledProcessError(status, child.args)
        print((args.source / 'src/game/recomp/gen/regen.stamp').read_text())
        print('Original generator completed with a sealed RAM-only XBE; no disc assets extracted.')
    finally:
        link.unlink()
        if made_folder:
            folder.rmdir()
finally:
    os.close(descriptor)
