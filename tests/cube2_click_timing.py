"""Pinned original attack gates compared with the production mouse-state adapter.

This is a source-fragment oracle, not a substitute for complete engine/UI tests.
"""
from pathlib import Path
import importlib.util, os, subprocess, tempfile

project = Path(__file__).resolve().parents[1]
spec = importlib.util.spec_from_file_location('cube2_build', project/'tools/build_cube2_engine.py')
build = importlib.util.module_from_spec(spec)
spec.loader.exec_module(build)
build.verify()
fps = (build.SOURCE/'fpsgame/fps.cpp').read_text()
weapon = (build.SOURCE/'fpsgame/weapon.cpp').read_text()
def extract(source, start, end):
    assert source.count(start) == 1, start
    begin = source.index(start)
    return source[begin:source.index(end, begin)]
attack = extract(fps, '    void doattack(bool on)', '\n    VARP(jumpspawn,')
# Preserve every original eligibility guard, ending before ammo/shot effects.
shoot = extract(weapon, '    void shoot(fpsent *d, const vec &targ)', '        if(!d->ammo[d->gunselect])') + '    }\n'
with tempfile.TemporaryDirectory(prefix='gamepkg-cube-click-timing-') as temporary:
    root = Path(temporary)
    (root/'original-click.h').write_text(attack+'\n'+shoot)
    binary = root/'click-timing'
    subprocess.run(['g++','-O2','-Wall','-Wextra','-Werror','-Wno-unused-parameter','-fsanitize=address,undefined','-fno-omit-frame-pointer','-I'+str(root),str(project/'tests/fixtures/cube2-click-timing.cpp'),'-o',str(binary)],check=True)
    subprocess.run([str(binary)],check=True,timeout=10,env=dict(os.environ,ASAN_OPTIONS='detect_leaks=1:halt_on_error=1',UBSAN_OPTIONS='halt_on_error=1:print_stacktrace=1'))
print('Pinned attack/shot gates: same-update click, update-spanning click, cooldown, duplicate edges and physical/controller overlap match production mouse state under sanitizers. Full GUI event timing remains a separate check.')
