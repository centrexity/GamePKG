#!/usr/bin/env python3
"""Maintainer-only Cube 2 bridge build; players will use the Rust build path.

No upstream file is modified. Stage and patch a verified source copy under target.
This development executable is not an installed GamePKG runtime.
"""
import argparse
import hashlib
import json
from pathlib import Path
import shutil
import subprocess

PROJECT = Path(__file__).resolve().parents[1]
SOURCE = PROJECT / 'engines/cube2/upstream'
LOCK = PROJECT / 'engines/cube2/upstream-lock.json'


def verify(source=SOURCE, lock_path=LOCK):
    lock = json.loads(lock_path.read_text())
    expected = lock['files']
    actual = {p.relative_to(source).as_posix() for p in source.rglob('*') if p.is_file()}
    if actual != set(expected):
        raise ValueError('Cube 2 source file set differs from the pinned revision')
    for name, digest in expected.items():
        path = source / name
        if path.is_symlink() or hashlib.sha256(path.read_bytes()).hexdigest() != digest:
            raise ValueError(f'Cube 2 source integrity failed: {name}')
    return lock


def replace_once(text, before, after):
    if text.count(before) != 1:
        raise ValueError(f'Cube 2 adaptation anchor changed: {before!r}')
    return text.replace(before, after, 1)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--verify-only', action='store_true')
    parser.add_argument('--jobs', type=int, default=2, choices=range(1, 17))
    args = parser.parse_args()
    lock = verify()
    print(f"Verified Cube 2 r{lock['revision']}: {len(lock['files'])} pinned source files.", flush=True)
    if args.verify_only:
        return
    stage = PROJECT / 'target/cube2-bridge-build'
    if stage.exists():
        shutil.rmtree(stage)
    stage.mkdir(parents=True)
    # Copy only pinned input; never compile an unexpected extra source file.
    for name in lock['files']:
        path = stage / name
        path.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(SOURCE / name, path)
    (stage / 'enet/check_cflags.sh').chmod(0o700)
    shutil.copyfile(PROJECT / 'engines/cube2/adapters/syscall_guard.h', stage / 'engine/syscall_guard.h')
    shutil.copyfile(PROJECT / 'engines/cube2/adapters/persistence.h', stage / 'engine/persistence.h')
    shutil.copyfile(PROJECT / 'engines/cube2/adapters/audio.h', stage / 'engine/audio.h')
    shutil.copyfile(PROJECT / 'engines/cube2/adapters/network.h', stage / 'enet/gamepkg_network.h')
    unix = stage / 'enet/unix.c'
    text = unix.read_text()
    anchor = 'int\nenet_address_set_host_ip (ENetAddress * address, const char * name)\n{'
    if text.count(anchor) != 1:raise ValueError('Cube 2 ENet adaptation anchor changed')
    unix.write_text(text.split(anchor, 1)[0]+'#include "gamepkg_network.h"\n#endif\n')
    bridge = (PROJECT / 'engines/cube2/adapters/frame_bridge.h').read_bytes()
    (stage / 'engine/gamepkg_bridge.h').write_bytes(bridge)
    path = stage / 'engine/main.cpp'
    text = path.read_text()
    text = replace_once(text, '#include "engine.h"', '#include "engine.h"\n#include "gamepkg_bridge.h"')
    text = replace_once(text, 'void quit()                     // normal exit\n{', 'void quit()                     // normal exit\n{\n    gamepkg_quit();')
    text = replace_once(text, 'int main(int argc, char **argv)\n{', 'int main(int argc, char **argv)\n{\n    gamepkg_prepare();')
    text = replace_once(text, '        swapbuffers();\n        renderedframe = inbetweenframes = true;', '        gamepkg_present();\n        swapbuffers();\n        renderedframe = inbetweenframes = true;')
    text = replace_once(text, '    for(;;)\n    {\n        static int frames = 0;', '    for(;;)\n    {\n        gamepkg_input();\n        static int frames = 0;')
    text = replace_once(text, '                        SDL_GetWindowSize(screen, &screenw, &screenh);', '                        screenw = gamepkg_width; screenh = gamepkg_height;')
    path.write_text(text)
    render = stage / 'engine/rendergl.cpp'
    text = replace_once(render.read_text(), '    return SDL_GL_GetProcAddress(name);', '    extern void *gamepkg_glproc(const char *name); return gamepkg_glproc(name);')
    text = replace_once(text, '        crosshairs[i] = textureload(name, 3, true);\n    }\n}', '        crosshairs[i] = textureload(name, 3, true);\n        if(crosshairs[i] == notexture) crosshairs[i] = textureload(i == 2 ? "data/hit.png" : i == 1 ? "data/teammate.png" : "data/crosshair.png", 3, true);\n    }\n}')
    render.write_text(text)
    world=stage / 'engine/worldio.cpp'
    world.write_text(replace_once(world.read_text(), '    int loadingstart = SDL_GetTicks();', '    extern void gamepkg_loading(); gamepkg_loading();\n    int loadingstart = SDL_GetTicks();'))
    tools = stage / 'shared/tools.h'
    text = replace_once(tools.read_text(), 'inline void *operator new(size_t, void *p) { return p; }\ninline void *operator new[](size_t, void *p) { return p; }\ninline void operator delete(void *, void *) {}\ninline void operator delete[](void *, void *) {}', '#include <new>')
    tools.write_text(text)
    sound = stage / 'engine/sound.cpp'
    text = sound.read_text()
    text = replace_once(text, 'Mix_OpenAudio(soundfreq, MIX_DEFAULT_FORMAT, 2, soundbufferlen)', 'Mix_OpenAudio(48000, AUDIO_S16SYS, 2, soundbufferlen)')
    text = replace_once(text, 'Mix_AllocateChannels(soundchans);', 'extern void gamepkg_audio_hook(); gamepkg_audio_hook(); Mix_AllocateChannels(soundchans);')
    sound.write_text(text)
    subprocess.run(['make', '-C', str(stage), f'-j{args.jobs}', 'client', 'CC=/usr/bin/gcc', 'CXX=/usr/bin/g++', 'AR=/usr/bin/ar', 'CXXFLAGS=-O3 -fomit-frame-pointer -ffast-math -pthread'], check=True)
    print(stage / 'sauer_client')


if __name__ == '__main__':
    main()
