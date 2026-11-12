#!/usr/bin/env python3
"""Maintainer-only exporter. Player builds consume the reviewed data with Rust."""
import argparse
import hashlib
import importlib.util
import json
from pathlib import Path
import re
import shlex
import subprocess


def module(path, name):
    spec = importlib.util.spec_from_file_location(name, path)
    value = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(value)
    return value


def digest(data):
    return hashlib.sha256(data).hexdigest()


def tree_manifest(root):
    result = []
    for path in sorted(root.rglob('*')):
        relative = path.relative_to(root).as_posix()
        if path.is_symlink():
            result.append(dict(path=relative, link=str(path.readlink())))
        elif path.is_file():
            result.append(dict(path=relative, sha256=digest(path.read_bytes())))
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--source', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    source = args.source.resolve()
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=True)
    assert subprocess.check_output(['git', '-C', source, 'rev-parse', 'HEAD'], text=True).strip() == 'ce95f7deed5bbf726e50a9d5fe5227437194cf3b'
    embed = module(source/'tools/embed_assets.py', 'asset_export')
    sysroot = module(source/'tools/linux_sysroot.py', 'dependency_export')

    generated = source/'build/linux/generated/hud_hires_assets.c'
    template = generated.read_text()
    blobs = []
    items = []
    for index, (folder, asset, _) in enumerate(embed.textures()):
        items.append((f'asset{index}', folder/(asset['name']+'.png'), 'bytes'))
    for index, name in enumerate(embed.font_files()):
        items.append((f'font{index}', embed.FONT_ASSETS/name, 'bytes'))
    for index, name in enumerate(embed.menu_files()):
        items.append((f'menu{index}', embed.MENU_ASSETS/name, 'bytes'))
    for name, symbol in embed.smaa_files():
        items.append((symbol, embed.SMAA_ASSETS/name, 'ascii-nul' if name.endswith('.hlsl') else 'bytes'))
    for symbol, path in [('platform_window_icon', embed.WINDOW_ICON), ('platform_menu_cursor', embed.MENU_CURSOR)]:
        if (source/path).is_file():
            items.append((symbol, path, 'bytes'))
    for symbol, path, mode in items:
        raw = (source/path).read_bytes()
        data = bytes(byte if byte < 128 else 32 for byte in raw)+b'\0' if mode == 'ascii-nul' else raw
        pattern = re.compile(r'((?:static )?const unsigned int '+re.escape(symbol)+r'\[\] = \{\n)(.*?)(\n\};)', re.S)
        match = pattern.search(template)
        assert match, symbol
        expected = '\n'.join(embed.words(data))
        assert match[2] == expected, symbol
        template = template[:match.start(2)]+'@@'+symbol+'@@'+template[match.end(2):]
        blobs.append(dict(symbol=symbol, source=path.as_posix(), sha256=digest(raw), mode=mode))
    (output/'resources.c.in').write_text(template)
    headers = []
    for name in ['halo_msvc_semantics.h', 'platform_msvc_semantics.h']:
        (output/name).write_bytes((source/'build/linux'/name).read_bytes())
        headers.append(dict(input=name, output='build/linux/'+name))
    (output/'dependencies.stamp').write_text(sysroot.stamp_text())
    headers.append(dict(input='dependencies.stamp', output='build/linux/third_party/stamp'))

    text = (source/'build.ninja').read_text()
    start = text.index('build linux: phony')
    end = text.index('# Android build', start)
    prefix = text[:end]
    blocks = []
    for line in prefix.splitlines(keepends=True):
        if line and not line[0].isspace() and not line.startswith('#'):
            blocks.append([])
        if not blocks:
            blocks.append([])
        blocks[-1].append(line)
    removed = {'linux_msvc_semantics', 'linux_pgo_train', 'linux_embed_assets', 'linux_sysroot', 'linux_sdl3'}
    sdl_rule = next(''.join(block) for block in blocks if block[0].startswith('rule linux_sdl3'))
    command = re.search(r'  command = (.*?)(?:\n  description)', sdl_rule, re.S)[1].replace('$\n', '')
    words = shlex.split(command)
    commands = []
    for program in ['cmake', 'ninja']:
        index = words.index(program)
        stop = next((i for i in range(index+1, len(words)) if words[i] in ('>', '&&')), len(words))
        values = [value.replace('$$PWD', '{source}') for value in words[index+1:stop]]
        if program == 'cmake':
            values += ['-DCMAKE_DISABLE_FIND_PACKAGE_Python3=TRUE', '-DCMAKE_DISABLE_FIND_PACKAGE_Python=TRUE']
        commands.append(dict(program=program, args=values))
    rendered = []
    for block in blocks:
        head = block[0].strip()
        joined = ''.join(block).replace('$\n', '')
        if head.startswith(('python =', 'configure_args =')):
            continue
        if head.startswith('rule ') and head[5:] in removed:
            continue
        if head.startswith('build ') and any(re.search(r':\s*'+name+r'\b', joined) for name in removed):
            continue
        if head == 'rule linux_link':
            rendered.append('rule linux_link\n  command = $linux_cc $ldflags -o $out @$out.rsp $libs\n  description = LINUX LINK $out\n  rspfile = $out.rsp\n  rspfile_content = $in_newline\n')
        elif head.startswith('build ') and re.search(r':\s+linux_cc\s', re.sub(r'\$\n\s*', ' ', joined)):
            index = next(i for i, line in enumerate(block) if line.startswith('  cflags ='))
            header = ''.join(block[:index]).rstrip()
            assert '||' not in header
            separator = ' $\n' if '|' in header else ' | $\n'
            rendered.append(header+separator+'    build/gamepkg-compiler.stamp\n'+''.join(block[index:]))
        else:
            rendered.extend(block)
    ninja = ''.join(rendered).rstrip()+'\n'
    assert 'python' not in ninja.casefold()
    (output/'build.ninja').write_text(ninja)
    subprocess.run(['ninja', '-f', str(output/'build.ninja'), '-t', 'targets'], cwd=source,
                   stdout=subprocess.DEVNULL, check=True)
    # Check the parsed graph, including rules whose colon/command wrap onto
    # different physical lines. Every translation unit must track the compiler.
    compiler_outputs = subprocess.check_output(
        ['ninja', '-f', str(output/'build.ninja'), '-t', 'query', 'build/gamepkg-compiler.stamp'],
        cwd=source, text=True).split('  outputs:\n', 1)[1]
    compiler_outputs = {line.strip() for line in compiler_outputs.splitlines()}
    compile_commands = json.loads(subprocess.check_output(
        ['ninja', '-f', str(output/'build.ninja'), '-t', 'compdb', 'linux_cc'], cwd=source, text=True))
    assert compile_commands and all(command['output'] in compiler_outputs for command in compile_commands)

    packages = []
    for path, sha256, snapshot in sysroot.PACKAGES:
        urls = [mirror+path for mirror in [*sysroot.DEBIAN_MIRRORS, sysroot.SNAPSHOT.format(snapshot)]]
        packages.append(dict(urls=urls, sha256=sha256, format='deb', output='build/linux/third_party/sysroot', strip=0))
    packages.append(dict(urls=sysroot.SDL_URLS, sha256=sysroot.SDL_SHA256, format='tar-gz',
        output='build/linux/third_party/SDL3', strip=1))
    plan = dict(version=1, ninja='build.ninja', ninja_output='build/gamepkg-linux.ninja', headers=headers,
        compiler_stamp='build/gamepkg-compiler.stamp', command_inputs=['tools/linux_sysroot.cmake'],
        command_outputs=['build/linux/sdl3-build/libSDL3.so.0', 'build/linux/sdl3-build/include-config-release/build_config/SDL_build_config.h'],
        reset_cmake_dirs=['build/linux/sdl3-build'],
        resource_template='resources.c.in', resource_output='build/linux/generated/hud_hires_assets.c',
        resource_sha256=digest(generated.read_bytes()), blobs=blobs, downloads=packages,
        skip_archive_paths=['/'.join(path) for path in sysroot.UNUSED], commands=commands,
        dependency_trees=[dict(root='build/linux/third_party/'+name, files=tree_manifest(source/'build/linux/third_party'/name))
            for name in ['sysroot', 'SDL3']],
        required_defines=dict(path='build/linux/sdl3-build/include-config-release/build_config/SDL_build_config.h',
            names=sysroot.SDL_REQUIRED),
        link_objects='build/linux/halo.rsp', binary='build/linux/halo')
    (output/'plan.json').write_text(json.dumps(plan, indent=2)+'\n')
    print('Prepared native build data:', output)
    print('Embedded source template:', len(template), 'bytes; blobs fetched from pinned source:', len(blobs))


if __name__ == '__main__':
    main()
